//! IPC surface for the MCP server.
//!
//! Everything here is a deliberate user action. Nothing starts on its own,
//! nothing is shared on its own, and the confirm flow mirrors
//! `ssh_hostkey_response`: emit an event, park on a oneshot, fail closed on a
//! missing UI handle, an emit error, or a timeout.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use serde::Serialize;
use tauri::{Emitter, Manager};

use crate::mcp::agents;
use crate::mcp::server::RunningServer;
use crate::mcp::session::SessionKind;
use crate::mcp::{ConfirmRequest, McpState, SendRequest};
use crate::state::AppState;

/// Pending confirmations, keyed by prompt id. Same shape as the host-key
/// prompts so the two behave identically under timeout and window close.
fn pending() -> &'static Mutex<HashMap<String, tokio::sync::oneshot::Sender<bool>>> {
    static P: OnceLock<Mutex<HashMap<String, tokio::sync::oneshot::Sender<bool>>>> =
        OnceLock::new();
    P.get_or_init(|| Mutex::new(HashMap::new()))
}

/// The running listener, if any. Separate from [`McpState`] because stopping is
/// a drop, and the state is shared with the server's own handlers.
fn running() -> &'static Mutex<Option<RunningServer>> {
    static R: OnceLock<Mutex<Option<RunningServer>>> = OnceLock::new();
    R.get_or_init(|| Mutex::new(None))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpStatus {
    pub enabled: bool,
    pub port: Option<u16>,
    pub token: Option<String>,
    pub agent_id: String,
    pub agent_name: String,
    pub read_only: bool,
    pub shared_session_ids: Vec<String>,
    /// Ready to paste into a client's config.
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub read_only: bool,
    pub tools: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConfirmPayload {
    prompt_id: String,
    session_id: String,
    host: String,
    command: String,
    rationale: String,
    references: Vec<String>,
    rollback: Option<String>,
    agent_name: String,
    danger: String,
    danger_reason: Option<String>,
}

/// A fresh token per start.
///
/// Not persisted anywhere: there is nothing at rest to leak, and a credential
/// that outlives the session it authorised is a credential nobody remembers
/// revoking. The cost is re-pasting it into the client config after a restart,
/// which is the correct trade for a token that opens a shell.
fn new_token() -> String {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn status_of(state: &McpState, port: Option<u16>, shared: Vec<String>) -> McpStatus {
    let agent = state.agent();
    McpStatus {
        enabled: state.is_enabled(),
        port,
        token: state.token(),
        agent_id: agent.id.clone(),
        agent_name: agent.name.clone(),
        read_only: agent.is_read_only(),
        shared_session_ids: shared,
        url: port.map(|p| format!("http://127.0.0.1:{p}/mcp")),
    }
}

#[tauri::command]
#[tracing::instrument(skip(state, app))]
pub async fn mcp_start(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    port: Option<u16>,
) -> Result<McpStatus, String> {
    let mcp = state.mcp.clone();

    if running().lock().unwrap().is_some() {
        let p = running().lock().unwrap().as_ref().map(|r| r.port);
        return Ok(status_of(&mcp, p, mcp.shared_ids().await));
    }

    mcp.set_token(Some(new_token()));

    // Wire the confirm dialog. Until this exists every write fails closed, so
    // it is attached as part of starting rather than lazily on first use.
    let app_for_confirm = app.clone();
    mcp.set_confirmer(Some(std::sync::Arc::new(move |req: ConfirmRequest| {
        let app = app_for_confirm.clone();
        Box::pin(async move { ask_user(app, req).await })
    })))
    .await;

    let app_for_send = app.clone();
    mcp.set_sender(Some(std::sync::Arc::new(move |req: SendRequest| {
        let app = app_for_send.clone();
        Box::pin(async move { type_into_session(app, req).await })
    })))
    .await;

    // 0 asks the OS for a free port. A fixed default that collides becomes a
    // support question; letting the system choose never does.
    let server = crate::mcp::server::start(mcp.clone(), port.unwrap_or(0)).await?;
    let bound = server.port;
    *running().lock().unwrap() = Some(server);
    mcp.set_enabled(true);

    tracing::info!("MCP server enabled on port {bound}");
    Ok(status_of(&mcp, Some(bound), mcp.shared_ids().await))
}

#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn mcp_stop(state: tauri::State<'_, AppState>) -> Result<McpStatus, String> {
    let mcp = state.mcp.clone();

    if let Some(mut s) = running().lock().unwrap().take() {
        s.stop();
    }
    mcp.set_enabled(false);
    // Stopping revokes: the token dies, sharing is cleared, and every client's
    // preconditions go with it. Restarting is a clean slate, not a resumption.
    mcp.set_token(None);
    mcp.set_confirmer(None).await;
    mcp.set_sender(None).await;
    mcp.clear().await;
    pending().lock().unwrap().clear();

    tracing::info!("MCP server disabled");
    Ok(status_of(&mcp, None, Vec::new()))
}

#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn mcp_status(state: tauri::State<'_, AppState>) -> Result<McpStatus, String> {
    let mcp = state.mcp.clone();
    let port = running().lock().unwrap().as_ref().map(|r| r.port);
    Ok(status_of(&mcp, port, mcp.shared_ids().await))
}

/// Offer one session to the AI. Enabling the server does not do this; every
/// session is a separate, explicit decision.
#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state))]
pub async fn mcp_share_session(
    state: tauri::State<'_, AppState>,
    session_id: String,
    kind: String,
    host: String,
    username: String,
) -> Result<Vec<String>, String> {
    let mcp = state.mcp.clone();
    if !mcp.is_enabled() {
        return Err("The MCP server is not running. Enable it in Settings > AI first.".into());
    }
    let kind = match kind.as_str() {
        "ssh" => SessionKind::Ssh,
        "local" => SessionKind::Local,
        other => return Err(format!("Unknown session kind `{other}`")),
    };
    mcp.share(session_id, kind, host, username).await;
    Ok(mcp.shared_ids().await)
}

#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state))]
pub async fn mcp_unshare_session(
    state: tauri::State<'_, AppState>,
    session_id: String,
) -> Result<Vec<String>, String> {
    let mcp = state.mcp.clone();
    mcp.unshare(&session_id).await;
    Ok(mcp.shared_ids().await)
}

/// The agents available to choose from. The AI never calls this — it is for
/// Reach's own settings UI.
#[tauri::command]
pub async fn mcp_list_agents() -> Result<Vec<AgentSummary>, String> {
    Ok(agents::built_in()
        .into_iter()
        .map(|a| AgentSummary {
            id: a.id.clone(),
            name: a.name.clone(),
            description: a.description.clone(),
            read_only: a.is_read_only(),
            tools: a.tools.iter().map(|t| t.as_str().to_string()).collect(),
        })
        .collect())
}

/// Select the active agent. Only reachable from Reach's UI; there is no MCP
/// method that lands here.
#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state))]
pub async fn mcp_set_agent(
    state: tauri::State<'_, AppState>,
    agent_id: String,
) -> Result<McpStatus, String> {
    let mcp = state.mcp.clone();
    mcp.set_agent(&agent_id)?;
    let port = running().lock().unwrap().as_ref().map(|r| r.port);
    Ok(status_of(&mcp, port, mcp.shared_ids().await))
}

/// Resolve a pending confirmation. Called by the dialog.
#[tauri::command(rename_all = "snake_case")]
pub fn mcp_confirm_response(prompt_id: String, approved: bool) -> Result<(), String> {
    if let Some(tx) = pending().lock().unwrap().remove(&prompt_id) {
        let _ = tx.send(approved);
        Ok(())
    } else {
        // Already resolved or timed out. Not an error worth surfacing: the
        // command has already been refused, which is the safe outcome.
        Ok(())
    }
}

/// Put a proposed command in front of the user and wait.
///
/// Fails closed on every path: no window, emit failure, or timeout all mean
/// "not approved". Two minutes matches the host-key prompt — long enough to
/// read a rationale, short enough that a forgotten dialog does not leave an AI
/// blocked indefinitely.
async fn ask_user(app: tauri::AppHandle, req: ConfirmRequest) -> bool {
    if app.get_webview_window("main").is_none() {
        tracing::warn!("No window to confirm an MCP command; refusing");
        return false;
    }

    let prompt_id = uuid::Uuid::new_v4().to_string();
    let (tx, rx) = tokio::sync::oneshot::channel();
    pending().lock().unwrap().insert(prompt_id.clone(), tx);

    let payload = ConfirmPayload {
        prompt_id: prompt_id.clone(),
        session_id: req.session_id,
        host: req.host,
        command: req.command,
        rationale: req.rationale,
        references: req.references,
        rollback: req.rollback,
        agent_name: req.agent_name,
        danger: req.danger,
        danger_reason: req.danger_reason,
    };

    if app.emit("mcp-confirm-request", &payload).is_err() {
        pending().lock().unwrap().remove(&prompt_id);
        return false;
    }

    match tokio::time::timeout(std::time::Duration::from_secs(120), rx).await {
        Ok(Ok(approved)) => approved,
        _ => {
            pending().lock().unwrap().remove(&prompt_id);
            false
        }
    }
}

/// Echo the command into the visible terminal, then actually type it.
///
/// The echo is written to the *display only* — it goes out on the same event the
/// terminal already listens to, so it lands in the scrollback without a byte of
/// it reaching the remote shell. That matters: an AI typing into a terminal you
/// are also watching should leave a trace you can scroll back to, and "what did
/// it just do?" should be answerable from the terminal itself rather than from
/// a settings panel.
///
/// Dim and bracketed with the agent's name, so it reads as annotation rather
/// than as something the shell printed.
async fn type_into_session(app: tauri::AppHandle, req: SendRequest) -> Result<(), String> {
    // \r\n rather than \n: the terminal is in raw mode, so a bare newline moves
    // down without returning to column zero and the banner walks diagonally.
    let banner = format!(
        "\r\n\x1b[2m\x1b[38;5;180m\u{250c}\u{2500} AI \u{00b7} {} \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\x1b[0m\r\n\
         \x1b[2m\u{2502}\x1b[0m \x1b[38;5;180m{}\x1b[0m\r\n\
         \x1b[2m\u{2514}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\x1b[0m\r\n",
        req.agent_name, req.command
    );

    let state = app
        .try_state::<AppState>()
        .ok_or_else(|| "Reach is still starting up".to_string())?;

    // The newline is added here rather than asked of the caller: the tool
    // contract says "do not include one", and a model that forgets would leave
    // a half-typed line sitting at the prompt with no way to notice.
    let mut bytes = req.command.clone().into_bytes();
    bytes.push(b'\n');

    match req.kind {
        SessionKind::Ssh => {
            let _ = app.emit(&format!("ssh-data-{}", req.session_id), banner);
            let manager = state.ssh_manager.lock().await;
            manager
                .send_data(&req.session_id, &bytes)
                .map_err(|e| e.to_string())
        }
        SessionKind::Local => {
            #[cfg(desktop)]
            {
                let _ = app.emit(&format!("pty-data-{}", req.session_id), banner);
                let mut manager = state
                    .pty_manager
                    .lock()
                    .map_err(|_| "PTY manager is locked".to_string())?;
                manager
                    .write(&req.session_id, &bytes)
                    .map_err(|e| e.to_string())
            }
            #[cfg(not(desktop))]
            {
                Err("Local terminals are not available on this platform".to_string())
            }
        }
    }
}
