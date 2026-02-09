use crate::state::AppState;
use crate::ssh::client::{AuthParams, ConnectionInfo, JumpHostParams, exec_on_connection};
use crate::plugin::hooks;

/// Parameters for a jump host received from the frontend.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JumpHostConnectParams {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_method: String,
    pub password: Option<String>,
    pub key_path: Option<String>,
    pub key_passphrase: Option<String>,
}

fn build_auth(
    auth_method: &str,
    password: Option<String>,
    key_path: Option<String>,
    key_passphrase: Option<String>,
) -> Result<AuthParams, String> {
    match auth_method {
        "password" => Ok(AuthParams::Password(
            password.ok_or("Password required for password auth")?,
        )),
        "key" => Ok(AuthParams::Key {
            path: key_path.ok_or("Key path required for key auth")?,
            passphrase: key_passphrase,
        }),
        "agent" => Ok(AuthParams::Agent),
        _ => Err(format!("Unknown auth method: {}", auth_method)),
    }
}

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
    jump_chain: Option<Vec<JumpHostConnectParams>>,
) -> Result<String, String> {
    let auth = build_auth(&auth_method, password, key_path, key_passphrase)?;

    let mut manager = state.ssh_manager.lock().await;

    let info = if let Some(chain) = jump_chain {
        if chain.is_empty() {
            // No jump hosts, connect directly
            manager
                .connect(&id, &host, port, &username, auth, cols, rows, app.clone())
                .await
                .map_err(|e| e.to_string())?
        } else {
            // Build jump host params
            let jump_params: Result<Vec<JumpHostParams>, String> = chain
                .into_iter()
                .map(|j| {
                    let jauth = build_auth(
                        &j.auth_method,
                        j.password,
                        j.key_path,
                        j.key_passphrase,
                    )?;
                    Ok(JumpHostParams {
                        host: j.host,
                        port: j.port,
                        username: j.username,
                        auth: jauth,
                    })
                })
                .collect();

            manager
                .connect_via_jump(
                    &id,
                    &host,
                    port,
                    &username,
                    auth,
                    jump_params?,
                    cols,
                    rows,
                    app.clone(),
                )
                .await
                .map_err(|e| e.to_string())?
        }
    } else {
        manager
            .connect(&id, &host, port, &username, auth, cols, rows, app.clone())
            .await
            .map_err(|e| e.to_string())?
    };

    let connection_id = info.id.clone();

    // Drop ssh_manager lock BEFORE dispatching plugin hooks
    // (plugins may call reach.ssh.exec which needs the lock)
    drop(manager);

    // Dispatch plugin hook
    let hook = hooks::session_connected(&connection_id, &host, &username);
    let mut plugin_mgr = state.plugin_manager.lock().await;
    plugin_mgr.dispatch_hook(&hook, Some(&app)).await;

    Ok(connection_id)
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
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    connection_id: String,
) -> Result<(), String> {
    let mut manager = state.ssh_manager.lock().await;
    manager.disconnect(&connection_id).map_err(|e| e.to_string())?;
    drop(manager);

    // Dispatch plugin hook
    let hook = hooks::session_disconnected(&connection_id);
    let mut plugin_mgr = state.plugin_manager.lock().await;
    plugin_mgr.dispatch_hook(&hook, Some(&app)).await;

    Ok(())
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
