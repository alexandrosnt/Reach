use std::collections::HashMap;
use std::sync::Arc;
use async_trait::async_trait;
use russh::ChannelMsg;
use tauri::Emitter;
use thiserror::Error;
use tokio::sync::mpsc;

/// A shared, clonable wrapper around the russh Handle.
/// Handle is not Clone, so we wrap it in Arc<Mutex<>> for reuse.
pub type SharedHandle = Arc<tokio::sync::Mutex<russh::client::Handle<SshClientHandler>>>;

#[derive(Debug, Error)]
pub enum SshError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Authentication failed")]
    AuthFailed,
    #[error("Channel error: {0}")]
    ChannelError(String),
    #[error("Connection not found: {0}")]
    NotFound(String),
    #[error("Send error: {0}")]
    SendError(String),
}

enum SessionCommand {
    Data(Vec<u8>),
    Resize { cols: u32, rows: u32 },
    Close,
}

pub enum AuthParams {
    Password(String),
    Key { path: String, passphrase: Option<String> },
    Agent,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ConnectionInfo {
    pub id: String,
    pub host: String,
    pub port: u16,
    pub username: String,
}

struct ActiveConnection {
    cmd_tx: mpsc::UnboundedSender<SessionCommand>,
    info: ConnectionInfo,
    handle: SharedHandle,
}

pub struct SshManager {
    connections: HashMap<String, ActiveConnection>,
}

impl SshManager {
    pub fn new() -> Self {
        Self { connections: HashMap::new() }
    }

    pub async fn connect(
        &mut self,
        id: &str,
        host: &str,
        port: u16,
        username: &str,
        auth: AuthParams,
        cols: u16,
        rows: u16,
        app_handle: tauri::AppHandle,
    ) -> Result<ConnectionInfo, SshError> {
        tracing::info!("SSH connecting to {}@{}:{}", username, host, port);

        let config = Arc::new(russh::client::Config::default());
        let handler = SshClientHandler;

        let mut handle = russh::client::connect(config, (host, port), handler)
            .await
            .map_err(|e| SshError::ConnectionFailed(format!("{}", e)))?;

        // Authenticate
        let authenticated = match auth {
            AuthParams::Password(ref password) => {
                handle.authenticate_password(username, password).await
                    .map_err(|e| SshError::ConnectionFailed(format!("Auth error: {}", e)))?
            }
            AuthParams::Key { ref path, ref passphrase } => {
                let key = russh_keys::load_secret_key(path, passphrase.as_deref())
                    .map_err(|e| SshError::ConnectionFailed(format!("Key load error: {}", e)))?;
                handle.authenticate_publickey(username, Arc::new(key)).await
                    .map_err(|e| SshError::ConnectionFailed(format!("Auth error: {}", e)))?
            }
            AuthParams::Agent => {
                return Err(SshError::ConnectionFailed("Agent auth not yet implemented".into()));
            }
        };

        if !authenticated {
            return Err(SshError::AuthFailed);
        }

        tracing::info!("SSH authenticated for {}@{}:{}", username, host, port);

        let channel = handle.channel_open_session().await
            .map_err(|e| SshError::ChannelError(format!("Failed to open session: {}", e)))?;

        channel.request_pty(false, "xterm-256color", cols as u32, rows as u32, 0, 0, &[]).await
            .map_err(|e| SshError::ChannelError(format!("PTY request failed: {}", e)))?;

        channel.request_shell(false).await
            .map_err(|e| SshError::ChannelError(format!("Shell request failed: {}", e)))?;

        tracing::info!("SSH shell opened for {}@{}:{}", username, host, port);

        // Inject color initialization for remote shells that may lack color config
        // (e.g. root on Debian/Ubuntu ships with a minimal .bashrc without colors).
        // stty -echo hides the commands; clear wipes any artifacts afterward.
        let color_init = concat!(
            r#"stty -echo; export COLORTERM=truecolor; "#,
            r#"[ -z "$LS_COLORS" ] && eval "$(dircolors -b 2>/dev/null)"; "#,
            r#"alias ls='ls --color=auto' 2>/dev/null; "#,
            r#"alias grep='grep --color=auto' 2>/dev/null; "#,
            r#"alias diff='diff --color=auto' 2>/dev/null; "#,
            r#"if [ -n "$BASH" ]; then "#,
            r#"case "$PS1" in *033*|*\\e\[*) ;; *) "#,
            r#"_c=32; [ "${EUID:-$(id -u)}" = "0" ] && _c=31; "#,
            r#"PS1="\\[\\033[01;${_c}m\\]\\u@\\h\\[\\033[00m\\]:\\[\\033[01;34m\\]\\w\\[\\033[00m\\]\\$ "; "#,
            r#"unset _c; esac; fi; stty echo; clear"#,
            "\n"
        );
        channel.data(color_init.as_bytes()).await
            .map_err(|e| SshError::ChannelError(format!("Color init failed: {}", e)))?;

        let info = ConnectionInfo {
            id: id.to_string(),
            host: host.to_string(),
            port,
            username: username.to_string(),
        };

        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();

        let task_id = id.to_string();
        let task_handle = app_handle.clone();
        tokio::spawn(async move {
            ssh_session_task(channel, cmd_rx, task_id, task_handle).await;
        });

        let shared_handle = Arc::new(tokio::sync::Mutex::new(handle));

        self.connections.insert(id.to_string(), ActiveConnection {
            cmd_tx,
            info: info.clone(),
            handle: shared_handle,
        });

        Ok(info)
    }

    pub fn send_data(&self, id: &str, data: &[u8]) -> Result<(), SshError> {
        let conn = self.connections.get(id)
            .ok_or_else(|| SshError::NotFound(id.to_string()))?;
        conn.cmd_tx.send(SessionCommand::Data(data.to_vec()))
            .map_err(|e| SshError::SendError(format!("{}", e)))
    }

    pub fn resize(&self, id: &str, cols: u16, rows: u16) -> Result<(), SshError> {
        let conn = self.connections.get(id)
            .ok_or_else(|| SshError::NotFound(id.to_string()))?;
        conn.cmd_tx.send(SessionCommand::Resize { cols: cols as u32, rows: rows as u32 })
            .map_err(|e| SshError::SendError(format!("{}", e)))
    }

    pub fn disconnect(&mut self, id: &str) -> Result<(), SshError> {
        let conn = self.connections.remove(id)
            .ok_or_else(|| SshError::NotFound(id.to_string()))?;
        let _ = conn.cmd_tx.send(SessionCommand::Close);
        tracing::info!("SSH disconnected: {}", id);
        Ok(())
    }

    pub fn list_connections(&self) -> Vec<ConnectionInfo> {
        self.connections.values().map(|c| c.info.clone()).collect()
    }

    pub fn is_connected(&self, id: &str) -> bool {
        self.connections.contains_key(id)
    }

    pub fn get_handle(&self, id: &str) -> Result<SharedHandle, SshError> {
        self.connections.get(id)
            .map(|c| c.handle.clone())
            .ok_or_else(|| SshError::NotFound(id.to_string()))
    }
}

impl Default for SshManager {
    fn default() -> Self { Self::new() }
}

pub async fn exec_on_connection(
    handle: &SharedHandle,
    command: &str,
) -> Result<String, SshError> {
    let mut channel = {
        let guard = handle.lock().await;
        guard.channel_open_session().await
            .map_err(|e| SshError::ChannelError(format!("{}", e)))?
    };
    channel.exec(true, command).await
        .map_err(|e| SshError::ChannelError(format!("{}", e)))?;
    let mut output = String::new();
    let mut got_eof = false;
    let mut got_exit = false;
    loop {
        // Timeout to avoid hanging forever
        let msg = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            channel.wait(),
        ).await;

        match msg {
            Ok(Some(ChannelMsg::Data { ref data })) => {
                output.push_str(&String::from_utf8_lossy(data));
            }
            Ok(Some(ChannelMsg::ExtendedData { .. })) => {
                // stderr — skip
            }
            Ok(Some(ChannelMsg::Eof)) => {
                got_eof = true;
                if got_exit { break; }
            }
            Ok(Some(ChannelMsg::ExitStatus { .. })) => {
                got_exit = true;
                if got_eof { break; }
            }
            Ok(None) | Err(_) => break, // channel closed or timeout
            _ => {
                // WindowAdjusted, etc.
            }
        }
    }
    Ok(output)
}

pub struct SshClientHandler;

#[async_trait]
impl russh::client::Handler for SshClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh_keys::key::PublicKey,
    ) -> Result<bool, Self::Error> {
        // Accept all server keys for now
        // TODO: known_hosts verification
        Ok(true)
    }
}

async fn ssh_session_task(
    mut channel: russh::Channel<russh::client::Msg>,
    mut cmd_rx: mpsc::UnboundedReceiver<SessionCommand>,
    connection_id: String,
    app_handle: tauri::AppHandle,
) {
    let data_event = format!("ssh-data-{}", connection_id);
    let exit_event = format!("ssh-exit-{}", connection_id);

    loop {
        tokio::select! {
            msg = channel.wait() => {
                match msg {
                    Some(ChannelMsg::Data { ref data }) => {
                        let payload = String::from_utf8_lossy(data).to_string();
                        if let Err(e) = app_handle.emit(&data_event, &payload) {
                            tracing::error!("Failed to emit '{}': {}", data_event, e);
                            break;
                        }
                    }
                    Some(ChannelMsg::ExtendedData { ref data, .. }) => {
                        let payload = String::from_utf8_lossy(data).to_string();
                        if let Err(e) = app_handle.emit(&data_event, &payload) {
                            tracing::error!("Failed to emit '{}': {}", data_event, e);
                            break;
                        }
                    }
                    Some(ChannelMsg::ExitStatus { exit_status }) => {
                        tracing::info!("SSH '{}' exited with status {}", connection_id, exit_status);
                        let _ = app_handle.emit(&exit_event, exit_status);
                        break;
                    }
                    Some(ChannelMsg::Eof) => {
                        tracing::info!("SSH '{}' received EOF", connection_id);
                        break;
                    }
                    None => {
                        tracing::info!("SSH '{}' channel closed", connection_id);
                        break;
                    }
                    _ => {}
                }
            }
            cmd = cmd_rx.recv() => {
                match cmd {
                    Some(SessionCommand::Data(data)) => {
                        if let Err(e) = channel.data(&data[..]).await {
                            tracing::error!("SSH '{}' write error: {}", connection_id, e);
                            break;
                        }
                    }
                    Some(SessionCommand::Resize { cols, rows }) => {
                        if let Err(e) = channel.window_change(cols, rows, 0, 0).await {
                            tracing::error!("SSH '{}' resize error: {}", connection_id, e);
                        }
                    }
                    Some(SessionCommand::Close) | None => {
                        tracing::info!("SSH '{}' closing", connection_id);
                        let _ = channel.close().await;
                        break;
                    }
                }
            }
        }
    }

    if let Err(e) = app_handle.emit(&exit_event, ()) {
        tracing::error!("Failed to emit '{}': {}", exit_event, e);
    }
    tracing::info!("SSH '{}' session task exiting", connection_id);
}
