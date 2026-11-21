//! The listener.
//!
//! Bound to `127.0.0.1` and nothing else. Binding `0.0.0.0` would put every
//! shared root shell on the local network behind one bearer token, which is not
//! a trade anyone would make deliberately — so the address is not configurable.
//!
//! The socket does not exist until the user enables it. Not bound-and-empty:
//! there is no listener, nothing to scan, nothing to attach to. Enabling it
//! still exposes no sessions — that is a separate per-tab decision, and
//! `list_sessions` returns an empty array until the user makes it.
//!
//! Auth is a bearer token compared in constant time. Not because a timing
//! attack on loopback is likely, but because the alternative costs nothing and
//! the failure mode is somebody's production server.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::{DefaultBodyLimit, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};
use tokio::sync::oneshot;

use crate::mcp::protocol::{self, Request, Response};
use crate::mcp::McpState;

/// The largest request body accepted, in bytes. Calls are small JSON-RPC
/// messages (a command, a few parameters); a limit of our own rather than the
/// framework's default, so it is a decision written down here.
pub(crate) const MAX_BODY_BYTES: usize = 1024 * 1024;

/// The most calls one batch may carry. MCP clients send one call at a time;
/// batches are accepted because JSON-RPC allows them, not to be unbounded.
pub(crate) const MAX_BATCH: usize = 32;

/// Handle to a running server, so it can be shut down cleanly.
pub struct RunningServer {
    pub port: u16,
    shutdown: Option<oneshot::Sender<()>>,
}

impl RunningServer {
    pub fn stop(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
    }
}

impl Drop for RunningServer {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Start listening. Returns the bound port, which matters because port 0 asks
/// the OS to choose — the default, so a fresh install does not collide with
/// whatever else is running.
pub async fn start(state: Arc<McpState>, port: u16) -> Result<RunningServer, String> {
    let app = Router::new()
        .route("/mcp", post(handle_rpc))
        // A plain GET is what a human pastes into a browser to check it is up.
        // Answering with the setup snippet turns a confusing blank page into
        // the thing they were about to go looking for.
        .route("/mcp", get(handle_probe))
        .route("/health", get(handle_probe))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await.map_err(|e| {
        // The common failure is a port already in use. Say so plainly: a bare
        // OS error here turns into "MCP doesn't work" bug reports.
        if e.kind() == std::io::ErrorKind::AddrInUse {
            format!(
                "Port {port} is already in use. Choose another in Settings, or leave the port \
                 at 0 to let the system pick a free one."
            )
        } else {
            format!("Could not listen on 127.0.0.1:{port}: {e}")
        }
    })?;

    let bound = listener.local_addr().map_err(|e| e.to_string())?.port();
    let (tx, rx) = oneshot::channel::<()>();

    tokio::spawn(async move {
        let server = axum::serve(listener, app).with_graceful_shutdown(async {
            let _ = rx.await;
        });
        if let Err(e) = server.await {
            tracing::error!("MCP server stopped: {e}");
        }
    });

    tracing::info!("MCP server listening on 127.0.0.1:{bound}");
    Ok(RunningServer { port: bound, shutdown: Some(tx) })
}

/// Constant-time comparison, so a wrong token leaks nothing by how long it took.
fn token_matches(expected: &str, given: &str) -> bool {
    if expected.len() != given.len() {
        return false;
    }
    let mut diff = 0u8;
    for (a, b) in expected.bytes().zip(given.bytes()) {
        diff |= a ^ b;
    }
    diff == 0
}

fn authorized(state: &McpState, headers: &HeaderMap) -> bool {
    let Some(expected) = state.token() else {
        // No token means the server should not be running at all.
        return false;
    };
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .is_some_and(|given| token_matches(&expected, given.trim()))
}

async fn handle_probe(State(state): State<Arc<McpState>>) -> impl IntoResponse {
    // Deliberately does not include the token: this endpoint is unauthenticated
    // so that "is it up?" is answerable, and printing the credential on an
    // unauthenticated route would defeat having one.
    Json(json!({
        "server": protocol::SERVER_NAME,
        "version": env!("CARGO_PKG_VERSION"),
        "protocolVersion": protocol::PROTOCOL_VERSION,
        "status": "listening",
        "sharedSessions": state.session_count(),
        "hint": "POST JSON-RPC to /mcp with an Authorization: Bearer <token> header. \
                 The token is shown in Reach under Settings > AI."
    }))
}

async fn handle_rpc(
    State(state): State<Arc<McpState>>,
    headers: HeaderMap,
    body: String,
) -> impl IntoResponse {
    if !authorized(&state, &headers) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "error": "Missing or invalid bearer token. Reach shows the current token under \
                          Settings > AI; it changes when the server is restarted."
            })),
        )
            .into_response();
    }

    // A batch is a JSON array. Handled because the spec allows it and a client
    // that sends one should not get a parse error.
    let parsed: Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => {
            return Json(Response::err(
                Value::Null,
                protocol::PARSE_ERROR,
                format!("Invalid JSON: {e}"),
            ))
            .into_response()
        }
    };

    match parsed {
        Value::Array(items) if items.len() > MAX_BATCH => Json(Response::err(
            Value::Null,
            protocol::INVALID_REQUEST,
            format!("A batch may hold at most {MAX_BATCH} calls; this one has {}.", items.len()),
        ))
        .into_response(),
        Value::Array(items) => {
            let mut out = Vec::new();
            for item in items {
                if let Some(r) = dispatch_value(&state, item).await {
                    out.push(r);
                }
            }
            // An all-notification batch expects no body at all.
            if out.is_empty() {
                StatusCode::ACCEPTED.into_response()
            } else {
                Json(out).into_response()
            }
        }
        single => match dispatch_value(&state, single).await {
            Some(r) => Json(r).into_response(),
            None => StatusCode::ACCEPTED.into_response(),
        },
    }
}

async fn dispatch_value(state: &Arc<McpState>, value: Value) -> Option<Response> {
    let req: Request = match serde_json::from_value(value) {
        Ok(r) => r,
        Err(e) => {
            return Some(Response::err(
                Value::Null,
                protocol::INVALID_REQUEST,
                format!("Not a JSON-RPC request: {e}"),
            ))
        }
    };

    let is_note = req.is_notification();
    let id = req.id.clone().unwrap_or(Value::Null);
    let result = state.handle(req).await;

    // Notifications get no reply, even on error.
    if is_note {
        return None;
    }

    Some(match result {
        Ok(v) => Response::ok(id, v),
        Err((code, msg)) => Response::err(id, code, msg),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_comparison_rejects_the_obvious_cases() {
        assert!(token_matches("abc123", "abc123"));
        assert!(!token_matches("abc123", "abc124"));
        assert!(!token_matches("abc123", "abc12"), "length differs");
        assert!(!token_matches("abc123", ""), "empty");
        assert!(!token_matches("", "abc"), "empty expected");
    }

    /// The comparison must not stop early on the first differing byte.
    #[test]
    fn token_comparison_examines_the_whole_string() {
        // Same length, differing only in the last byte: a short-circuiting
        // implementation would return after one byte for the first case and
        // after all of them for the second. Both must simply be false.
        assert!(!token_matches("aaaaaaaa", "baaaaaaa"));
        assert!(!token_matches("aaaaaaaa", "aaaaaaab"));
    }

    fn headers_with(auth: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert("authorization", auth.parse().unwrap());
        h
    }

    #[test]
    fn authorization_requires_the_bearer_scheme_and_the_right_token() {
        let state = McpState::default();
        state.set_token(Some("s3cr3t".into()));

        assert!(authorized(&state, &headers_with("Bearer s3cr3t")));
        assert!(authorized(&state, &headers_with("Bearer  s3cr3t ")), "surrounding space tolerated");
        assert!(!authorized(&state, &headers_with("Bearer wrong")));
        assert!(!authorized(&state, &headers_with("s3cr3t")), "scheme required");
        assert!(!authorized(&state, &headers_with("Basic s3cr3t")));
        assert!(!authorized(&state, &HeaderMap::new()), "no header at all");
    }

    async fn body_of(r: axum::response::Response) -> (StatusCode, Value) {
        use http_body_util::BodyExt;
        let status = r.status();
        let bytes = r.into_body().collect().await.unwrap().to_bytes();
        (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
    }

    #[tokio::test]
    async fn a_batch_larger_than_the_cap_is_refused_whole() {
        let state = Arc::new(McpState::default());
        state.set_token(Some("t".into()));
        let call = json!({"jsonrpc": "2.0", "id": 1, "method": "ping"});
        let batch = Value::Array(vec![call; MAX_BATCH + 1]).to_string();
        let r = handle_rpc(State(state), headers_with("Bearer t"), batch).await.into_response();
        let (status, body) = body_of(r).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["error"]["code"], protocol::INVALID_REQUEST);
        assert!(body["error"]["message"].as_str().unwrap().contains("at most"));
    }

    #[tokio::test]
    async fn a_batch_at_the_cap_is_answered() {
        let state = Arc::new(McpState::default());
        state.set_token(Some("t".into()));
        let call = json!({"jsonrpc": "2.0", "id": 1, "method": "ping"});
        let batch = Value::Array(vec![call; MAX_BATCH]).to_string();
        let r = handle_rpc(State(state), headers_with("Bearer t"), batch).await.into_response();
        let (_, body) = body_of(r).await;
        assert_eq!(body.as_array().map(Vec::len), Some(MAX_BATCH));
    }

    /// The limit is checked on the running server, over a real socket, since
    /// it is applied by the router and not by the handler.
    #[tokio::test]
    async fn a_body_over_the_limit_is_refused_before_it_is_parsed() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let state = Arc::new(McpState::default());
        state.set_token(Some("t".into()));
        let mut server = start(state, 0).await.unwrap();
        let mut sock = tokio::net::TcpStream::connect(("127.0.0.1", server.port)).await.unwrap();
        let body = vec![b' '; MAX_BODY_BYTES + 1];
        let head = format!(
            "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer t\r\n\
             Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        sock.write_all(head.as_bytes()).await.unwrap();
        // The server may answer and close before reading it all.
        let _ = sock.write_all(&body).await;
        let mut reply = Vec::new();
        let _ = sock.read_to_end(&mut reply).await;
        let reply = String::from_utf8_lossy(&reply);
        assert!(reply.starts_with("HTTP/1.1 413"), "got: {}", reply.lines().next().unwrap_or(""));
        server.stop();
    }

    /// With no token configured the server must refuse everything, rather than
    /// falling open to an empty comparison.
    #[test]
    fn no_token_means_nothing_is_authorized() {
        let state = McpState::default();
        state.set_token(None);
        assert!(!authorized(&state, &headers_with("Bearer anything")));
        assert!(!authorized(&state, &headers_with("Bearer ")));
    }
}
