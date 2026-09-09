// Standalone smoke test: start the real server, speak real MCP to it over TCP.
use reach_lib::mcp::{server, McpState};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let state = Arc::new(McpState::default());
    state.set_token(Some("test-token".into()));
    state.set_agent("linux-administrator").unwrap();
    state.share("s1".into(), reach_lib::mcp::session::SessionKind::Ssh,
                "db.internal".into(), "root".into()).await;
    state.push_output("s1", b"root@db:~# uptime\n 21:04:11 up 3 days\nroot@db:~# ").await;

    let srv = server::start(state.clone(), 0).await.expect("start");
    let url = format!("http://127.0.0.1:{}/mcp", srv.port);
    let c = reqwest::Client::new();

    let call = |m: &str, p: serde_json::Value| {
        let (c, url) = (c.clone(), url.clone());
        let m = m.to_string();
        async move {
            c.post(&url).header("Authorization", "Bearer test-token")
                .json(&serde_json::json!({"jsonrpc":"2.0","id":1,"method":m,"params":p}))
                .send().await.unwrap().json::<serde_json::Value>().await.unwrap()
        }
    };

    // 1. unauthorized
    let code = c.post(&url).json(&serde_json::json!({"jsonrpc":"2.0","id":1,"method":"ping"}))
        .send().await.unwrap().status();
    println!("no token          -> HTTP {}", code);

    // 2. initialize
    let v = call("initialize", serde_json::json!({"protocolVersion":"2025-06-18"})).await;
    println!("initialize        -> {} v{}", v["result"]["serverInfo"]["name"], v["result"]["serverInfo"]["version"]);

    // 3. tools/list
    let v = call("tools/list", serde_json::json!({})).await;
    let names: Vec<&str> = v["result"]["tools"].as_array().unwrap().iter()
        .map(|t| t["name"].as_str().unwrap()).collect();
    println!("tools/list        -> {:?}", names);

    // 4. list_sessions
    let v = call("tools/call", serde_json::json!({"name":"list_sessions","arguments":{}})).await;
    println!("list_sessions     -> {}", v["result"]["content"][0]["text"].as_str().unwrap());

    // 5. send_input without describe -> refusal that teaches
    let v = call("tools/call", serde_json::json!({"name":"send_input","arguments":{
        "sessionId":"s1","command":"uptime","rationale":"checking system uptime now"}})).await;
    println!("write undescribed -> isError={} :: {}", v["result"]["isError"],
        &v["result"]["content"][0]["text"].as_str().unwrap()[..70]);

    // 6. describe, then write -> reaches the confirm gate (no UI = fails closed)
    call("tools/call", serde_json::json!({"name":"describe_session","arguments":{"sessionId":"s1"}})).await;
    let v = call("tools/call", serde_json::json!({"name":"send_input","arguments":{
        "sessionId":"s1","command":"uptime","rationale":"checking system uptime now"}})).await;
    println!("write described   -> isError={} :: {}", v["result"]["isError"],
        &v["result"]["content"][0]["text"].as_str().unwrap()[..60]);
}
