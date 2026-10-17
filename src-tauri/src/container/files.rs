//! Log files inside a container: listing them, following them, and a
//! snapshot where following cannot work (issue #101). The rules are in
//! [`crate::logfiles`]; this is the Docker and Podman side, all through the
//! engine's API.

use std::sync::Arc;
use std::time::Duration;

use bollard::exec::{CreateExecOptions, StartExecResults};
use bollard::query_parameters as q;
use futures_util::StreamExt;

use super::{ContainerManager, Host};
use crate::logfiles::{self, Archived, FoundFile, Refusal};

/// What an exec printed, and its exit code.
pub struct Ran {
    pub text: String,
    pub code: Option<i64>,
}

impl Ran {
    /// Why it could not run, if that is what happened: 126 and 127 are the
    /// runtime saying the program is missing or not runnable.
    fn refusal(&self) -> Option<Refusal> {
        logfiles::refusal(&self.text).or(if matches!(self.code, Some(126 | 127)) { Some(Refusal::NoTool) } else { None })
    }
}

impl Host {
    /// Run `cmd` in the container (no shell, no terminal) and collect what it
    /// prints, up to `secs`.
    pub async fn exec_collect(&self, id: &str, cmd: Vec<String>, secs: u64) -> Result<Ran, String> {
        let work = async {
            let ex = self
                .docker
                .create_exec(id, CreateExecOptions { attach_stdout: Some(true), attach_stderr: Some(true), cmd: Some(cmd), ..Default::default() })
                .await
                .map_err(|e| e.to_string())?;
            let mut text = String::new();
            if let StartExecResults::Attached { mut output, .. } = self.docker.start_exec(&ex.id, None).await.map_err(|e| e.to_string())? {
                while let Some(chunk) = output.next().await {
                    text.push_str(&chunk.map_err(|e| e.to_string())?.to_string());
                    if text.len() > 2_000_000 {
                        break;
                    }
                }
            }
            let code = self.docker.inspect_exec(&ex.id).await.ok().and_then(|i| i.exit_code);
            Ok(Ran { text, code })
        };
        tokio::time::timeout(Duration::from_secs(secs), work).await.map_err(|_| format!("No answer within {secs} seconds"))?
    }

    /// The log files in a running container, written-to first.
    pub async fn find_log_files(&self, id: &str) -> Result<Vec<FoundFile>, String> {
        let ran = self
            .exec_collect(id, vec!["sh".into(), "-c".into(), logfiles::DISCOVER_SCRIPT.into()], 30)
            .await
            .map_err(|e| match logfiles::refusal(&e) {
                Some(Refusal::NotRunning) => NOT_RUNNING_LIST.to_string(),
                _ => e,
            })?;
        match ran.refusal() {
            Some(Refusal::NoTool) => Err(NO_SHELL.into()),
            Some(Refusal::NotRunning) => Err(NOT_RUNNING_LIST.into()),
            None => Ok(logfiles::parse_discovery(&ran.text)),
        }
    }

    /// The last `lines` lines of a file as it is now, through the engine's
    /// file copy: works without `tail`, and on a stopped container.
    pub async fn snapshot(&self, id: &str, path: &str, lines: usize) -> Result<String, String> {
        let mut path = path.to_string();
        // A symlink comes back as itself; glog's api.INFO is one.
        for _ in 0..5 {
            let mut stream = self.docker.download_from_container(id, Some(q::DownloadFromContainerOptions { path: path.clone() }));
            let mut buf = Vec::new();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|e| {
                    let m = e.to_string();
                    if m.contains("404") || m.to_lowercase().contains("no such") || m.contains("Could not find") {
                        format!("{path}: no such file in the container")
                    } else {
                        m
                    }
                })?;
                buf.extend_from_slice(&chunk);
                if buf.len() > logfiles::SNAPSHOT_LIMIT + (1 << 20) {
                    break;
                }
            }
            match logfiles::read_archive(&buf)? {
                Archived::File(bytes) => {
                    let mut text = logfiles::last_lines(&bytes, lines);
                    if bytes.len() >= logfiles::SNAPSHOT_LIMIT {
                        text.push_str(&format!("{}Only the first 16 MB of this file can be read this way.\n", logfiles::NOTE));
                    }
                    return Ok(text);
                }
                Archived::Link(target) => path = logfiles::resolve_link(&path, &target),
                Archived::Missing => return Err(format!("{path}: no such file in the container")),
            }
        }
        Err(format!("{path}: too many symlinks"))
    }

    async fn running(&self, id: &str) -> bool {
        self.docker
            .inspect_container(id, None::<q::InspectContainerOptions>)
            .await
            .ok()
            .and_then(|c| c.state)
            .and_then(|s| s.running)
            .unwrap_or(false)
    }
}

const NO_SHELL: &str = "This image has no shell, so Reach cannot list its files. Type the file's path instead.";
const NOT_RUNNING_LIST: &str = "The container is not running, so its files cannot be listed. Type a file's path to read it as it was left.";

/// The files' last lines as they are now, one after another, then why they
/// are not followed.
async fn send_snapshot(host: &Host, id: &str, paths: &[String], lines: u32, why: Refusal, out: &tauri::ipc::Channel<String>) {
    for p in paths {
        let mut text = if paths.len() > 1 { format!("==> {p} <==\n") } else { String::new() };
        match host.snapshot(id, p, lines as usize).await {
            Ok(body) => text.push_str(&logfiles::data(body)),
            Err(e) => text.push_str(&format!("{}{e}\n", logfiles::NOTE)),
        }
        if out.send(text.replace('\n', "\r\n")).is_err() {
            return;
        }
    }
    let note = match why {
        Refusal::NoTool => "This image has no tail, so this is the file as it is now, not followed. Read it again to refresh.",
        Refusal::NotRunning => "The container is not running: this is the file as it was left.",
    };
    let _ = out.send(logfiles::note(note));
}

impl ContainerManager {
    /// Follow files inside a container into `out`, as `tail -F`; where that
    /// cannot run, send a snapshot and say why.
    pub fn follow_files(&mut self, stream_id: String, host: Arc<Host>, id: String, paths: Vec<String>, lines: u32, out: tauri::ipc::Channel<String>) {
        if let Some(old) = self.streams.remove(&stream_id) {
            old.abort();
        }
        let task = tokio::spawn(async move {
            // One quick look first, so a missing tail or a stopped container
            // reads as a sentence instead of a runtime error. A missing file
            // is not a refusal: tail -F waits for it to appear.
            let mut probe = vec!["tail".to_string(), "-n".into(), "0".into(), "--".into()];
            probe.extend(paths.iter().cloned());
            let why = match host.exec_collect(&id, probe, 15).await {
                Ok(r) => r.refusal(),
                Err(e) => match logfiles::refusal(&e) {
                    Some(r) => Some(r),
                    None => {
                        let _ = out.send(logfiles::note(&e));
                        return;
                    }
                },
            };
            if let Some(why) = why {
                send_snapshot(&host, &id, &paths, lines, why, &out).await;
                return;
            }
            let ex = match host
                .docker
                .create_exec(
                    &id,
                    CreateExecOptions { attach_stdout: Some(true), attach_stderr: Some(true), cmd: Some(logfiles::tail_command(&paths, lines)), ..Default::default() },
                )
                .await
            {
                Ok(ex) => ex,
                Err(e) => {
                    let _ = out.send(logfiles::note(&e.to_string()));
                    return;
                }
            };
            match host.docker.start_exec(&ex.id, None).await {
                Ok(StartExecResults::Attached { mut output, .. }) => {
                    while let Some(chunk) = output.next().await {
                        match chunk {
                            Ok(c) => {
                                if out.send(logfiles::data(c.to_string())).is_err() {
                                    return;
                                }
                            }
                            Err(e) => {
                                let _ = out.send(logfiles::note(&e.to_string()));
                                return;
                            }
                        }
                    }
                }
                Ok(StartExecResults::Detached) => {}
                Err(e) => {
                    let _ = out.send(logfiles::note(&e.to_string()));
                    return;
                }
            }
            if !host.running(&id).await {
                let _ = out.send(logfiles::note("The container stopped; the file is no longer followed."));
            }
        });
        self.streams.insert(stream_id, task);
    }
}
