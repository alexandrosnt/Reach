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

#[derive(Debug, Clone)]
pub enum AuthParams {
    Password(String),
    Key { path: String, passphrase: Option<String> },
    Agent,
}

/// Parameters for a single jump host in a proxy chain.
#[derive(Debug, Clone)]
pub struct JumpHostParams {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth: AuthParams,
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
    /// Keep intermediate jump host sessions alive for the lifetime of this connection.
    /// These are intentionally stored but never directly read — dropping them closes the tunnels.
    #[allow(dead_code)]
    jump_handles: Vec<SharedHandle>,
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

        let timeout_duration = std::time::Duration::from_secs(15);
        let connect_future = async {
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

            Ok((handle, channel))
        };

        let (handle, channel) = tokio::time::timeout(timeout_duration, connect_future)
            .await
            .map_err(|_| SshError::ConnectionFailed("Connection timed out".into()))??;

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
            jump_handles: Vec::new(),
        });

        Ok(info)
    }

    /// Connect to a target host through one or more jump hosts (ProxyJump).
    /// `jump_chain` is ordered outermost-first: connect to first hop, then tunnel through.
    pub async fn connect_via_jump(
        &mut self,
        id: &str,
        target_host: &str,
        target_port: u16,
        target_username: &str,
        target_auth: AuthParams,
        jump_chain: Vec<JumpHostParams>,
        cols: u16,
        rows: u16,
        app_handle: tauri::AppHandle,
    ) -> Result<ConnectionInfo, SshError> {
        tracing::info!(
            "SSH connecting to {}@{}:{} via {} jump host(s)",
            target_username, target_host, target_port, jump_chain.len()
        );

        let timeout_duration = std::time::Duration::from_secs(30);
        let connect_future = async {
            let mut jump_handles: Vec<SharedHandle> = Vec::new();

            // Step 1: Connect to the first jump host directly
            let first_jump = &jump_chain[0];
            let config = Arc::new(russh::client::Config::default());
            let handler = SshClientHandler;

            let mut current_handle = russh::client::connect(
                config,
                (first_jump.host.as_str(), first_jump.port),
                handler,
            )
            .await
            .map_err(|e| {
                SshError::ConnectionFailed(format!(
                    "Jump host {} connection failed: {}",
                    first_jump.host, e
                ))
            })?;

            // Authenticate on first jump host
            Self::authenticate_handle(&mut current_handle, &first_jump.username, &first_jump.auth)
                .await?;

            tracing::info!("Authenticated on jump host {}", first_jump.host);

            // Step 2: Chain through remaining jump hosts or tunnel to target
            if jump_chain.len() > 1 {
                let shared = Arc::new(tokio::sync::Mutex::new(current_handle));
                jump_handles.push(shared.clone());

                let mut prev_shared = shared;

                for i in 1..jump_chain.len() {
                    let next_jump = &jump_chain[i];

                    // Open direct-tcpip channel to next hop through current handle
                    let channel = {
                        let guard = prev_shared.lock().await;
                        guard
                            .channel_open_direct_tcpip(
                                &next_jump.host,
                                next_jump.port as u32,
                                "127.0.0.1",
                                0,
                            )
                            .await
                            .map_err(|e| {
                                SshError::ConnectionFailed(format!(
                                    "Failed to open tunnel to {}: {}",
                                    next_jump.host, e
                                ))
                            })?
                    };

                    let stream = channel.into_stream();
                    let config = Arc::new(russh::client::Config::default());
                    let handler = SshClientHandler;

                    let mut next_handle =
                        russh::client::connect_stream(config, stream, handler)
                            .await
                            .map_err(|e| {
                                SshError::ConnectionFailed(format!(
                                    "SSH over tunnel to {} failed: {}",
                                    next_jump.host, e
                                ))
                            })?;

                    Self::authenticate_handle(
                        &mut next_handle,
                        &next_jump.username,
                        &next_jump.auth,
                    )
                    .await?;

                    tracing::info!("Authenticated on jump host {}", next_jump.host);

                    let next_shared = Arc::new(tokio::sync::Mutex::new(next_handle));
                    jump_handles.push(next_shared.clone());
                    prev_shared = next_shared;
                }

                // Now open a tunnel from the last jump host to the target
                let channel = {
                    let guard = prev_shared.lock().await;
                    guard
                        .channel_open_direct_tcpip(
                            target_host,
                            target_port as u32,
                            "127.0.0.1",
                            0,
                        )
                        .await
                        .map_err(|e| {
                            SshError::ConnectionFailed(format!(
                                "Failed to open tunnel to target {}:{}: {}",
                                target_host, target_port, e
                            ))
                        })?
                };

                let stream = channel.into_stream();
                let config = Arc::new(russh::client::Config::default());
                let handler = SshClientHandler;

                let mut target_handle =
                    russh::client::connect_stream(config, stream, handler)
                        .await
                        .map_err(|e| {
                            SshError::ConnectionFailed(format!(
                                "SSH to target {}:{} via jump failed: {}",
                                target_host, target_port, e
                            ))
                        })?;

                Self::authenticate_handle(
                    &mut target_handle,
                    target_username,
                    &target_auth,
                )
                .await?;

                Ok((target_handle, jump_handles))
            } else {
                // Single jump host: tunnel directly to target
                let shared = Arc::new(tokio::sync::Mutex::new(current_handle));
                jump_handles.push(shared.clone());

                let channel = {
                    let guard = shared.lock().await;
                    guard
                        .channel_open_direct_tcpip(
                            target_host,
                            target_port as u32,
                            "127.0.0.1",
                            0,
                        )
                        .await
                        .map_err(|e| {
                            SshError::ConnectionFailed(format!(
                                "Failed to open tunnel to target {}:{}: {}",
                                target_host, target_port, e
                            ))
                        })?
                };

                let stream = channel.into_stream();
                let config = Arc::new(russh::client::Config::default());
                let handler = SshClientHandler;

                let mut target_handle =
                    russh::client::connect_stream(config, stream, handler)
                        .await
                        .map_err(|e| {
                            SshError::ConnectionFailed(format!(
                                "SSH to target {}:{} via jump failed: {}",
                                target_host, target_port, e
                            ))
                        })?;

                Self::authenticate_handle(
                    &mut target_handle,
                    target_username,
                    &target_auth,
                )
                .await?;

                Ok((target_handle, jump_handles))
            }
        };

        let (target_handle, jump_handles) =
            tokio::time::timeout(timeout_duration, connect_future)
                .await
                .map_err(|_| SshError::ConnectionFailed("Connection via jump timed out".into()))??;

        tracing::info!(
            "SSH authenticated for {}@{}:{} (via jump)",
            target_username, target_host, target_port
        );

        // Open session, request PTY and shell on target
        let channel = target_handle
            .channel_open_session()
            .await
            .map_err(|e| SshError::ChannelError(format!("Failed to open session: {}", e)))?;

        channel
            .request_pty(false, "xterm-256color", cols as u32, rows as u32, 0, 0, &[])
            .await
            .map_err(|e| SshError::ChannelError(format!("PTY request failed: {}", e)))?;

        channel
            .request_shell(false)
            .await
            .map_err(|e| SshError::ChannelError(format!("Shell request failed: {}", e)))?;

        tracing::info!(
            "SSH shell opened for {}@{}:{} (via jump)",
            target_username, target_host, target_port
        );

        // Inject color initialization (same as direct connect)
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
        channel
            .data(color_init.as_bytes())
            .await
            .map_err(|e| SshError::ChannelError(format!("Color init failed: {}", e)))?;

        let info = ConnectionInfo {
            id: id.to_string(),
            host: target_host.to_string(),
            port: target_port,
            username: target_username.to_string(),
        };

        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();

        let task_id = id.to_string();
        let task_handle = app_handle.clone();
        tokio::spawn(async move {
            ssh_session_task(channel, cmd_rx, task_id, task_handle).await;
        });

        let shared_handle = Arc::new(tokio::sync::Mutex::new(target_handle));

        self.connections.insert(
            id.to_string(),
            ActiveConnection {
                cmd_tx,
                info: info.clone(),
                handle: shared_handle,
                jump_handles,
            },
        );

        Ok(info)
    }

    /// Authenticate on a russh handle with the given auth params.
    async fn authenticate_handle(
        handle: &mut russh::client::Handle<SshClientHandler>,
        username: &str,
        auth: &AuthParams,
    ) -> Result<(), SshError> {
        let authenticated = match auth {
            AuthParams::Password(ref password) => handle
                .authenticate_password(username, password)
                .await
                .map_err(|e| SshError::ConnectionFailed(format!("Auth error: {}", e)))?,
            AuthParams::Key {
                ref path,
                ref passphrase,
            } => {
                let key = russh_keys::load_secret_key(path, passphrase.as_deref())
                    .map_err(|e| SshError::ConnectionFailed(format!("Key load error: {}", e)))?;
                handle
                    .authenticate_publickey(username, Arc::new(key))
                    .await
                    .map_err(|e| SshError::ConnectionFailed(format!("Auth error: {}", e)))?
            }
            AuthParams::Agent => {
                return Err(SshError::ConnectionFailed(
                    "Agent auth not yet implemented".into(),
                ));
            }
        };

        if !authenticated {
            return Err(SshError::AuthFailed);
        }

        Ok(())
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

/// Execute a command on an existing SSH connection and return (stdout, stderr, exit_code).
/// Unlike `exec_on_connection`, this captures stderr separately and returns the exit code.
pub async fn exec_on_connection_with_exit_code(
    handle: &SharedHandle,
    command: &str,
) -> Result<(String, String, i32), SshError> {
    let mut channel = {
        let guard = handle.lock().await;
        guard.channel_open_session().await
            .map_err(|e| SshError::ChannelError(format!("{}", e)))?
    };
    channel.exec(true, command).await
        .map_err(|e| SshError::ChannelError(format!("{}", e)))?;

    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut exit_code: i32 = -1;
    let mut got_eof = false;
    let mut got_exit = false;

    loop {
        let msg = tokio::time::timeout(
            std::time::Duration::from_secs(300),
            channel.wait(),
        ).await;

        match msg {
            Ok(Some(ChannelMsg::Data { ref data })) => {
                stdout.push_str(&String::from_utf8_lossy(data));
            }
            Ok(Some(ChannelMsg::ExtendedData { ref data, .. })) => {
                stderr.push_str(&String::from_utf8_lossy(data));
            }
            Ok(Some(ChannelMsg::Eof)) => {
                got_eof = true;
                if got_exit { break; }
            }
            Ok(Some(ChannelMsg::ExitStatus { exit_status })) => {
                exit_code = exit_status as i32;
                got_exit = true;
                if got_eof { break; }
            }
            Ok(None) | Err(_) => break,
            _ => {}
        }
    }

    Ok((stdout, stderr, exit_code))
}

/// Generic streaming output event used by all remote streaming commands.
#[derive(Debug, Clone, serde::Serialize)]
pub struct StreamingOutputEvent {
    pub run_id: String,
    pub stream: String,
    pub data: String,
}

/// Streaming variant of `exec_on_connection`.
/// Emits each chunk as a `{event_prefix}-{run_id}` Tauri event.
/// Returns the exit code (defaults to -1 if not received).
pub async fn exec_on_connection_streaming(
    handle: &SharedHandle,
    command: &str,
    run_id: &str,
    event_prefix: &str,
    app_handle: &tauri::AppHandle,
) -> Result<i32, SshError> {
    let mut channel = {
        let guard = handle.lock().await;
        guard.channel_open_session().await
            .map_err(|e| SshError::ChannelError(format!("{}", e)))?
    };
    channel.exec(true, command).await
        .map_err(|e| SshError::ChannelError(format!("{}", e)))?;

    let output_event = format!("{}-{}", event_prefix, run_id);
    let mut exit_code: i32 = -1;
    let mut got_eof = false;
    let mut got_exit = false;

    loop {
        let msg = tokio::time::timeout(
            std::time::Duration::from_secs(300),
            channel.wait(),
        ).await;

        match msg {
            Ok(Some(ChannelMsg::Data { ref data })) => {
                let text = String::from_utf8_lossy(data).to_string();
                let _ = app_handle.emit(
                    &output_event,
                    StreamingOutputEvent {
                        run_id: run_id.to_string(),
                        stream: "stdout".to_string(),
                        data: text,
                    },
                );
            }
            Ok(Some(ChannelMsg::ExtendedData { ref data, .. })) => {
                let text = String::from_utf8_lossy(data).to_string();
                let _ = app_handle.emit(
                    &output_event,
                    StreamingOutputEvent {
                        run_id: run_id.to_string(),
                        stream: "stderr".to_string(),
                        data: text,
                    },
                );
            }
            Ok(Some(ChannelMsg::Eof)) => {
                got_eof = true;
                if got_exit { break; }
            }
            Ok(Some(ChannelMsg::ExitStatus { exit_status })) => {
                exit_code = exit_status as i32;
                got_exit = true;
                if got_eof { break; }
            }
            Ok(None) | Err(_) => break,
            _ => {}
        }
    }

    Ok(exit_code)
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
