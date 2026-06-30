use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use russh::ChannelMsg;
use tauri::Emitter;
use thiserror::Error;
use tokio::sync::mpsc;

use crate::state::ProxyConfig;

/// Expand `~` and `~/` to the user's home directory. Cross-platform: works
/// on Windows (resolves to %USERPROFILE%), macOS, and Linux. Leaves absolute
/// paths and paths without leading `~` unchanged.
pub(crate) fn expand_tilde(path: &str) -> PathBuf {
    let trimmed = path.trim();
    if trimmed == "~" {
        return dirs::home_dir().unwrap_or_else(|| PathBuf::from(trimmed));
    }
    if let Some(rest) = trimmed.strip_prefix("~/").or_else(|| trimmed.strip_prefix("~\\")) {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(trimmed)
}

/// POSIX color/prompt initialization injected after login. Safe for bash, zsh,
/// sh, dash, etc. It sets truecolor + `ls`/`grep` color aliases and, for bash
/// without a colored prompt, a sensible `PS1`. `stty -echo`/`clear` hide it.
/// NOTE: this is bash/POSIX syntax — it is NOT valid in fish, which is why the
/// init is selected per shell family (see `shell_init`).
///
/// Hardened for cross-platform edge cases:
/// - `dircolors` is guarded with `command -v` (absent on macOS/BSD).
/// - `ls` color flag is detected: GNU `--color=auto` vs BSD `-G`, so the alias
///   doesn't break `ls` on macOS/BSD where `--color` is an unknown option.
/// - `stty`/`clear` are tolerated-if-missing so a minimal box doesn't error.
const POSIX_COLOR_INIT: &str = concat!(
    r#"stty -echo 2>/dev/null; export COLORTERM=truecolor; "#,
    r#"[ -z "$LS_COLORS" ] && command -v dircolors >/dev/null 2>&1 && eval "$(dircolors -b 2>/dev/null)"; "#,
    r#"if ls --color=auto >/dev/null 2>&1; then alias ls='ls --color=auto'; "#,
    r#"elif ls -G >/dev/null 2>&1; then alias ls='ls -G'; fi; "#,
    r#"alias grep='grep --color=auto' 2>/dev/null; "#,
    r#"alias diff='diff --color=auto' 2>/dev/null; "#,
    r#"if [ -n "$BASH" ]; then "#,
    r#"case "$PS1" in *033*|*\\e\[*) ;; *) "#,
    r#"_c=32; [ "${EUID:-$(id -u)}" = "0" ] && _c=31; "#,
    r#"PS1="\\[\\033[01;${_c}m\\]\\u@\\h\\[\\033[00m\\]:\\[\\033[01;34m\\]\\w\\[\\033[00m\\]\\$ "; "#,
    r#"unset _c; esac; fi; stty echo 2>/dev/null; clear 2>/dev/null"#,
    "\n"
);

/// Shell families we tailor the post-login init for.
enum ShellFamily {
    /// bash / zsh / sh / dash / ksh … — gets `POSIX_COLOR_INIT`.
    Posix,
    /// fish — gets a minimal fish-native init (its prompt/colors are good by default).
    Fish,
    /// Anything we don't recognize — inject nothing rather than guess its syntax.
    Other,
}

/// Classify a configured login-shell command (e.g. `"fish"`, `"/usr/bin/zsh -l"`).
/// `None`/empty means "use the account's default shell", which we treat as POSIX
/// to preserve the long-standing behavior for the common bash/zsh case.
fn shell_family(shell: Option<&str>) -> ShellFamily {
    let Some(s) = shell.map(str::trim).filter(|s| !s.is_empty()) else {
        return ShellFamily::Posix;
    };
    let prog = s.split_whitespace().next().unwrap_or("");
    let base = prog.rsplit(['/', '\\']).next().unwrap_or(prog).to_ascii_lowercase();
    match base.as_str() {
        "fish" => ShellFamily::Fish,
        "bash" | "sh" | "zsh" | "dash" | "ash" | "ksh" | "mksh" | "busybox" => ShellFamily::Posix,
        _ => ShellFamily::Other,
    }
}

/// The post-login init to inject for the given shell, or `None` to inject nothing.
fn shell_init(shell: Option<&str>) -> Option<String> {
    match shell_family(shell) {
        ShellFamily::Posix => Some(POSIX_COLOR_INIT.to_string()),
        // Valid fish: avoids the bash-isms (`export`, `$(...)`, `if…then…fi`)
        // that make fish throw a syntax error on every connect.
        ShellFamily::Fish => Some("set -gx COLORTERM truecolor; clear\n".to_string()),
        ShellFamily::Other => None,
    }
}

/// Request a PTY and start the interactive shell on `channel`. When `shell` is
/// set, `exec` it as the login shell instead of the account's default; a bare
/// program name (e.g. `fish`) gets a `-l` login flag, while a value with flags
/// (e.g. `fish -l`) is run verbatim. When `shell` is empty, request the default
/// login shell exactly as before.
async fn open_interactive_shell(
    channel: &russh::Channel<russh::client::Msg>,
    cols: u16,
    rows: u16,
    shell: Option<&str>,
) -> Result<(), SshError> {
    channel
        .request_pty(false, "xterm-256color", cols as u32, rows as u32, 0, 0, &[])
        .await
        .map_err(|e| SshError::ChannelError(format!("PTY request failed: {}", e)))?;

    match shell.map(str::trim).filter(|s| !s.is_empty()) {
        Some(cmd) => {
            let full = if cmd.split_whitespace().nth(1).is_some() {
                format!("exec {}", cmd)
            } else {
                format!("exec {} -l", cmd)
            };
            channel
                .exec(false, full.as_bytes())
                .await
                .map_err(|e| SshError::ChannelError(format!("Shell exec failed: {}", e)))?;
        }
        None => {
            channel
                .request_shell(false)
                .await
                .map_err(|e| SshError::ChannelError(format!("Shell request failed: {}", e)))?;
        }
    }
    Ok(())
}

/// Turn an opaque `load_secret_key` failure into a message that tells the user
/// what's actually wrong. The most common mistake is selecting an OpenSSH
/// *public* key (`id_ed25519.pub`) where the *private* key is required — russh
/// reports that as a generic parse error (the public key's spaces look like a
/// formatting problem), so we classify the file ourselves and point at the fix.
fn describe_key_load_error(
    raw_path: &str,
    expanded: &Path,
    had_passphrase: bool,
    err: &impl std::fmt::Display,
) -> String {
    use crate::ssh::keyfile::{classify_path, KeyFileKind};

    let info = classify_path(raw_path);
    match info.kind {
        KeyFileKind::PublicKey => {
            let algo = info
                .algo
                .as_deref()
                .map(|a| format!(" ({a})"))
                .unwrap_or_default();
            let fix = if let Some(c) = &info.suggested_private_key {
                format!(" Use the matching private key instead: {}", c.path)
            } else if !info.sibling_private_keys.is_empty() {
                let names: Vec<_> = info
                    .sibling_private_keys
                    .iter()
                    .map(|c| c.name.clone())
                    .collect();
                format!(" Private keys in that folder: {}", names.join(", "))
            } else {
                String::new()
            };
            format!(
                "'{}' is an OpenSSH public key{}, not a private key.{}",
                expanded.display(),
                algo,
                fix
            )
        }
        KeyFileKind::NotFound => format!("Key file not found: {}", expanded.display()),
        KeyFileKind::NotAKey => format!(
            "'{}' is not a recognized private key file ({})",
            expanded.display(),
            err
        ),
        KeyFileKind::PrivateKey => {
            if had_passphrase {
                format!(
                    "Could not load private key '{}' — wrong passphrase? ({})",
                    expanded.display(),
                    err
                )
            } else {
                format!(
                    "Could not load private key '{}'. If it is passphrase-protected, enter the passphrase. ({})",
                    expanded.display(),
                    err
                )
            }
        }
    }
}

/// Attempt to authenticate via the local SSH agent (OpenSSH agent or Pageant
/// on Windows; SSH_AUTH_SOCK on Unix). Tries every identity the agent offers
/// and returns Ok(true) on the first one the server accepts. Returns Ok(false)
/// if no agent identity is accepted, or Err if the agent is unreachable.
/// Cascade through the available auth methods in OpenSSH order: configured
/// public key → ssh-agent identities → password. Returns Ok(true) when the
/// server accepts a method, Ok(false) when every available method is rejected.
/// Returns Err only on hard transport-level errors; all "auth was tried but
/// rejected" outcomes resolve to Ok(false) so the caller can decide what to
/// do (e.g. surface a password fallback prompt to the user).
async fn cascade_authenticate(
    handle: &mut russh::client::Handle<SshClientHandler>,
    username: &str,
    auth: &AuthParams,
) -> Result<bool, SshError> {
    // 1. Configured private key (file).
    if let Some(key_auth) = &auth.key {
        let expanded = expand_tilde(&key_auth.path);
        tracing::info!(
            "SSH key auth: loading key from '{}' (raw input: '{}')",
            expanded.display(),
            key_auth.path
        );
        let key = russh_keys::load_secret_key(&expanded, key_auth.passphrase.as_deref())
            .map_err(|e| {
                tracing::error!("SSH key load failed for '{}': {}", expanded.display(), e);
                SshError::ConnectionFailed(describe_key_load_error(
                    &key_auth.path,
                    &expanded,
                    key_auth.passphrase.is_some(),
                    &e,
                ))
            })?;
        tracing::info!(
            "SSH key loaded successfully, attempting publickey auth as '{}'",
            username
        );
        let accepted = handle
            .authenticate_publickey(username, Arc::new(key))
            .await
            .map_err(|e| {
                tracing::error!("SSH publickey auth error: {}", e);
                SshError::ConnectionFailed(format!("Auth error: {}", e))
            })?;
        tracing::info!("SSH publickey auth result: {}", accepted);
        if accepted {
            return Ok(true);
        }
    }

    // 2. ssh-agent identities (auto-detected: OpenSSH agent / Pageant / SSH_AUTH_SOCK).
    if auth.allow_agent {
        match try_agent_auth(handle, username.to_string()).await {
            Ok(true) => return Ok(true),
            Ok(false) => tracing::info!("SSH agent: no identity accepted"),
            Err(e) => tracing::info!("SSH agent fallback skipped: {}", e),
        }
    }

    // 3. Password.
    if let Some(password) = &auth.password {
        tracing::info!(
            "SSH password auth: attempting as '{}' (password length: {})",
            username,
            password.len()
        );
        let accepted = handle
            .authenticate_password(username, password)
            .await
            .map_err(|e| {
                tracing::error!("SSH password auth error: {}", e);
                SshError::ConnectionFailed(format!("Auth error: {}", e))
            })?;
        tracing::info!("SSH password auth result: {}", accepted);
        if accepted {
            return Ok(true);
        }
    }

    Ok(false)
}

/// Try every identity from the local SSH agent. Returns Ok(true) on the first
/// identity accepted by the server, Ok(false) if none are accepted, or Err if
/// the agent is unreachable or holds no keys. Cross-platform: uses OpenSSH's
/// Windows named pipe / Pageant on Windows; SSH_AUTH_SOCK on Unix.
async fn try_agent_auth(
    handle: &mut russh::client::Handle<SshClientHandler>,
    username: String,
) -> Result<bool, String> {
    #[cfg(unix)]
    {
        let agent = russh_keys::agent::client::AgentClient::connect_env()
            .await
            .map_err(|e| format!("ssh-agent unavailable (SSH_AUTH_SOCK): {}", e))?;
        try_agent_auth_inner(handle, username, agent).await
    }
    #[cfg(windows)]
    {
        // Try OpenSSH for Windows agent named pipe first (most common on Win10+).
        match russh_keys::agent::client::AgentClient::connect_named_pipe(
            r"\\.\pipe\openssh-ssh-agent",
        )
        .await
        {
            Ok(agent) => try_agent_auth_inner(handle, username, agent).await,
            Err(e) => {
                tracing::debug!("OpenSSH Windows agent named pipe unavailable: {}", e);
                let pageant = russh_keys::agent::client::AgentClient::connect_pageant().await;
                try_agent_auth_inner(handle, username, pageant).await
            }
        }
    }
}

async fn try_agent_auth_inner<S>(
    handle: &mut russh::client::Handle<SshClientHandler>,
    username: String,
    mut agent: russh_keys::agent::client::AgentClient<S>,
) -> Result<bool, String>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin + 'static,
{
    let identities = agent
        .request_identities()
        .await
        .map_err(|e| format!("agent request_identities failed: {}", e))?;
    if identities.is_empty() {
        return Err("ssh agent is reachable but holds no identities".into());
    }
    tracing::info!(
        "SSH agent offers {} identit{}",
        identities.len(),
        if identities.len() == 1 { "y" } else { "ies" }
    );
    let mut current_agent = agent;
    for (idx, key) in identities.into_iter().enumerate() {
        tracing::info!(
            "SSH agent: trying identity #{} (type: {})",
            idx + 1,
            key.name()
        );
        let (returned, result) = handle
            .authenticate_future(username.clone(), key, current_agent)
            .await;
        current_agent = returned;
        match result {
            Ok(true) => {
                tracing::info!("SSH agent: identity #{} accepted by server", idx + 1);
                return Ok(true);
            }
            Ok(false) => {
                tracing::info!("SSH agent: identity #{} rejected by server", idx + 1);
            }
            Err(e) => {
                tracing::warn!("SSH agent: identity #{} signing error: {:?}", idx + 1, e);
            }
        }
    }
    Ok(false)
}

/// A shared, clonable wrapper around the russh Handle.
/// Handle is not Clone, so we wrap it in Arc<Mutex<>> for reuse.
pub type SharedHandle = Arc<tokio::sync::Mutex<russh::client::Handle<SshClientHandler>>>;

#[derive(Debug, Error)]
pub enum SshError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Authentication rejected — server did not accept the public key (not in authorized_keys?) or password is wrong")]
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
    /// The frontend has attached its data listener — flush any buffered output
    /// and switch to live streaming. Until this arrives, the session task holds
    /// remote output (motd/banner) so nothing emitted before the terminal
    /// mounts is lost.
    Ready,
    Close,
}

/// Cascading SSH auth parameters. Each field is optional and tried in order:
/// configured key → ssh-agent identities → password. The first method the
/// server accepts wins. This mirrors OpenSSH's progressive auth — `ssh root@h`
/// without an `IdentitiesOnly yes` will try every loaded identity, then prompt
/// for a password if all fail.
#[derive(Debug, Clone, Default)]
pub struct AuthParams {
    pub key: Option<KeyAuth>,
    pub password: Option<String>,
    pub allow_agent: bool,
}

#[derive(Debug, Clone)]
pub struct KeyAuth {
    pub path: String,
    pub passphrase: Option<String>,
}

impl AuthParams {
    pub fn from_password(password: String) -> Self {
        Self { password: Some(password), allow_agent: true, ..Default::default() }
    }

    pub fn from_key(path: String, passphrase: Option<String>) -> Self {
        Self { key: Some(KeyAuth { path, passphrase }), allow_agent: true, ..Default::default() }
    }

    pub fn from_agent() -> Self {
        Self { allow_agent: true, ..Default::default() }
    }
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

pub(crate) struct ActiveConnection {
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

    /// Register a freshly-established connection and return its info.
    ///
    /// The slow connect work (`connect` / `connect_via_jump`) runs lock-free;
    /// the caller takes the global `ssh_manager` lock only to call this, which
    /// is a single HashMap insert — so a slow/hanging handshake on one host no
    /// longer blocks `ssh_send` / `ssh_resize` / `ssh_disconnect` on others.
    pub(crate) fn register(&mut self, conn: ActiveConnection) -> ConnectionInfo {
        let info = conn.info.clone();
        self.connections.insert(info.id.clone(), conn);
        info
    }

    /// Establish a direct SSH connection. Takes no `self` and does NOT touch the
    /// connections map — it returns the finished `ActiveConnection` for the
    /// caller to `register` under a brief lock. This keeps the slow handshake/
    /// auth off the global lock so other connections stay responsive.
    pub(crate) async fn connect(
        id: &str,
        host: &str,
        port: u16,
        username: &str,
        auth: AuthParams,
        cols: u16,
        rows: u16,
        app_handle: tauri::AppHandle,
        proxy: Option<ProxyConfig>,
        shell: Option<String>,
        inject_colors: bool,
    ) -> Result<ActiveConnection, SshError> {
        tracing::info!("SSH connecting to {}@{}:{}", username, host, port);

        let timeout_duration = std::time::Duration::from_secs(15);
        let connect_future = async {
            let config = Arc::new(russh::client::Config::default());
            let handler = SshClientHandler::new(host, port, Some(app_handle.clone()));

            let mut handle = if let Some(ref proxy) = proxy {
                tracing::info!("SSH connecting via {} proxy {}:{}", proxy.proxy_type, proxy.host, proxy.port);
                let stream = Self::connect_via_proxy(proxy, host, port).await?;
                russh::client::connect_stream(config, stream, handler)
                    .await
                    .map_err(|e| SshError::ConnectionFailed(format!("Proxy SSH handshake failed: {}", e)))?
            } else {
                russh::client::connect(config, (host, port), handler)
                    .await
                    .map_err(|e| SshError::ConnectionFailed(format!("{}", e)))?
            };

            // Authenticate using a cascading strategy: configured key → agent → password.
            // The first method the server accepts wins. Mirrors OpenSSH's progressive auth.
            if !cascade_authenticate(&mut handle, username, &auth).await? {
                return Err(SshError::AuthFailed);
            }

            tracing::info!("SSH authenticated for {}@{}:{}", username, host, port);

            let channel = handle.channel_open_session().await
                .map_err(|e| SshError::ChannelError(format!("Failed to open session: {}", e)))?;

            open_interactive_shell(&channel, cols, rows, shell.as_deref()).await?;

            tracing::info!("SSH shell opened for {}@{}:{}", username, host, port);

            Ok((handle, channel))
        };

        let (handle, channel) = tokio::time::timeout(timeout_duration, connect_future)
            .await
            .map_err(|_| SshError::ConnectionFailed("Connection timed out".into()))??;

        let info = ConnectionInfo {
            id: id.to_string(),
            host: host.to_string(),
            port,
            username: username.to_string(),
        };

        into_active_connection(channel, handle, info, shell.as_deref(), inject_colors, app_handle, Vec::new()).await
    }

    /// Connect to a target host through one or more jump hosts (ProxyJump).
    /// `jump_chain` is ordered outermost-first: connect to first hop, then tunnel through.
    /// Connect to a target host through a SOCKS5/SOCKS4/HTTP proxy.
    async fn connect_via_proxy(
        proxy: &ProxyConfig,
        target_host: &str,
        target_port: u16,
    ) -> Result<tokio::net::TcpStream, SshError> {
        let proxy_addr = format!("{}:{}", proxy.host, proxy.port);
        let target_addr = (target_host, target_port);

        match proxy.proxy_type.to_lowercase().as_str() {
            "socks5" => {
                let stream = if let (Some(user), Some(pass)) = (&proxy.username, &proxy.password) {
                    tokio_socks::tcp::Socks5Stream::connect_with_password(
                        proxy_addr.as_str(),
                        target_addr,
                        user.as_str(),
                        pass.as_str(),
                    )
                    .await
                    .map_err(|e| SshError::ConnectionFailed(format!("SOCKS5 proxy error: {}", e)))?
                } else {
                    tokio_socks::tcp::Socks5Stream::connect(
                        proxy_addr.as_str(),
                        target_addr,
                    )
                    .await
                    .map_err(|e| SshError::ConnectionFailed(format!("SOCKS5 proxy error: {}", e)))?
                };
                Ok(stream.into_inner())
            }
            "socks4" => {
                let stream = tokio_socks::tcp::Socks4Stream::connect(
                    proxy_addr.as_str(),
                    target_addr,
                )
                .await
                .map_err(|e| SshError::ConnectionFailed(format!("SOCKS4 proxy error: {}", e)))?;
                Ok(stream.into_inner())
            }
            "http" => {
                // HTTP CONNECT proxy
                use tokio::io::{AsyncReadExt, AsyncWriteExt};
                let mut stream = tokio::net::TcpStream::connect(&proxy_addr)
                    .await
                    .map_err(|e| SshError::ConnectionFailed(format!("HTTP proxy connect error: {}", e)))?;

                let connect_req = if let (Some(user), Some(pass)) = (&proxy.username, &proxy.password) {
                    let creds = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, format!("{}:{}", user, pass));
                    format!(
                        "CONNECT {}:{} HTTP/1.1\r\nHost: {}:{}\r\nProxy-Authorization: Basic {}\r\n\r\n",
                        target_host, target_port, target_host, target_port, creds
                    )
                } else {
                    format!(
                        "CONNECT {}:{} HTTP/1.1\r\nHost: {}:{}\r\n\r\n",
                        target_host, target_port, target_host, target_port
                    )
                };

                stream.write_all(connect_req.as_bytes()).await
                    .map_err(|e| SshError::ConnectionFailed(format!("HTTP proxy write error: {}", e)))?;

                let mut buf = [0u8; 1024];
                let n = stream.read(&mut buf).await
                    .map_err(|e| SshError::ConnectionFailed(format!("HTTP proxy read error: {}", e)))?;
                let response = String::from_utf8_lossy(&buf[..n]);

                if !response.contains("200") {
                    return Err(SshError::ConnectionFailed(format!("HTTP proxy rejected: {}", response.lines().next().unwrap_or(""))));
                }

                Ok(stream)
            }
            _ => Err(SshError::ConnectionFailed(format!("Unsupported proxy type: {}", proxy.proxy_type))),
        }
    }

    /// Establish an SSH connection through one or more jump hosts. Like
    /// [`connect`], takes no `self` and returns the finished `ActiveConnection`
    /// for the caller to `register` under a brief lock.
    pub(crate) async fn connect_via_jump(
        id: &str,
        target_host: &str,
        target_port: u16,
        target_username: &str,
        target_auth: AuthParams,
        jump_chain: Vec<JumpHostParams>,
        cols: u16,
        rows: u16,
        app_handle: tauri::AppHandle,
        shell: Option<String>,
        inject_colors: bool,
    ) -> Result<ActiveConnection, SshError> {
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
            let handler = SshClientHandler::new(first_jump.host.as_str(), first_jump.port, Some(app_handle.clone()));

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
                    let handler = SshClientHandler::new(next_jump.host.as_str(), next_jump.port, Some(app_handle.clone()));

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
                let handler = SshClientHandler::new(target_host, target_port, Some(app_handle.clone()));

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
                let handler = SshClientHandler::new(target_host, target_port, Some(app_handle.clone()));

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

        // Open session, request PTY and the (optionally overridden) shell on target
        let channel = target_handle
            .channel_open_session()
            .await
            .map_err(|e| SshError::ChannelError(format!("Failed to open session: {}", e)))?;

        open_interactive_shell(&channel, cols, rows, shell.as_deref()).await?;

        tracing::info!(
            "SSH shell opened for {}@{}:{} (via jump)",
            target_username, target_host, target_port
        );

        let info = ConnectionInfo {
            id: id.to_string(),
            host: target_host.to_string(),
            port: target_port,
            username: target_username.to_string(),
        };

        into_active_connection(channel, target_handle, info, shell.as_deref(), inject_colors, app_handle, jump_handles).await
    }

    /// Authenticate on a russh handle by cascading through the configured
    /// methods. Used by jump hosts; the direct connect path uses the same
    /// `cascade_authenticate` free function.
    async fn authenticate_handle(
        handle: &mut russh::client::Handle<SshClientHandler>,
        username: &str,
        auth: &AuthParams,
    ) -> Result<(), SshError> {
        if !cascade_authenticate(handle, username, auth).await? {
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

    /// Signal that the frontend has attached its listener: flush buffered output
    /// and stream live. Idempotent — extra calls after the first are no-ops.
    pub fn mark_ready(&self, id: &str) -> Result<(), SshError> {
        let conn = self.connections.get(id)
            .ok_or_else(|| SshError::NotFound(id.to_string()))?;
        conn.cmd_tx.send(SessionCommand::Ready)
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

/// Pending host-key prompts awaiting a user decision, keyed by a per-prompt id.
/// The SSH handshake parks on the oneshot here while the frontend shows the
/// verification dialog; `resolve_hostkey_prompt` (via the `ssh_hostkey_response`
/// IPC command) wakes it. Safe to add now that connect runs lock-free, so a
/// parked handshake no longer blocks other SSH operations.
static HOSTKEY_PROMPTS: std::sync::OnceLock<
    std::sync::Mutex<HashMap<String, tokio::sync::oneshot::Sender<bool>>>,
> = std::sync::OnceLock::new();

fn hostkey_prompts(
) -> &'static std::sync::Mutex<HashMap<String, tokio::sync::oneshot::Sender<bool>>> {
    HOSTKEY_PROMPTS.get_or_init(|| std::sync::Mutex::new(HashMap::new()))
}

/// Resolve a pending host-key prompt with the user's accept/reject decision.
pub(crate) fn resolve_hostkey_prompt(prompt_id: &str, accept: bool) {
    let sender = hostkey_prompts().lock().unwrap().remove(prompt_id);
    if let Some(tx) = sender {
        let _ = tx.send(accept);
    }
}

/// Emitted to the frontend when a host key needs user verification.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HostKeyPrompt {
    prompt_id: String,
    host: String,
    port: u16,
    fingerprint: String,
    key_type: String,
    /// true = the stored key for this host CHANGED (possible MITM); false = a
    /// brand-new (unknown) host being trusted on first use (TOFU).
    changed: bool,
    old_fingerprint: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SshClientHandler {
    host: String,
    port: u16,
    app_handle: Option<tauri::AppHandle>,
}

impl SshClientHandler {
    pub fn new(host: impl Into<String>, port: u16, app_handle: Option<tauri::AppHandle>) -> Self {
        Self { host: host.into(), port, app_handle }
    }

    fn known_hosts_path() -> std::path::PathBuf {
        // Use the Tauri-resolved writable app data dir (not `dirs::data_dir()`)
        // so this works inside the Android/iOS sandbox too.
        crate::app_data_dir().join("ssh").join("known_hosts.json")
    }

    /// Ask the user to verify a host key. Emits `ssh-hostkey-prompt` and parks
    /// on a oneshot until `ssh_hostkey_response` resolves it, or a 120s timeout.
    /// Fails closed: rejects on a missing UI handle, emit error, or timeout.
    async fn prompt_hostkey(
        &self,
        fingerprint: &str,
        key_type: &str,
        changed: bool,
        old_fingerprint: Option<String>,
    ) -> bool {
        let Some(app) = self.app_handle.clone() else {
            tracing::warn!(
                "No UI handle to verify host key for {}:{}; rejecting",
                self.host,
                self.port
            );
            return false;
        };

        let prompt_id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = tokio::sync::oneshot::channel();
        hostkey_prompts().lock().unwrap().insert(prompt_id.clone(), tx);

        let payload = HostKeyPrompt {
            prompt_id: prompt_id.clone(),
            host: self.host.clone(),
            port: self.port,
            fingerprint: fingerprint.to_string(),
            key_type: key_type.to_string(),
            changed,
            old_fingerprint,
        };
        if app.emit("ssh-hostkey-prompt", &payload).is_err() {
            hostkey_prompts().lock().unwrap().remove(&prompt_id);
            return false;
        }

        match tokio::time::timeout(std::time::Duration::from_secs(120), rx).await {
            Ok(Ok(accept)) => accept,
            _ => {
                hostkey_prompts().lock().unwrap().remove(&prompt_id);
                false
            }
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct KnownHosts {
    entries: HashMap<String, String>,
}

#[async_trait]
impl russh::client::Handler for SshClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &russh_keys::key::PublicKey,
    ) -> Result<bool, Self::Error> {
        let host_id = format!("{}:{}", self.host, self.port);
        let fingerprint = server_public_key.fingerprint().to_string();
        let key_type = server_public_key.name().to_string();

        let path = Self::known_hosts_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let mut known: KnownHosts = match std::fs::read_to_string(&path) {
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
            Err(_) => KnownHosts::default(),
        };

        // Known host with a matching fingerprint — trusted, connect silently.
        if known.entries.get(&host_id) == Some(&fingerprint) {
            return Ok(true);
        }

        // Unknown host (TOFU) or — worse — a CHANGED key (possible MITM): ask
        // the user before trusting it. No silent accept.
        let (changed, old_fingerprint) = match known.entries.get(&host_id) {
            Some(existing) => {
                tracing::warn!(
                    "SSH host key CHANGED for {} (possible MITM). Old: {}, New: {}",
                    host_id, existing, fingerprint
                );
                (true, Some(existing.clone()))
            }
            None => {
                tracing::info!("SSH host key unknown for {} — prompting (TOFU)", host_id);
                (false, None)
            }
        };

        let accepted = self
            .prompt_hostkey(&fingerprint, &key_type, changed, old_fingerprint)
            .await;

        if accepted {
            known.entries.insert(host_id.clone(), fingerprint);
            if let Ok(raw) = serde_json::to_string_pretty(&known) {
                let _ = std::fs::write(&path, raw);
            }
            tracing::info!("SSH host key accepted by user for {}", host_id);
            Ok(true)
        } else {
            tracing::warn!("SSH host key rejected for {} — aborting connect", host_id);
            Ok(false)
        }
    }
}

/// Finish a connection once the channel is open: inject shell color/prompt init
/// (when enabled for the shell), spawn the streaming session task, and build the
/// `ActiveConnection`. Shared by both the direct and jump-host connect paths.
async fn into_active_connection(
    channel: russh::Channel<russh::client::Msg>,
    handle: russh::client::Handle<SshClientHandler>,
    info: ConnectionInfo,
    shell: Option<&str>,
    inject_colors: bool,
    app_handle: tauri::AppHandle,
    jump_handles: Vec<SharedHandle>,
) -> Result<ActiveConnection, SshError> {
    // Inject shell-appropriate color/prompt init (chosen per shell family so a
    // fish login never gets bash syntax), unless the user disabled it. `None`
    // shell-family => nothing injected.
    if inject_colors {
        if let Some(init) = shell_init(shell) {
            channel
                .data(init.as_bytes())
                .await
                .map_err(|e| SshError::ChannelError(format!("Color init failed: {}", e)))?;
        }
    }

    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
    let task_id = info.id.clone();
    let task_handle = app_handle.clone();
    tokio::spawn(async move {
        ssh_session_task(channel, cmd_rx, task_id, task_handle).await;
    });

    Ok(ActiveConnection {
        cmd_tx,
        info,
        handle: Arc::new(tokio::sync::Mutex::new(handle)),
        jump_handles,
    })
}

async fn ssh_session_task(
    mut channel: russh::Channel<russh::client::Msg>,
    mut cmd_rx: mpsc::UnboundedReceiver<SessionCommand>,
    connection_id: String,
    app_handle: tauri::AppHandle,
) {
    let data_event = format!("ssh-data-{}", connection_id);
    let exit_event = format!("ssh-exit-{}", connection_id);

    // Hold remote output until the frontend signals it's listening
    // (SessionCommand::Ready) so the motd/banner emitted before the terminal
    // mounts isn't dropped. A safety cap flushes anyway if Ready never arrives.
    const BACKLOG_CAP: usize = 2 * 1024 * 1024;
    let mut ready = false;
    let mut backlog: Vec<String> = Vec::new();
    let mut backlog_bytes = 0usize;

    // Emit live once ready, otherwise buffer. `break`s the loop on emit failure.
    macro_rules! deliver {
        ($payload:expr) => {{
            let payload = $payload;
            if ready {
                if let Err(e) = app_handle.emit(&data_event, &payload) {
                    tracing::error!("Failed to emit '{}': {}", data_event, e);
                    break;
                }
            } else {
                backlog_bytes += payload.len();
                backlog.push(payload);
                if backlog_bytes >= BACKLOG_CAP {
                    ready = true;
                    for p in backlog.drain(..) {
                        let _ = app_handle.emit(&data_event, &p);
                    }
                }
            }
        }};
    }

    loop {
        tokio::select! {
            msg = channel.wait() => {
                match msg {
                    Some(ChannelMsg::Data { ref data }) => {
                        deliver!(String::from_utf8_lossy(data).to_string());
                    }
                    Some(ChannelMsg::ExtendedData { ref data, .. }) => {
                        deliver!(String::from_utf8_lossy(data).to_string());
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
                    Some(SessionCommand::Ready) => {
                        if !ready {
                            ready = true;
                            for p in backlog.drain(..) {
                                let _ = app_handle.emit(&data_event, &p);
                            }
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

#[cfg(test)]
mod shell_tests {
    use super::*;

    #[test]
    fn family_defaults_to_posix_when_unset() {
        assert!(matches!(shell_family(None), ShellFamily::Posix));
        assert!(matches!(shell_family(Some("   ")), ShellFamily::Posix));
    }

    #[test]
    fn family_detects_fish_by_basename_and_flags() {
        assert!(matches!(shell_family(Some("fish")), ShellFamily::Fish));
        assert!(matches!(shell_family(Some("/usr/bin/fish")), ShellFamily::Fish));
        assert!(matches!(shell_family(Some("fish -l")), ShellFamily::Fish));
    }

    #[test]
    fn family_detects_posix_shells() {
        for s in ["bash", "/bin/bash", "zsh", "sh", "dash", "/usr/bin/zsh -l"] {
            assert!(matches!(shell_family(Some(s)), ShellFamily::Posix), "{s}");
        }
    }

    #[test]
    fn family_unknown_shell_is_other() {
        assert!(matches!(shell_family(Some("nu")), ShellFamily::Other));
        assert!(matches!(shell_family(Some("xonsh")), ShellFamily::Other));
    }

    #[test]
    fn init_is_posix_blob_when_unset_or_posix() {
        assert_eq!(shell_init(None).as_deref(), Some(POSIX_COLOR_INIT));
        assert_eq!(shell_init(Some("bash")).as_deref(), Some(POSIX_COLOR_INIT));
    }

    #[test]
    fn init_for_fish_avoids_bash_syntax() {
        // The whole point: fish must not receive `export`, `$(...)`, or `then/fi`.
        let init = shell_init(Some("fish")).expect("fish gets an init");
        assert!(init.contains("set -gx COLORTERM"));
        assert!(!init.contains("export "));
        assert!(!init.contains("$("));
        assert!(!init.contains("fi;"));
    }

    #[test]
    fn init_skipped_for_unknown_shell() {
        assert!(shell_init(Some("nu")).is_none());
    }
}
