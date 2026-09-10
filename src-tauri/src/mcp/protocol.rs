//! JSON-RPC 2.0 and the MCP envelope.
//!
//! # Transport choice
//!
//! **Streamable HTTP**, one endpoint, POST per request, keep-alive.
//!
//! stdio is the lowest-overhead MCP transport, but it requires the *client* to
//! spawn the server. Reach is a long-running desktop app that already owns the
//! sessions; nobody is going to launch a second copy of it per AI client. The
//! older HTTP+SSE transport splits request and response across two connections
//! for no benefit here. Streamable HTTP is what current clients speak, and it
//! is a plain request/response over loopback.
//!
//! # What actually makes this fast
//!
//! Not the transport. A JSON-RPC round trip on loopback is a fraction of a
//! millisecond; model inference is three orders of magnitude larger. Choosing
//! stdio over HTTP would optimise 0.1% of the latency.
//!
//! What costs real time is **round trips**, and the protocol is shaped to
//! remove them:
//!
//! * `describe_session` answers everything about a session in one call, so a
//!   model never needs a follow-up to learn the shell or whether it is root.
//! * `read_output` returns the sequence number, the truncation state and
//!   whether a secret prompt is up — so the next `send_input` has its
//!   preconditions already satisfied.
//! * **Refusals carry their own remedy.** A stale-view refusal returns the
//!   output the client missed, so the fix does not cost an extra call. This is
//!   the single biggest saving: the common failure path becomes zero round
//!   trips instead of two.
//!
//! A careless client is also a slow client, and the same design that keeps it
//! safe keeps it quick.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// The MCP revision this server implements.
pub const PROTOCOL_VERSION: &str = "2025-06-18";

/// Older revision still spoken by some clients. Accepted so a client that
/// negotiates it is not turned away for a version string.
pub const PROTOCOL_VERSION_FALLBACK: &str = "2025-03-26";

pub const SERVER_NAME: &str = "reach";

#[derive(Debug, Clone, Deserialize)]
pub struct Request {
    #[allow(dead_code)]
    pub jsonrpc: String,
    /// Absent for notifications, which expect no reply.
    #[serde(default)]
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

impl Request {
    pub fn is_notification(&self) -> bool {
        self.id.is_none()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Response {
    pub jsonrpc: &'static str,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

// JSON-RPC 2.0 reserved codes.
pub const PARSE_ERROR: i32 = -32700;
pub const INVALID_REQUEST: i32 = -32600;
pub const METHOD_NOT_FOUND: i32 = -32601;
pub const INVALID_PARAMS: i32 = -32602;
pub const INTERNAL_ERROR: i32 = -32603;

impl Response {
    pub fn ok(id: Value, result: Value) -> Self {
        Self { jsonrpc: "2.0", id, result: Some(result), error: None }
    }

    pub fn err(id: Value, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(RpcError { code, message: message.into(), data: None }),
        }
    }
}

/// A tool refusal is **not** a JSON-RPC error.
///
/// This distinction matters more than it looks. A transport-level error tells a
/// client "the call was malformed"; many clients surface that to the user and
/// stop. What the guard produces is a *result* — the tool ran, considered the
/// request, and declined — which the model is expected to read and act on.
/// Returning `isError: true` inside a normal result is what keeps a refusal in
/// the model's loop instead of aborting it.
pub fn tool_refusal(message: String) -> Value {
    json!({
        "content": [{ "type": "text", "text": message }],
        "isError": true
    })
}

/// A successful tool result.
pub fn tool_text(text: String) -> Value {
    json!({
        "content": [{ "type": "text", "text": text }],
        "isError": false
    })
}

/// The `initialize` reply. Declares only what is actually implemented: a client
/// that sees `resources` advertised will call `resources/list`, and answering
/// "method not found" to a capability we announced is worse than not
/// announcing it. Skills will add `resources` when they land.
pub fn initialize_result(client_version: Option<&str>) -> Value {
    // Echo the client's version when we can speak it, per the negotiation rule.
    let version = match client_version {
        Some(v) if v == PROTOCOL_VERSION || v == PROTOCOL_VERSION_FALLBACK => v,
        _ => PROTOCOL_VERSION,
    };
    json!({
        "protocolVersion": version,
        "capabilities": {
            "tools": { "listChanged": true },
            "prompts": { "listChanged": true }
        },
        "serverInfo": {
            "name": SERVER_NAME,
            "version": env!("CARGO_PKG_VERSION")
        },
        "instructions":
            "These tools operate on REAL terminal sessions on machines the user \
             depends on. Preconditions are enforced by the server, not merely \
             requested: call describe_session before acting on a session, call \
             read_output immediately before every send_input, and supply a \
             rationale that a human will read before approving. Refusals explain \
             exactly what to do next — read them rather than retrying."
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_request_and_a_notification() {
        let req: Request =
            serde_json::from_str(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#).unwrap();
        assert!(!req.is_notification());
        assert_eq!(req.method, "tools/list");

        let note: Request =
            serde_json::from_str(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
                .unwrap();
        assert!(note.is_notification(), "no id means no reply is expected");
    }

    #[test]
    fn a_missing_params_field_is_not_an_error() {
        // Clients omit `params` for no-argument methods; requiring it would
        // reject perfectly valid calls.
        let req: Request =
            serde_json::from_str(r#"{"jsonrpc":"2.0","id":2,"method":"ping"}"#).unwrap();
        assert!(req.params.is_null());
    }

    #[test]
    fn responses_omit_the_unused_half() {
        let ok = serde_json::to_value(Response::ok(json!(1), json!({"a":1}))).unwrap();
        assert!(ok.get("result").is_some());
        assert!(ok.get("error").is_none(), "a success must not carry a null error");

        let err = serde_json::to_value(Response::err(json!(1), METHOD_NOT_FOUND, "nope")).unwrap();
        assert!(err.get("error").is_some());
        assert!(err.get("result").is_none());
    }

    /// A refusal has to stay inside the model's loop. Sending it as a
    /// JSON-RPC error makes clients abort instead of reading the remedy.
    #[test]
    fn a_refusal_is_a_result_not_a_transport_error() {
        let v = tool_refusal("Call read_output first.".into());
        assert_eq!(v["isError"], json!(true));
        assert_eq!(v["content"][0]["type"], json!("text"));
        assert!(v["content"][0]["text"].as_str().unwrap().contains("read_output"));
    }

    #[test]
    fn initialize_echoes_a_version_we_can_speak() {
        let v = initialize_result(Some(PROTOCOL_VERSION_FALLBACK));
        assert_eq!(v["protocolVersion"], json!(PROTOCOL_VERSION_FALLBACK));

        let v = initialize_result(Some("1999-01-01"));
        assert_eq!(v["protocolVersion"], json!(PROTOCOL_VERSION), "unknown → offer ours");

        let v = initialize_result(None);
        assert_eq!(v["protocolVersion"], json!(PROTOCOL_VERSION));
    }

    /// Announcing a capability we do not implement makes clients call methods
    /// we answer with "method not found", which reads as a broken server.
    #[test]
    fn only_implemented_capabilities_are_announced() {
        let v = initialize_result(None);
        let caps = &v["capabilities"];
        assert!(caps.get("tools").is_some());
        assert!(caps.get("prompts").is_some());
        assert!(caps.get("resources").is_none(), "resources arrives with skills");
    }

    #[test]
    fn initialize_reports_the_real_app_version() {
        let v = initialize_result(None);
        assert_eq!(v["serverInfo"]["version"], json!(env!("CARGO_PKG_VERSION")));
        assert_eq!(v["serverInfo"]["name"], json!("reach"));
    }

    /// The instructions block is the one piece of text every client reads
    /// before doing anything, so the enforced rules belong in it.
    #[test]
    fn instructions_state_the_enforced_preconditions() {
        let v = initialize_result(None);
        let text = v["instructions"].as_str().unwrap();
        for needle in ["describe_session", "read_output", "rationale"] {
            assert!(text.contains(needle), "instructions omit {needle}");
        }
    }
}
