use crate::state::AppState;
use crate::ssh::client::{AuthParams, ConnectionInfo, exec_on_connection};

#[tauri::command]
pub async fn ssh_connect(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    id: String,
    host: String,
    port: u16,
    username: String,
    auth_method: String,
    password: Option<String>,
    key_path: Option<String>,
    key_passphrase: Option<String>,
    cols: u16,
    rows: u16,
) -> Result<String, String> {
    let auth = match auth_method.as_str() {
        "password" => AuthParams::Password(password.ok_or("Password required for password auth")?),
        "key" => AuthParams::Key {
            path: key_path.ok_or("Key path required for key auth")?,
            passphrase: key_passphrase,
        },
        "agent" => AuthParams::Agent,
        _ => return Err(format!("Unknown auth method: {}", auth_method)),
    };

    let mut manager = state.ssh_manager.lock().await;
    let info = manager.connect(&id, &host, port, &username, auth, cols, rows, app)
        .await
        .map_err(|e| e.to_string())?;

    Ok(info.id)
}

#[tauri::command]
pub async fn ssh_send(
    state: tauri::State<'_, AppState>,
    connection_id: String,
    data: Vec<u8>,
) -> Result<(), String> {
    let manager = state.ssh_manager.lock().await;
    manager.send_data(&connection_id, &data).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn ssh_resize(
    state: tauri::State<'_, AppState>,
    connection_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    let manager = state.ssh_manager.lock().await;
    manager.resize(&connection_id, cols, rows).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn ssh_disconnect(
    state: tauri::State<'_, AppState>,
    connection_id: String,
) -> Result<(), String> {
    let mut manager = state.ssh_manager.lock().await;
    manager.disconnect(&connection_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn ssh_list_connections(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ConnectionInfo>, String> {
    let manager = state.ssh_manager.lock().await;
    Ok(manager.list_connections())
}

/// Detect the remote operating system by parsing /etc/os-release.
/// Returns a lowercase distro ID (e.g. "debian", "ubuntu", "alpine")
/// or a fallback from uname -s (e.g. "darwin", "freebsd").
#[tauri::command]
pub async fn ssh_detect_os(
    state: tauri::State<'_, AppState>,
    connection_id: String,
) -> Result<String, String> {
    let handle = {
        let manager = state.ssh_manager.lock().await;
        manager.get_handle(&connection_id).map_err(|e| e.to_string())?
    };

    // Try /etc/os-release first (standard on modern Linux)
    if let Ok(output) = exec_on_connection(&handle, "cat /etc/os-release 2>/dev/null").await {
        for line in output.lines() {
            // Match the ID= line (not ID_LIKE=)
            if let Some(rest) = line.strip_prefix("ID=") {
                let id = rest.trim().trim_matches('"').to_lowercase();
                if !id.is_empty() {
                    return Ok(id);
                }
            }
        }
    }

    // Fallback: uname -s for non-Linux systems
    if let Ok(output) = exec_on_connection(&handle, "uname -s 2>/dev/null").await {
        let os = output.trim().to_lowercase();
        if !os.is_empty() {
            return Ok(os);
        }
    }

    Ok("linux".to_string())
}
