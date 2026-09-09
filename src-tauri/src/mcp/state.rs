//! Server state and method dispatch.
//!
//! Holds the shared sessions, the per-client bookkeeping, the token and the
//! agent the *user* selected. The AI can read which agent is active; it cannot
//! set one. If a model could choose its own agent it would choose the least
//! restricted, and the whole guard would be decoration.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde_json::{json, Value};
use tokio::sync::{Mutex, RwLock};

use crate::mcp::agents::{self, Agent};
use crate::mcp::guard::WriteRequest;
use crate::mcp::protocol::{self, Request};
use crate::mcp::session::{ClientState, SessionKind, SharedSession};
use crate::mcp::tools;

/// Asks the user to approve a write, returning true if they did.
///
/// Boxed rather than a concrete type so tests can approve or reject without a
/// window, and so nothing in this module needs a Tauri handle.
pub type Confirmer = Arc<
    dyn Fn(ConfirmRequest) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send>>
        + Send
        + Sync,
>;

/// Types an approved command into the session, and echoes it into the visible
/// terminal so the user watches it happen.
///
/// Separate from [`Confirmer`] and injected the same way, for the same reason:
/// nothing in this module holds a Tauri handle, and a test can observe what
/// would have been typed without a terminal existing.
pub type Sender = Arc<
    dyn Fn(SendRequest) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send>>
        + Send
        + Sync,
>;

/// What actually reaches the terminal.
#[derive(Debug, Clone)]
pub struct SendRequest {
    pub session_id: String,
    pub kind: SessionKind,
    pub command: String,
    /// Shown in the echo banner so the scrollback records which agent acted.
    pub agent_name: String,
}

/// What the user is shown before a command runs. Everything here is on the
/// dialog: the point is that the human decides with the model's reasoning in
/// front of them, not that they rubber-stamp an opaque string.
#[derive(Debug, Clone)]
pub struct ConfirmRequest {
    pub session_id: String,
    pub host: String,
    pub command: String,
    pub rationale: String,
    pub references: Vec<String>,
    pub rollback: Option<String>,
    pub agent_name: String,
    pub danger: String,
    pub danger_reason: Option<String>,
}

pub struct McpState {
    enabled: AtomicBool,
    token: std::sync::RwLock<Option<String>>,
    agent_id: std::sync::RwLock<String>,
    sessions: RwLock<HashMap<String, SharedSession>>,
    clients: RwLock<HashMap<String, ClientState>>,
    confirmer: Mutex<Option<Confirmer>>,
    sender: Mutex<Option<Sender>>,
}

impl Default for McpState {
    fn default() -> Self {
        Self {
            enabled: AtomicBool::new(false),
            token: std::sync::RwLock::new(None),
            // The most restricted built-in. A default that can write is a
            // default that surprises someone.
            agent_id: std::sync::RwLock::new(agents::default_agent().id),
            sessions: RwLock::new(HashMap::new()),
            clients: RwLock::new(HashMap::new()),
            confirmer: Mutex::new(None),
            sender: Mutex::new(None),
        }
    }
}

impl McpState {
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn set_enabled(&self, on: bool) {
        self.enabled.store(on, Ordering::Relaxed);
    }

    pub fn token(&self) -> Option<String> {
        self.token.read().ok().and_then(|g| g.clone())
    }

    pub fn set_token(&self, t: Option<String>) {
        if let Ok(mut g) = self.token.write() {
            *g = t;
        }
    }

    pub fn agent(&self) -> Agent {
        let id = self.agent_id.read().ok().map(|g| g.clone()).unwrap_or_default();
        agents::find(&id).unwrap_or_else(agents::default_agent)
    }

    /// Set by the user in Reach's settings. There is deliberately no MCP method
    /// that reaches this.
    pub fn set_agent(&self, id: &str) -> Result<(), String> {
        if agents::find(id).is_none() {
            return Err(format!("Unknown agent `{id}`"));
        }
        if let Ok(mut g) = self.agent_id.write() {
            *g = id.to_string();
        }
        Ok(())
    }

    pub fn session_count(&self) -> usize {
        self.sessions.try_read().map(|s| s.len()).unwrap_or(0)
    }

    pub async fn set_confirmer(&self, c: Option<Confirmer>) {
        *self.confirmer.lock().await = c;
    }

    pub async fn set_sender(&self, s: Option<Sender>) {
        *self.sender.lock().await = s;
    }

    /// Share a session. Nothing is visible to any client until this is called.
    pub async fn share(&self, id: String, kind: SessionKind, host: String, username: String) {
        self.sessions
            .write()
            .await
            .insert(id.clone(), SharedSession::new(id, kind, host, username));
    }

    /// Stop sharing. Every client's preconditions for it are dropped, so
    /// re-sharing later cannot reuse stale consent.
    pub async fn unshare(&self, id: &str) {
        self.sessions.write().await.remove(id);
        for client in self.clients.write().await.values_mut() {
            client.forget(id);
        }
    }

    pub async fn shared_ids(&self) -> Vec<String> {
        let mut v: Vec<String> = self.sessions.read().await.keys().cloned().collect();
        v.sort();
        v
    }

    /// Feed terminal output. A no-op for a session that is not shared, which is
    /// the common case — the read loops call this unconditionally.
    pub async fn push_output(&self, id: &str, bytes: &[u8]) {
        if let Some(s) = self.sessions.write().await.get_mut(id) {
            s.push_output(bytes);
        }
    }

    /// Everything a client is permitted to touch is dropped when sharing stops
    /// entirely, so disabling the server leaves nothing behind.
    pub async fn clear(&self) {
        self.sessions.write().await.clear();
        self.clients.write().await.clear();
    }

    /// Dispatch one JSON-RPC method.
    pub async fn handle(&self, req: Request) -> Result<Value, (i32, String)> {
        // One client for now: the token identifies the connection, and Reach
        // hands out a single token. Kept keyed so per-client preconditions do
        // not have to be rewritten when multiple clients are supported.
        let client_key = "default".to_string();

        match req.method.as_str() {
            "initialize" => {
                let version = req.params.get("protocolVersion").and_then(|v| v.as_str());
                Ok(protocol::initialize_result(version))
            }
            "notifications/initialized" | "notifications/cancelled" => Ok(Value::Null),
            "ping" => Ok(json!({})),

            "tools/list" => Ok(json!({ "tools": tools::definitions(&self.agent()) })),

            "prompts/list" => Ok(json!({ "prompts": tools::prompt_definitions(&self.agent()) })),
            "prompts/get" => {
                let agent = self.agent();
                let name = req.params.get("name").and_then(|v| v.as_str()).unwrap_or("");
                if name != agent.id {
                    return Err((
                        protocol::INVALID_PARAMS,
                        format!(
                            "`{name}` is not the active agent. The user selects the agent in \
                             Reach; the active one is `{}`.",
                            agent.id
                        ),
                    ));
                }
                Ok(tools::prompt_content(&agent))
            }

            "tools/call" => self.call_tool(req.params, &client_key).await,

            other => Err((
                protocol::METHOD_NOT_FOUND,
                format!("Method `{other}` is not implemented by this server."),
            )),
        }
    }

    async fn call_tool(&self, params: Value, client_key: &str) -> Result<Value, (i32, String)> {
        let name = params
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or((protocol::INVALID_PARAMS, "Missing tool name".to_string()))?;
        let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
        let agent = self.agent();

        // A tool the agent was not granted is not in `tools/list`, so calling it
        // is a mistake worth an explicit refusal rather than silence.
        let Some(tool) = crate::mcp::agents::ToolName::from_str(name) else {
            return Err((protocol::INVALID_PARAMS, format!("Unknown tool `{name}`")));
        };
        if let Err(refusal) = crate::mcp::guard::check_tool(&agent, tool) {
            return Ok(protocol::tool_refusal(refusal.message()));
        }

        let sessions = self.sessions.read().await;
        let mut clients = self.clients.write().await;
        let client = clients.entry(client_key.to_string()).or_default();

        let session_id = args.get("sessionId").and_then(|v| v.as_str()).unwrap_or("");

        match tool {
            crate::mcp::agents::ToolName::ListSessions => {
                Ok(protocol::tool_text(tools::list_sessions(&sessions).to_string()))
            }
            crate::mcp::agents::ToolName::DescribeSession => {
                match tools::describe_session(&sessions, client, session_id) {
                    Ok(v) => Ok(protocol::tool_text(v.to_string())),
                    Err(msg) => Ok(protocol::tool_refusal(msg)),
                }
            }
            crate::mcp::agents::ToolName::ReadOutput => {
                let lines = args.get("lines").and_then(|v| v.as_u64()).map(|n| n as usize);
                match tools::read_output(&sessions, client, session_id, lines) {
                    Ok(v) => Ok(protocol::tool_text(v.to_string())),
                    Err(msg) => Ok(protocol::tool_refusal(msg)),
                }
            }
            crate::mcp::agents::ToolName::SendInput => {
                let req = WriteRequest {
                    command: args.get("command").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    rationale: args
                        .get("rationale")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    references: args
                        .get("references")
                        .and_then(|v| v.as_array())
                        .map(|a| {
                            a.iter().filter_map(|s| s.as_str().map(str::to_string)).collect()
                        })
                        .unwrap_or_default(),
                    rollback: args
                        .get("rollback")
                        .and_then(|v| v.as_str())
                        .map(str::to_string),
                };

                let approved =
                    match tools::prepare_write(&agent, &sessions, client, session_id, &req) {
                        Ok(a) => a,
                        Err(refusal) => {
                            return Ok(tools::refusal_payload(
                                &refusal, &sessions, client, session_id,
                            ))
                        }
                    };

                let (host, kind) = sessions
                    .get(session_id)
                    .map(|s| (s.host.clone(), s.kind))
                    .unwrap_or_else(|| (String::new(), SessionKind::Ssh));

                // Locks are released before awaiting the human: a confirm
                // dialog can sit open for minutes, and holding the session lock
                // would freeze the terminal it is asking about.
                drop(clients);
                drop(sessions);

                let confirm = ConfirmRequest {
                    session_id: session_id.to_string(),
                    host,
                    command: req.command.clone(),
                    rationale: req.rationale.clone(),
                    references: req.references.clone(),
                    rollback: req.rollback.clone(),
                    agent_name: agent.name.clone(),
                    danger: format!("{:?}", approved.danger),
                    danger_reason: approved.danger_reason.clone(),
                };

                let confirmer = self.confirmer.lock().await.clone();
                let Some(confirmer) = confirmer else {
                    // Fails closed. No UI attached means nobody can approve, and
                    // approving on the user's behalf is exactly the thing this
                    // whole design exists to prevent.
                    return Ok(protocol::tool_refusal(
                        "Reach cannot ask the user to approve this right now, so it was not \
                         run. This is a safety default, not a bug — nothing executes without \
                         a human approving it."
                            .into(),
                    ));
                };

                if !confirmer(confirm).await {
                    return Ok(protocol::tool_refusal(
                        "The user rejected this command. Do not retry it or rephrase it to get \
                         a different answer — ask them what they would prefer instead."
                            .into(),
                    ));
                }

                // The write itself. This is what was missing: the gate
                // passed, the user approved, and the tool reported "sent"
                // without a byte reaching the terminal. Every test asserted
                // the gate; none asserted the act, because a unit test has
                // no terminal to assert on.
                let sender = self.sender.lock().await.clone();
                let Some(sender) = sender else {
                    return Ok(protocol::tool_refusal(
                        "Reach approved the command but has no way to type it into the \
                         session right now. Nothing ran."
                            .into(),
                    ));
                };

                let send = SendRequest {
                    session_id: session_id.to_string(),
                    kind,
                    command: req.command.clone(),
                    agent_name: agent.name.clone(),
                };

                if let Err(e) = sender(send).await {
                    return Ok(protocol::tool_refusal(format!(
                        "The user approved this, but writing to the session failed: {e}. \
                         Nothing ran — check the session is still connected."
                    )));
                }

                Ok(protocol::tool_text(
                    json!({
                        "status": "sent",
                        "note": "The command was approved by the user and typed into the \
                                 session. Call read_output to see what it did — the result is \
                                 not returned here, because output arrives asynchronously."
                    })
                    .to_string(),
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(method: &str, params: Value) -> Request {
        serde_json::from_value(json!({
            "jsonrpc": "2.0", "id": 1, "method": method, "params": params
        }))
        .unwrap()
    }

    #[tokio::test]
    async fn defaults_are_closed() {
        let s = McpState::default();
        assert!(!s.is_enabled(), "must not listen until the user enables it");
        assert!(s.token().is_none());
        assert!(s.agent().is_read_only(), "default agent must not be able to write");
        assert_eq!(s.shared_ids().await.len(), 0, "enabling exposes nothing");
    }

    #[tokio::test]
    async fn the_ai_cannot_change_the_agent() {
        let s = McpState::default();
        // There is no MCP method for it; the dispatcher rejects anything trying.
        let err = s.handle(req("agents/set", json!({ "id": "linux-administrator" }))).await;
        assert!(err.is_err());
        assert!(s.agent().is_read_only(), "still the restricted default");
    }

    #[tokio::test]
    async fn tool_list_follows_the_selected_agent() {
        let s = McpState::default();
        let v = s.handle(req("tools/list", json!({}))).await.unwrap();
        assert_eq!(v["tools"].as_array().unwrap().len(), 3, "architect default");

        s.set_agent("linux-administrator").unwrap();
        let v = s.handle(req("tools/list", json!({}))).await.unwrap();
        assert_eq!(v["tools"].as_array().unwrap().len(), 4);
    }

    #[tokio::test]
    async fn a_write_with_no_ui_attached_fails_closed() {
        let s = McpState::default();
        s.set_agent("linux-administrator").unwrap();
        s.share("s1".into(), SessionKind::Ssh, "db".into(), "root".into()).await;

        // Satisfy the preconditions.
        s.handle(req("tools/call", json!({
            "name": "describe_session", "arguments": { "sessionId": "s1" }
        }))).await.unwrap();

        let v = s.handle(req("tools/call", json!({
            "name": "send_input",
            "arguments": {
                "sessionId": "s1",
                "command": "uptime",
                "rationale": "checking how long the host has been up"
            }
        }))).await.unwrap();

        assert_eq!(v["isError"], json!(true));
        assert!(v["content"][0]["text"].as_str().unwrap().contains("safety default"));
    }

    #[tokio::test]
    async fn a_rejected_command_tells_the_model_not_to_rephrase() {
        let s = McpState::default();
        s.set_agent("linux-administrator").unwrap();
        s.share("s1".into(), SessionKind::Ssh, "db".into(), "root".into()).await;
        s.set_confirmer(Some(Arc::new(|_| Box::pin(async { false })))).await;
        let (sender, _typed) = recording_sender();
        s.set_sender(Some(sender)).await;

        s.handle(req("tools/call", json!({
            "name": "describe_session", "arguments": { "sessionId": "s1" }
        }))).await.unwrap();

        let v = s.handle(req("tools/call", json!({
            "name": "send_input",
            "arguments": {
                "sessionId": "s1", "command": "uptime",
                "rationale": "checking how long the host has been up"
            }
        }))).await.unwrap();

        let text = v["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("rejected"));
        assert!(text.contains("Do not retry"), "a rejection must not invite rephrasing: {text}");
    }

    /// Records what actually reached the terminal, so a test can assert the
    /// *act* and not merely the gate. Its absence is what let send_input ship
    /// twice reporting "sent" while writing nothing.
    fn recording_sender() -> (Sender, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
        let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = log.clone();
        let sender: Sender = Arc::new(move |req: SendRequest| {
            let sink = sink.clone();
            Box::pin(async move {
                sink.lock().unwrap().push(req.command);
                Ok(())
            })
        });
        (sender, log)
    }

    /// The test that was missing. Everything else asserted the guard; nothing
    /// asserted that an approved command is typed.
    #[tokio::test]
    async fn an_approved_command_is_actually_typed() {
        let s = McpState::default();
        s.set_agent("linux-administrator").unwrap();
        s.share("s1".into(), SessionKind::Ssh, "db".into(), "root".into()).await;
        s.set_confirmer(Some(Arc::new(|_| Box::pin(async { true })))).await;
        let (sender, typed) = recording_sender();
        s.set_sender(Some(sender)).await;

        s.handle(req("tools/call", json!({
            "name": "describe_session", "arguments": { "sessionId": "s1" }
        }))).await.unwrap();

        s.handle(req("tools/call", json!({
            "name": "send_input",
            "arguments": {
                "sessionId": "s1", "command": "uptime",
                "rationale": "checking how long the host has been up"
            }
        }))).await.unwrap();

        assert_eq!(typed.lock().unwrap().as_slice(), ["uptime"], "the command must reach the terminal");
    }

    /// A rejected command must not be typed. The reply already said so; now the
    /// terminal agrees.
    #[tokio::test]
    async fn a_rejected_command_is_never_typed() {
        let s = McpState::default();
        s.set_agent("linux-administrator").unwrap();
        s.share("s1".into(), SessionKind::Ssh, "db".into(), "root".into()).await;
        s.set_confirmer(Some(Arc::new(|_| Box::pin(async { false })))).await;
        let (sender, typed) = recording_sender();
        s.set_sender(Some(sender)).await;

        s.handle(req("tools/call", json!({
            "name": "describe_session", "arguments": { "sessionId": "s1" }
        }))).await.unwrap();
        s.handle(req("tools/call", json!({
            "name": "send_input",
            "arguments": {
                "sessionId": "s1", "command": "rm -rf /tmp/x",
                "rationale": "this should never be typed at all"
            }
        }))).await.unwrap();

        assert!(typed.lock().unwrap().is_empty(), "a rejection must not reach the terminal");
    }

    /// Approved, but nothing can type it: report the failure rather than
    /// claiming success, which is exactly the lie this whole area shipped with.
    #[tokio::test]
    async fn an_approved_command_with_no_sender_reports_failure() {
        let s = McpState::default();
        s.set_agent("linux-administrator").unwrap();
        s.share("s1".into(), SessionKind::Ssh, "db".into(), "root".into()).await;
        s.set_confirmer(Some(Arc::new(|_| Box::pin(async { true })))).await;
        // No sender installed.

        s.handle(req("tools/call", json!({
            "name": "describe_session", "arguments": { "sessionId": "s1" }
        }))).await.unwrap();
        let v = s.handle(req("tools/call", json!({
            "name": "send_input",
            "arguments": {
                "sessionId": "s1", "command": "uptime",
                "rationale": "checking how long the host has been up"
            }
        }))).await.unwrap();

        assert_eq!(v["isError"], json!(true), "must not report success when nothing was typed");
    }

    #[tokio::test]
    async fn an_approved_command_reports_that_output_comes_later() {
        let s = McpState::default();
        s.set_agent("linux-administrator").unwrap();
        s.share("s1".into(), SessionKind::Ssh, "db".into(), "root".into()).await;
        s.set_confirmer(Some(Arc::new(|_| Box::pin(async { true })))).await;
        let (sender, _typed) = recording_sender();
        s.set_sender(Some(sender)).await;

        s.handle(req("tools/call", json!({
            "name": "describe_session", "arguments": { "sessionId": "s1" }
        }))).await.unwrap();

        let v = s.handle(req("tools/call", json!({
            "name": "send_input",
            "arguments": {
                "sessionId": "s1", "command": "uptime",
                "rationale": "checking how long the host has been up"
            }
        }))).await.unwrap();

        assert_eq!(v["isError"], json!(false));
        assert!(v["content"][0]["text"].as_str().unwrap().contains("read_output"));
    }

    #[tokio::test]
    async fn a_read_only_agent_is_refused_the_write_tool() {
        let s = McpState::default(); // architect
        s.share("s1".into(), SessionKind::Ssh, "db".into(), "root".into()).await;
        let v = s.handle(req("tools/call", json!({
            "name": "send_input",
            "arguments": { "sessionId": "s1", "command": "ls", "rationale": "look at files" }
        }))).await.unwrap();
        assert_eq!(v["isError"], json!(true));
        assert!(v["content"][0]["text"].as_str().unwrap().contains("does not have"));
    }

    #[tokio::test]
    async fn unsharing_drops_the_preconditions() {
        let s = McpState::default();
        s.set_agent("linux-administrator").unwrap();
        s.share("s1".into(), SessionKind::Ssh, "db".into(), "root".into()).await;
        s.handle(req("tools/call", json!({
            "name": "describe_session", "arguments": { "sessionId": "s1" }
        }))).await.unwrap();

        s.unshare("s1").await;
        s.share("s1".into(), SessionKind::Ssh, "db".into(), "root".into()).await;
        s.set_confirmer(Some(Arc::new(|_| Box::pin(async { true })))).await;
        let (sender, _typed) = recording_sender();
        s.set_sender(Some(sender)).await;

        let v = s.handle(req("tools/call", json!({
            "name": "send_input",
            "arguments": {
                "sessionId": "s1", "command": "uptime",
                "rationale": "checking how long the host has been up"
            }
        }))).await.unwrap();
        assert_eq!(v["isError"], json!(true), "re-sharing must not reuse old consent");
    }

    #[tokio::test]
    async fn unknown_methods_are_reported_not_ignored() {
        let s = McpState::default();
        let err = s.handle(req("resources/list", json!({}))).await.unwrap_err();
        assert_eq!(err.0, protocol::METHOD_NOT_FOUND);
    }

    #[tokio::test]
    async fn output_only_accumulates_for_shared_sessions() {
        let s = McpState::default();
        // Not shared: must be a silent no-op, since read loops call it always.
        s.push_output("ghost", b"data").await;
        assert_eq!(s.session_count(), 0);

        s.share("s1".into(), SessionKind::Local, "local".into(), "me".into()).await;
        s.push_output("s1", b"hello").await;
        assert_eq!(s.session_count(), 1);
    }
}
