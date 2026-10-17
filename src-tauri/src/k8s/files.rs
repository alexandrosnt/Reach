//! Log files inside a pod's container (issue #101): the Kubernetes side of
//! [`crate::logfiles`], through the exec API, as `kubectl exec` runs it.
//! Kubernetes has no file copy of its own (`kubectl cp` runs tar inside the
//! container), so an image without `tail` gets `cat` for a snapshot, and one
//! with neither says so.

use std::sync::Arc;
use std::time::Duration;

use k8s_openapi::api::core::v1::Pod;
use kube::api::{Api, AttachParams};
use tokio::io::{AsyncRead, AsyncReadExt};

use super::{explain, Cluster, K8sManager};
use crate::logfiles::{self, FoundFile, Refusal};

/// What an exec printed, and the failure Kubernetes reported, if any.
pub struct Ran {
    pub stdout: String,
    pub stderr: String,
    pub failure: Option<String>,
}

impl Ran {
    fn refusal(&self) -> Option<Refusal> {
        self.failure.as_deref().and_then(logfiles::refusal).or_else(|| logfiles::refusal(&self.stderr))
    }
}

async fn drain<R: AsyncRead + Unpin>(r: Option<R>) -> Vec<u8> {
    let mut buf = Vec::new();
    if let Some(r) = r {
        let _ = r.take(2_000_000).read_to_end(&mut buf).await;
    }
    buf
}

fn params(container: Option<String>) -> AttachParams {
    let ap = AttachParams::default().stdin(false).stdout(true).stderr(true).tty(false);
    match container {
        Some(c) => ap.container(c),
        None => ap,
    }
}

impl Cluster {
    /// Run `cmd` in a pod's container and collect what it prints, up to `secs`.
    pub async fn exec_collect(&self, ns: &str, pod: &str, container: Option<String>, cmd: Vec<String>, secs: u64) -> Result<Ran, String> {
        let work = async {
            let api: Api<Pod> = Api::namespaced(self.client.clone(), ns);
            let mut p = api.exec(pod, cmd, &params(container)).await.map_err(explain)?;
            let status = p.take_status();
            let (out, err) = tokio::join!(drain(p.stdout()), drain(p.stderr()));
            let st = match status {
                Some(f) => f.await,
                None => None,
            };
            let failure = st.filter(|s| s.status.as_deref() == Some("Failure")).map(|s| s.message.unwrap_or_default());
            Ok(Ran { stdout: String::from_utf8_lossy(&out).into_owned(), stderr: String::from_utf8_lossy(&err).into_owned(), failure })
        };
        tokio::time::timeout(Duration::from_secs(secs), work).await.map_err(|_| format!("No answer within {secs} seconds"))?
    }

    /// The log files in a pod's container, written-to first.
    pub async fn find_log_files(&self, ns: &str, pod: &str, container: Option<String>) -> Result<Vec<FoundFile>, String> {
        let ran = self.exec_collect(ns, pod, container, vec!["sh".into(), "-c".into(), logfiles::DISCOVER_SCRIPT.into()], 30).await?;
        if ran.refusal() == Some(Refusal::NoTool) {
            return Err("This image has no shell, so Reach cannot list its files. Type the file's path instead.".into());
        }
        Ok(logfiles::parse_discovery(&ran.stdout))
    }
}

impl K8sManager {
    /// Follow files inside a pod's container into `out`, as `tail -F`.
    #[allow(clippy::too_many_arguments)]
    pub fn follow_files(
        &mut self,
        stream_id: String,
        cluster: Arc<Cluster>,
        ns: String,
        pod: String,
        container: Option<String>,
        paths: Vec<String>,
        lines: u32,
        out: tauri::ipc::Channel<String>,
    ) {
        if let Some(old) = self.streams.remove(&stream_id) {
            old.abort();
        }
        let task = tokio::spawn(async move {
            let mut probe = vec!["tail".to_string(), "-n".into(), "0".into(), "--".into()];
            probe.extend(paths.iter().cloned());
            match cluster.exec_collect(&ns, &pod, container.clone(), probe, 15).await {
                Ok(r) if r.refusal() == Some(Refusal::NoTool) => {
                    snapshot_with_cat(&cluster, &ns, &pod, container, &paths, lines, &out).await;
                    return;
                }
                Ok(_) => {}
                Err(e) => {
                    let _ = out.send(logfiles::note(&e));
                    return;
                }
            }
            let api: Api<Pod> = Api::namespaced(cluster.client.clone(), &ns);
            let mut p = match api.exec(&pod, logfiles::tail_command(&paths, lines), &params(container)).await {
                Ok(p) => p,
                Err(e) => {
                    let _ = out.send(logfiles::note(&explain(e)));
                    return;
                }
            };
            let pump = |r: Option<Box<dyn AsyncRead + Unpin + Send>>, out: tauri::ipc::Channel<String>| async move {
                let Some(mut r) = r else { return };
                let mut buf = vec![0u8; 16 * 1024];
                loop {
                    match r.read(&mut buf).await {
                        Ok(0) | Err(_) => return,
                        Ok(n) => {
                            if out.send(logfiles::data(String::from_utf8_lossy(&buf[..n]).into_owned())).is_err() {
                                return;
                            }
                        }
                    }
                }
            };
            let so = p.stdout().map(|r| Box::new(r) as Box<dyn AsyncRead + Unpin + Send>);
            let se = p.stderr().map(|r| Box::new(r) as Box<dyn AsyncRead + Unpin + Send>);
            tokio::join!(pump(so, out.clone()), pump(se, out.clone()));
            let _ = out.send(logfiles::note("The container's tail ended; the file is no longer followed."));
        });
        self.streams.insert(stream_id, task);
    }
}

/// Where the image has no `tail`: each file's last lines through `cat`.
async fn snapshot_with_cat(cluster: &Cluster, ns: &str, pod: &str, container: Option<String>, paths: &[String], lines: u32, out: &tauri::ipc::Channel<String>) {
    for p in paths {
        let ran = cluster.exec_collect(ns, pod, container.clone(), vec!["cat".into(), "--".into(), p.clone()], 60).await;
        let mut text = if paths.len() > 1 { format!("==> {p} <==\n") } else { String::new() };
        match ran {
            Ok(r) if r.refusal() == Some(Refusal::NoTool) => {
                let _ = out.send(logfiles::note(
                    "This image has neither tail nor cat, and Kubernetes has no file copy of its own, so its files cannot be read from here.",
                ));
                return;
            }
            Ok(r) if !r.stderr.trim().is_empty() && r.stdout.is_empty() => text.push_str(&format!("{}{}\n", logfiles::NOTE, r.stderr.trim())),
            Ok(r) => text.push_str(&logfiles::data(logfiles::last_lines(r.stdout.as_bytes(), lines as usize))),
            Err(e) => text.push_str(&format!("{}{e}\n", logfiles::NOTE)),
        }
        if out.send(text.replace('\n', "\r\n")).is_err() {
            return;
        }
    }
    let _ = out.send(logfiles::note("This image has no tail, so this is the file as it is now, not followed. Read it again to refresh."));
}
