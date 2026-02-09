use crate::plugin::schema::HookEvent;

// Hook event name constants
pub const HOOK_SESSION_CONNECTED: &str = "session:connected";
pub const HOOK_SESSION_DISCONNECTED: &str = "session:disconnected";
pub const HOOK_SFTP_UPLOAD_COMPLETE: &str = "sftp:upload_complete";
pub const HOOK_SFTP_DOWNLOAD_COMPLETE: &str = "sftp:download_complete";
pub const HOOK_TUNNEL_STARTED: &str = "tunnel:started";
pub const HOOK_TUNNEL_STOPPED: &str = "tunnel:stopped";
pub const HOOK_VAULT_UNLOCKED: &str = "vault:unlocked";
pub const HOOK_VAULT_LOCKED: &str = "vault:locked";

pub fn session_connected(connection_id: &str, host: &str, username: &str) -> HookEvent {
    HookEvent {
        event_name: HOOK_SESSION_CONNECTED.to_string(),
        data: serde_json::json!({
            "connectionId": connection_id,
            "host": host,
            "username": username,
        }),
    }
}

pub fn session_disconnected(connection_id: &str) -> HookEvent {
    HookEvent {
        event_name: HOOK_SESSION_DISCONNECTED.to_string(),
        data: serde_json::json!({
            "connectionId": connection_id,
        }),
    }
}

pub fn sftp_upload_complete(connection_id: &str, remote_path: &str) -> HookEvent {
    HookEvent {
        event_name: HOOK_SFTP_UPLOAD_COMPLETE.to_string(),
        data: serde_json::json!({
            "connectionId": connection_id,
            "remotePath": remote_path,
        }),
    }
}

pub fn sftp_download_complete(connection_id: &str, remote_path: &str, local_path: &str) -> HookEvent {
    HookEvent {
        event_name: HOOK_SFTP_DOWNLOAD_COMPLETE.to_string(),
        data: serde_json::json!({
            "connectionId": connection_id,
            "remotePath": remote_path,
            "localPath": local_path,
        }),
    }
}

pub fn tunnel_started(tunnel_id: &str, local_port: u16) -> HookEvent {
    HookEvent {
        event_name: HOOK_TUNNEL_STARTED.to_string(),
        data: serde_json::json!({
            "tunnelId": tunnel_id,
            "localPort": local_port,
        }),
    }
}

pub fn tunnel_stopped(tunnel_id: &str) -> HookEvent {
    HookEvent {
        event_name: HOOK_TUNNEL_STOPPED.to_string(),
        data: serde_json::json!({
            "tunnelId": tunnel_id,
        }),
    }
}
