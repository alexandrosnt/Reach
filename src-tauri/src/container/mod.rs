//! Docker, Podman and Compose on a server reached through SSH, or on this
//! computer.
//!
//! Everything goes through the engine's own API (see [`transport`]), except
//! Compose, which has no API: its commands run on the server, as Portainer
//! and Dockge run them. Destructive actions are confirmed in the UI, which
//! names what they affect; the server's own permissions decide what the user
//! may do at all.

pub mod compose;
pub mod files;
pub mod transport;

use std::collections::HashMap;
use std::sync::Arc;

use bollard::models::ContainerSummary;
use bollard::query_parameters as q;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::task::JoinHandle;

use crate::ssh::client::{HeadlessConnection, SharedHandle};
use transport::{Dial, SshTransport};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Engine {
    Docker,
    Podman,
}

impl Engine {
    pub fn cli(self) -> &'static str {
        match self {
            Engine::Docker => "docker",
            Engine::Podman => "podman",
        }
    }
}

/// Where an engine command runs: on the server over SSH, or here.
#[derive(Clone)]
pub enum Shell {
    Ssh(SharedHandle),
    #[cfg_attr(target_os = "android", allow(dead_code))]
    Local,
}

/// What a command printed, and how it ended.
#[derive(Debug, Default)]
pub struct Output {
    pub code: Option<u32>,
    pub stdout: String,
    pub stderr: String,
}

impl Output {
    pub fn ok(&self) -> bool {
        self.code == Some(0)
    }

    /// The command's own words when it failed, for the user.
    pub fn failure(&self, what: &str) -> String {
        let said = if self.stderr.trim().is_empty() { self.stdout.trim() } else { self.stderr.trim() };
        if said.is_empty() {
            format!("{what} failed (exit {})", self.code.map_or("unknown".into(), |c| c.to_string()))
        } else {
            format!("{what} failed: {said}")
        }
    }
}

/// Quote one word for a POSIX shell. Every value that reaches a command line
/// on the server (names, paths) goes through this.
pub fn sh_quote(s: &str) -> String {
    if !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_./=:@%+,".contains(&b)) {
        return s.to_string();
    }
    format!("'{}'", s.replace('\'', r"'\''"))
}

impl Shell {
    /// Run `cmd` (a shell command line) and wait for it, up to `secs`.
    pub async fn run(&self, cmd: &str, secs: u64) -> Result<Output, String> {
        self.run_with_input(cmd, None, secs).await
    }

    /// As [`Shell::run`], with `input` on the command's stdin. Secrets go
    /// this way, never on the command line, where `ps` shows them to every
    /// user on the machine.
    pub async fn run_with_input(&self, cmd: &str, input: Option<&[u8]>, secs: u64) -> Result<Output, String> {
        let work = async {
            match self {
                Shell::Ssh(h) => run_ssh(h, cmd, input).await,
                Shell::Local => run_local(cmd, input).await,
            }
        };
        tokio::time::timeout(std::time::Duration::from_secs(secs), work)
            .await
            .map_err(|_| format!("No answer within {secs} seconds"))?
    }
}

async fn run_ssh(h: &SharedHandle, cmd: &str, input: Option<&[u8]>) -> Result<Output, String> {
    let mut ch = h.lock().await.channel_open_session().await.map_err(|e| format!("Could not open an SSH channel: {e}"))?;
    ch.exec(true, cmd).await.map_err(|e| e.to_string())?;
    if let Some(bytes) = input {
        ch.data(bytes).await.map_err(|e| e.to_string())?;
    }
    // End of input either way, so a command that reads stdin finishes.
    ch.eof().await.map_err(|e| e.to_string())?;
    let (mut out, mut err, mut code) = (Vec::new(), Vec::new(), None);
    while let Some(msg) = ch.wait().await {
        match msg {
            russh::ChannelMsg::Data { data } => out.extend_from_slice(&data),
            russh::ChannelMsg::ExtendedData { data, .. } => err.extend_from_slice(&data),
            russh::ChannelMsg::ExitStatus { exit_status } => code = Some(exit_status),
            _ => {}
        }
    }
    Ok(Output { code, stdout: String::from_utf8_lossy(&out).into(), stderr: String::from_utf8_lossy(&err).into() })
}

#[cfg(not(target_os = "android"))]
async fn run_local(cmd: &str, input: Option<&[u8]>) -> Result<Output, String> {
    use tokio::io::AsyncWriteExt;
    #[cfg(windows)]
    let mut c = {
        let mut c = tokio::process::Command::new("cmd");
        c.args(["/C", cmd]);
        // No console window flashing up behind Reach.
        c.creation_flags(0x0800_0000);
        c
    };
    #[cfg(not(windows))]
    let mut c = {
        let mut c = tokio::process::Command::new("sh");
        c.args(["-c", cmd]);
        c
    };
    c.stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    let mut child = c.spawn().map_err(|e| e.to_string())?;
    if let Some(mut stdin) = child.stdin.take() {
        if let Some(bytes) = input {
            stdin.write_all(bytes).await.map_err(|e| e.to_string())?;
        }
        // Dropped here: end of input.
    }
    let o = child.wait_with_output().await.map_err(|e| e.to_string())?;
    Ok(Output {
        code: o.status.code().map(|c| c as u32),
        stdout: String::from_utf8_lossy(&o.stdout).into(),
        stderr: String::from_utf8_lossy(&o.stderr).into(),
    })
}

#[cfg(target_os = "android")]
async fn run_local(_cmd: &str, _input: Option<&[u8]>) -> Result<Output, String> {
    Err("A phone has no container engine of its own; choose a server.".into())
}

/// A connected engine.
pub struct Host {
    pub docker: bollard::Docker,
    pub engine: Engine,
    pub shell: Shell,
    pub info: HostInfo,
    /// Set from the saved host; every change refuses while it is on.
    pub read_only: bool,
}

/// How much of a log to show, as `docker logs --tail/--since/-t` choose it.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogView {
    pub tail: u32,
    #[serde(default)]
    pub since_secs: Option<u32>,
    #[serde(default)]
    pub timestamps: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostInfo {
    pub engine: Engine,
    pub version: String,
    pub api_version: String,
    pub os: String,
    pub arch: String,
    /// Whether `<engine> compose` works on this host.
    pub compose: bool,
}

/// Connect to `engine` through `ssh`, or on this computer when `None`.
///
/// Before the API, the engine's own CLI is asked for its version: when the
/// user may not use the engine, or it is not installed, that prints the
/// clearest message there is (permission denied on the socket, command not
/// found), and the user gets it word for word.
pub(crate) async fn open(engine: Engine, ssh: Option<(SharedHandle, Option<HeadlessConnection>)>) -> Result<Host, String> {
    let (shell, keep) = match ssh {
        Some((h, keep)) => (Shell::Ssh(h), keep),
        None => (Shell::Local, None),
    };
    let check = shell.run(&format!("{} version --format '{{{{.Server.Version}}}}'", engine.cli()), 30).await?;
    if !check.ok() {
        return Err(check.failure(&format!("{} on this host", engine.cli())));
    }

    let docker = match (&shell, engine) {
        (Shell::Ssh(h), Engine::Docker) => {
            let keep = keep.map(|k| Box::new(k) as Box<dyn std::any::Any + Send + Sync>);
            SshTransport::new(h.clone(), Dial::Exec("docker system dial-stdio".into()), keep).docker()?
        }
        (Shell::Ssh(h), Engine::Podman) => {
            let path = podman_socket(&shell).await?;
            let keep = keep.map(|k| Box::new(k) as Box<dyn std::any::Any + Send + Sync>);
            SshTransport::new(h.clone(), Dial::Socket(path), keep).docker()?
        }
        (Shell::Local, Engine::Docker) => local_docker()?,
        (Shell::Local, Engine::Podman) => local_podman(&shell).await?,
    };
    // An older engine answers only up to its own API version.
    let docker = docker.negotiate_version().await.map_err(|e| format!("The engine did not answer: {e}"))?;
    let v = docker.version().await.map_err(|e| e.to_string())?;
    let compose = shell
        .run(&format!("{} compose version", engine.cli()), 30)
        .await
        .map(|o| o.ok())
        .unwrap_or(false);
    let info = HostInfo {
        engine,
        version: v.version.unwrap_or_default(),
        api_version: v.api_version.unwrap_or_default(),
        os: v.os.unwrap_or_default(),
        arch: v.arch.unwrap_or_default(),
        compose,
    };
    Ok(Host { docker, engine, shell, info, read_only: false })
}

/// Podman's API socket, as `podman system connection` finds it.
async fn podman_socket(shell: &Shell) -> Result<String, String> {
    let o = shell.run("podman info --format '{{.Host.RemoteSocket.Path}}'", 30).await?;
    if !o.ok() {
        return Err(o.failure("podman info"));
    }
    let path = o.stdout.trim().to_string();
    if path.is_empty() {
        return Err("Podman reports no API socket".into());
    }
    // `RemoteSocket.Exists` is not to be trusted: measured on Podman 5.4,
    // it said true for a user whose service had never run. The socket file
    // itself is the answer. (A Windows named pipe is not a file; it is left
    // to the connection to find out.)
    if path.starts_with('/') {
        let there = shell.run(&format!("test -S {}", sh_quote(&path)), 30).await?;
        if !there.ok() {
            return Err(format!(
                "Podman's API socket ({path}) is not running. On the server: systemctl --user enable --now podman.socket"
            ));
        }
    }
    Ok(path)
}

#[cfg(not(target_os = "android"))]
fn local_docker() -> Result<bollard::Docker, String> {
    bollard::Docker::connect_with_local_defaults().map_err(|e| e.to_string())
}

#[cfg(target_os = "android")]
fn local_docker() -> Result<bollard::Docker, String> {
    Err("A phone has no container engine of its own; choose a server.".into())
}

#[cfg(not(target_os = "android"))]
async fn local_podman(shell: &Shell) -> Result<bollard::Docker, String> {
    let path = podman_socket(shell).await?;
    let host = if path.starts_with('/') { format!("unix://{path}") } else { path };
    bollard::Docker::connect_with_host(&host).map_err(|e| e.to_string())
}

#[cfg(target_os = "android")]
async fn local_podman(_shell: &Shell) -> Result<bollard::Docker, String> {
    Err("A phone has no container engine of its own; choose a server.".into())
}

// ---------------------------------------------------------------------------
// What the pages show

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ContainerRow {
    pub id: String,
    pub name: String,
    pub image: String,
    pub state: String,
    pub status: String,
    pub created: i64,
    pub ports: Vec<String>,
    pub project: Option<String>,
    pub service: Option<String>,
}

pub fn container_row(c: ContainerSummary) -> ContainerRow {
    let labels = c.labels.unwrap_or_default();
    let mut ports: Vec<String> = c
        .ports
        .unwrap_or_default()
        .into_iter()
        .map(|p| {
            let proto = p.typ.map(|t| t.to_string()).unwrap_or_else(|| "tcp".into());
            match p.public_port {
                Some(pub_port) => format!("{}:{}->{}/{}", p.ip.unwrap_or_default(), pub_port, p.private_port, proto),
                None => format!("{}/{}", p.private_port, proto),
            }
        })
        .collect();
    ports.sort();
    ports.dedup();
    ContainerRow {
        id: c.id.unwrap_or_default(),
        name: c.names.unwrap_or_default().first().map(|n| n.trim_start_matches('/').to_string()).unwrap_or_default(),
        image: c.image.unwrap_or_default(),
        state: c.state.map(|s| s.to_string()).unwrap_or_default(),
        status: c.status.unwrap_or_default(),
        created: c.created.unwrap_or_default(),
        ports,
        project: labels.get("com.docker.compose.project").cloned(),
        service: labels.get("com.docker.compose.service").cloned(),
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageRow {
    pub id: String,
    pub tags: Vec<String>,
    pub size: i64,
    pub created: i64,
    pub containers: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeRow {
    pub name: String,
    pub driver: String,
    pub mountpoint: String,
    pub created: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkRow {
    pub id: String,
    pub name: String,
    pub driver: String,
    pub scope: String,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Start,
    Stop,
    Restart,
    Pause,
    Unpause,
    Kill,
    Remove,
}

impl Host {
    /// Refuse a change on a read-only host. The backend checks this, so a
    /// read-only host stays read-only whatever the page does.
    pub fn writable(&self) -> Result<(), String> {
        if self.read_only {
            Err("This host is read-only. Turn that off in its settings to make changes.".into())
        } else {
            Ok(())
        }
    }

    pub async fn containers(&self) -> Result<Vec<ContainerRow>, String> {
        let list = self
            .docker
            .list_containers(Some(q::ListContainersOptions { all: true, ..Default::default() }))
            .await
            .map_err(|e| e.to_string())?;
        Ok(list.into_iter().map(container_row).collect())
    }

    pub async fn images(&self) -> Result<Vec<ImageRow>, String> {
        let list = self.docker.list_images(Some(q::ListImagesOptions { all: false, ..Default::default() })).await.map_err(|e| e.to_string())?;
        Ok(list
            .into_iter()
            .map(|i| ImageRow {
                id: i.id,
                tags: i.repo_tags.into_iter().filter(|t| t != "<none>:<none>").collect(),
                size: i.size,
                created: i.created,
                containers: i.containers,
            })
            .collect())
    }

    pub async fn volumes(&self) -> Result<Vec<VolumeRow>, String> {
        let list = self.docker.list_volumes(None::<q::ListVolumesOptions>).await.map_err(|e| e.to_string())?;
        Ok(list
            .volumes
            .unwrap_or_default()
            .into_iter()
            .map(|v| VolumeRow { name: v.name, driver: v.driver, mountpoint: v.mountpoint, created: v.created_at.unwrap_or_default() })
            .collect())
    }

    pub async fn networks(&self) -> Result<Vec<NetworkRow>, String> {
        let list = self.docker.list_networks(None::<q::ListNetworksOptions>).await.map_err(|e| e.to_string())?;
        Ok(list
            .into_iter()
            .map(|n| NetworkRow {
                id: n.id.unwrap_or_default(),
                name: n.name.unwrap_or_default(),
                driver: n.driver.unwrap_or_default(),
                scope: n.scope.unwrap_or_default(),
            })
            .collect())
    }

    pub async fn act(&self, id: &str, action: Action) -> Result<(), String> {
        self.writable()?;
        let d = &self.docker;
        let r = match action {
            Action::Start => d.start_container(id, None::<q::StartContainerOptions>).await,
            Action::Stop => d.stop_container(id, None::<q::StopContainerOptions>).await,
            Action::Restart => d.restart_container(id, None::<q::RestartContainerOptions>).await,
            Action::Pause => d.pause_container(id).await,
            Action::Unpause => d.unpause_container(id).await,
            Action::Kill => d.kill_container(id, None::<q::KillContainerOptions>).await,
            // Anonymous volumes stay: removing data is a separate, named step.
            Action::Remove => d.remove_container(id, Some(q::RemoveContainerOptions { force: true, v: false, link: false })).await,
        };
        r.map_err(|e| e.to_string())
    }

    pub async fn remove_image(&self, id: &str) -> Result<(), String> {
        self.writable()?;
        self.docker
            .remove_image(id, Some(q::RemoveImageOptions { force: false, noprune: false, platforms: None }), None)
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    pub async fn remove_volume(&self, name: &str) -> Result<(), String> {
        self.writable()?;
        self.docker.remove_volume(name, Some(q::RemoveVolumeOptions { force: false })).await.map_err(|e| e.to_string())
    }

    pub async fn remove_network(&self, id: &str) -> Result<(), String> {
        self.writable()?;
        self.docker.remove_network(id).await.map_err(|e| e.to_string())
    }

    /// The engine's full JSON for one object, as `docker inspect` shows it.
    pub async fn inspect(&self, kind: &str, id: &str) -> Result<String, String> {
        let value = match kind {
            "container" => serde_json::to_value(self.docker.inspect_container(id, None::<q::InspectContainerOptions>).await.map_err(|e| e.to_string())?),
            "image" => serde_json::to_value(self.docker.inspect_image(id).await.map_err(|e| e.to_string())?),
            "volume" => serde_json::to_value(self.docker.inspect_volume(id).await.map_err(|e| e.to_string())?),
            "network" => serde_json::to_value(self.docker.inspect_network(id, None::<q::InspectNetworkOptions>).await.map_err(|e| e.to_string())?),
            other => return Err(format!("Unknown kind {other}")),
        }
        .map_err(|e| e.to_string())?;
        serde_json::to_string_pretty(&value).map_err(|e| e.to_string())
    }
}

/// Open engines, keyed by the host they are on, and the log streams running.
#[derive(Default)]
pub struct ContainerManager {
    hosts: HashMap<String, Arc<Host>>,
    streams: HashMap<String, JoinHandle<()>>,
}

impl ContainerManager {
    pub fn get(&self, key: &str) -> Result<Arc<Host>, String> {
        self.hosts.get(key).cloned().ok_or_else(|| "Not connected to that host; open it again.".to_string())
    }

    pub fn insert(&mut self, key: String, host: Host) -> HostInfo {
        let info = host.info.clone();
        self.hosts.insert(key, Arc::new(host));
        info
    }

    pub fn close(&mut self, key: &str) {
        self.hosts.remove(key);
    }

    pub fn close_all(&mut self) {
        self.hosts.clear();
        for (_, t) in self.streams.drain() {
            t.abort();
        }
    }

    /// Follow a container's log into `out` until stopped or the container
    /// ends. `view.tail` lines of history come first, or only those newer
    /// than `view.since_secs` ago, as `docker logs --since`.
    pub fn follow_logs(&mut self, stream_id: String, host: Arc<Host>, id: String, view: LogView, out: tauri::ipc::Channel<String>) {
        if let Some(old) = self.streams.remove(&stream_id) {
            old.abort();
        }
        let task = tokio::spawn(async move {
            let since = view
                .since_secs
                .map(|s| (chrono::Utc::now().timestamp() - s as i64).clamp(0, i32::MAX as i64) as i32)
                .unwrap_or(0);
            let opts = q::LogsOptions {
                follow: true,
                stdout: true,
                stderr: true,
                timestamps: view.timestamps,
                // With --since, everything since then: a count would cut it.
                tail: if view.since_secs.is_some() { "all".into() } else { view.tail.to_string() },
                since,
                ..Default::default()
            };
            let mut s = host.docker.logs(&id, Some(opts));
            while let Some(item) = s.next().await {
                match item {
                    Ok(chunk) => {
                        if out.send(crate::logfiles::data(chunk.to_string())).is_err() {
                            return;
                        }
                    }
                    Err(e) => {
                        let _ = out.send(crate::logfiles::note(&e.to_string()));
                        return;
                    }
                }
            }
            // The engine ends a followed log when the container stops; say
            // so, or the pane looks like it is still waiting.
            let _ = out.send(crate::logfiles::note("The container is not running; nothing more will come."));
        });
        self.streams.insert(stream_id, task);
    }

    pub fn stop_stream(&mut self, stream_id: &str) {
        if let Some(t) = self.streams.remove(stream_id) {
            t.abort();
        }
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

#[cfg(test)]
#[path = "live_tests.rs"]
mod live_tests;
