use std::collections::HashSet;
use std::sync::Arc;

use mlua::{Lua, Result as LuaResult, UserDataRef};
use tauri::Emitter;

use crate::plugin::schema::{PluginPermission, UiElement};
use crate::ssh::client::SshManager;
use crate::tunnel::manager::TunnelManager;
use crate::vault::VaultManager;

/// Shared reference to app state passed into Lua via userdata.
#[derive(Clone)]
pub struct PluginAppState {
    pub ssh_manager: Arc<tokio::sync::Mutex<SshManager>>,
    pub tunnel_manager: Arc<tokio::sync::Mutex<TunnelManager>>,
    pub vault_manager: Arc<tokio::sync::Mutex<VaultManager>>,
    pub app_handle: Option<tauri::AppHandle>,
    pub plugin_id: String,
}

impl mlua::UserData for PluginAppState {}

/// Inject the `reach` API table into the Lua VM, gated by granted permissions.
pub fn inject_host_api(
    lua: &Lua,
    granted: &HashSet<PluginPermission>,
    app_state: PluginAppState,
) -> LuaResult<()> {
    // Store app state as userdata in Lua registry
    lua.set_named_registry_value("__plugin_app_state", app_state)?;

    let reach = lua.create_table()?;

    // reach.log is always available
    inject_log_api(lua, &reach)?;

    if granted.contains(&PluginPermission::SshExec)
        || granted.contains(&PluginPermission::SshListConnections)
    {
        inject_ssh_api(lua, &reach, granted)?;
    }

    if granted.contains(&PluginPermission::SftpList)
        || granted.contains(&PluginPermission::SftpRead)
        || granted.contains(&PluginPermission::SftpWrite)
    {
        inject_sftp_api(lua, &reach, granted)?;
    }

    if granted.contains(&PluginPermission::VaultRead)
        || granted.contains(&PluginPermission::VaultWrite)
    {
        inject_vault_api(lua, &reach, granted)?;
    }

    if granted.contains(&PluginPermission::TunnelManage) {
        inject_tunnel_api(lua, &reach)?;
    }

    if granted.contains(&PluginPermission::Http) {
        inject_http_api(lua, &reach)?;
    }

    if granted.contains(&PluginPermission::Notify) {
        inject_notify_api(lua, &reach)?;
    }

    if granted.contains(&PluginPermission::Ui) {
        inject_ui_api(lua, &reach)?;
    }

    lua.globals().set("reach", reach)?;
    Ok(())
}

fn inject_log_api(lua: &Lua, reach: &mlua::Table) -> LuaResult<()> {
    let log = lua.create_table()?;

    log.set(
        "info",
        lua.create_function(|_, msg: String| {
            tracing::info!("[plugin] {}", msg);
            Ok(())
        })?,
    )?;

    log.set(
        "warn",
        lua.create_function(|_, msg: String| {
            tracing::warn!("[plugin] {}", msg);
            Ok(())
        })?,
    )?;

    log.set(
        "error",
        lua.create_function(|_, msg: String| {
            tracing::error!("[plugin] {}", msg);
            Ok(())
        })?,
    )?;

    reach.set("log", log)?;
    Ok(())
}

fn inject_ssh_api(
    lua: &Lua,
    reach: &mlua::Table,
    granted: &HashSet<PluginPermission>,
) -> LuaResult<()> {
    let ssh = lua.create_table()?;

    if granted.contains(&PluginPermission::SshExec) {
        ssh.set(
            "exec",
            lua.create_async_function(|lua, (conn_id, cmd): (String, String)| async move {
                let state: UserDataRef<PluginAppState> =
                    lua.named_registry_value("__plugin_app_state")?;
                let handle = {
                    let manager = state.ssh_manager.lock().await;
                    manager
                        .get_handle(&conn_id)
                        .map_err(|e| mlua::Error::external(e.to_string()))?
                };
                let output =
                    crate::ssh::client::exec_on_connection(&handle, &cmd)
                        .await
                        .map_err(|e| mlua::Error::external(e.to_string()))?;
                Ok(output)
            })?,
        )?;
    }

    if granted.contains(&PluginPermission::SshListConnections) {
        ssh.set(
            "list_connections",
            lua.create_async_function(|lua, ()| async move {
                let state: UserDataRef<PluginAppState> =
                    lua.named_registry_value("__plugin_app_state")?;
                let manager = state.ssh_manager.lock().await;
                let conns = manager.list_connections();
                let table = lua.create_table()?;
                for (i, conn) in conns.iter().enumerate() {
                    let entry = lua.create_table()?;
                    entry.set("id", conn.id.clone())?;
                    entry.set("host", conn.host.clone())?;
                    entry.set("port", conn.port)?;
                    entry.set("username", conn.username.clone())?;
                    table.set(i + 1, entry)?;
                }
                Ok(table)
            })?,
        )?;
    }

    reach.set("ssh", ssh)?;
    Ok(())
}

fn inject_sftp_api(
    lua: &Lua,
    reach: &mlua::Table,
    granted: &HashSet<PluginPermission>,
) -> LuaResult<()> {
    let sftp = lua.create_table()?;

    if granted.contains(&PluginPermission::SftpList) {
        sftp.set(
            "list",
            lua.create_async_function(|lua, (conn_id, path): (String, String)| async move {
                let state: UserDataRef<PluginAppState> =
                    lua.named_registry_value("__plugin_app_state")?;
                let handle = {
                    let manager = state.ssh_manager.lock().await;
                    manager
                        .get_handle(&conn_id)
                        .map_err(|e| mlua::Error::external(e.to_string()))?
                };
                let entries = crate::sftp::browser::list_directory(&handle, &path)
                    .await
                    .map_err(|e| mlua::Error::external(e.to_string()))?;
                let table = lua.create_table()?;
                for (i, entry) in entries.iter().enumerate() {
                    let e = lua.create_table()?;
                    e.set("name", entry.name.clone())?;
                    e.set("path", entry.path.clone())?;
                    e.set("isDirectory", entry.is_dir)?;
                    e.set("size", entry.size)?;
                    table.set(i + 1, e)?;
                }
                Ok(table)
            })?,
        )?;
    }

    if granted.contains(&PluginPermission::SftpRead) {
        sftp.set(
            "read",
            lua.create_async_function(|lua, (conn_id, path): (String, String)| async move {
                let state: UserDataRef<PluginAppState> =
                    lua.named_registry_value("__plugin_app_state")?;
                let handle = {
                    let manager = state.ssh_manager.lock().await;
                    manager
                        .get_handle(&conn_id)
                        .map_err(|e| mlua::Error::external(e.to_string()))?
                };
                let content = crate::sftp::browser::read_text_file(&handle, &path)
                    .await
                    .map_err(|e| mlua::Error::external(e.to_string()))?;
                Ok(content)
            })?,
        )?;
    }

    if granted.contains(&PluginPermission::SftpWrite) {
        sftp.set(
            "write",
            lua.create_async_function(
                |lua, (conn_id, path, content): (String, String, String)| async move {
                    let state: UserDataRef<PluginAppState> =
                        lua.named_registry_value("__plugin_app_state")?;
                    let handle = {
                        let manager = state.ssh_manager.lock().await;
                        manager
                            .get_handle(&conn_id)
                            .map_err(|e| mlua::Error::external(e.to_string()))?
                    };
                    crate::sftp::browser::write_text_file(&handle, &path, &content)
                        .await
                        .map_err(|e| mlua::Error::external(e.to_string()))?;
                    Ok(())
                },
            )?,
        )?;
    }

    reach.set("sftp", sftp)?;
    Ok(())
}

fn inject_vault_api(
    lua: &Lua,
    reach: &mlua::Table,
    granted: &HashSet<PluginPermission>,
) -> LuaResult<()> {
    let vault = lua.create_table()?;

    if granted.contains(&PluginPermission::VaultRead) {
        vault.set(
            "list",
            lua.create_async_function(|lua, ()| async move {
                let state: UserDataRef<PluginAppState> =
                    lua.named_registry_value("__plugin_app_state")?;
                let manager = state.vault_manager.lock().await;
                let vaults = manager.list_vaults().await
                    .map_err(|e| mlua::Error::external(e.to_string()))?;
                let table = lua.create_table()?;
                for (i, v) in vaults.iter().enumerate() {
                    let entry = lua.create_table()?;
                    entry.set("id", v.id.clone())?;
                    entry.set("name", v.name.clone())?;
                    table.set(i + 1, entry)?;
                }
                Ok(table)
            })?,
        )?;
    }

    reach.set("vault", vault)?;
    Ok(())
}

fn inject_tunnel_api(lua: &Lua, reach: &mlua::Table) -> LuaResult<()> {
    let tunnel = lua.create_table()?;

    tunnel.set(
        "list",
        lua.create_async_function(|lua, ()| async move {
            // Returns an empty table -- actual tunnel state is in AppState.tunnels RwLock
            // which we don't have direct access to from the plugin API.
            // This is a stub for now; can be expanded if needed.
            let table = lua.create_table()?;
            Ok(table)
        })?,
    )?;

    reach.set("tunnel", tunnel)?;
    Ok(())
}

fn inject_http_api(lua: &Lua, reach: &mlua::Table) -> LuaResult<()> {
    let http = lua.create_table()?;

    http.set(
        "get",
        lua.create_async_function(|_, url: String| async move {
            let resp = reqwest::get(&url)
                .await
                .map_err(|e| mlua::Error::external(e.to_string()))?;
            let body = resp
                .text()
                .await
                .map_err(|e| mlua::Error::external(e.to_string()))?;
            Ok(body)
        })?,
    )?;

    http.set(
        "post",
        lua.create_async_function(|_, (url, body): (String, String)| async move {
            let client = reqwest::Client::new();
            let resp = client
                .post(&url)
                .header("Content-Type", "application/json")
                .body(body)
                .send()
                .await
                .map_err(|e| mlua::Error::external(e.to_string()))?;
            let text = resp
                .text()
                .await
                .map_err(|e| mlua::Error::external(e.to_string()))?;
            Ok(text)
        })?,
    )?;

    reach.set("http", http)?;
    Ok(())
}

fn inject_notify_api(lua: &Lua, reach: &mlua::Table) -> LuaResult<()> {
    let ui_table = lua.create_table()?;

    ui_table.set(
        "notify",
        lua.create_async_function(|lua, (msg, level): (String, Option<String>)| async move {
            let state: UserDataRef<PluginAppState> =
                lua.named_registry_value("__plugin_app_state")?;
            let level = level.unwrap_or_else(|| "info".to_string());
            if let Some(ref handle) = state.app_handle {
                let _ = handle.emit(
                    "plugin-notify",
                    serde_json::json!({
                        "pluginId": state.plugin_id,
                        "message": msg,
                        "level": level,
                    }),
                );
            }
            Ok(())
        })?,
    )?;

    reach.set("ui", ui_table)?;
    Ok(())
}

fn inject_ui_api(lua: &Lua, reach: &mlua::Table) -> LuaResult<()> {
    // If reach.ui already exists (from notify), use it; otherwise create new
    let ui_table: mlua::Table = match reach.get::<mlua::Table>("ui") {
        Ok(t) => t,
        Err(_) => lua.create_table()?,
    };

    ui_table.set(
        "render",
        lua.create_function(|lua, elements: mlua::Value| -> LuaResult<()> {
            // Convert Lua table to JSON, then parse as Vec<UiElement>
            let json_val = mlua::LuaSerdeExt::from_value::<serde_json::Value>(lua, elements)?;
            let _parsed: Vec<UiElement> = serde_json::from_value(json_val)
                .map_err(|e| mlua::Error::external(format!("Invalid UI elements: {}", e)))?;
            // The actual rendering is handled by the manager which reads this after call
            Ok(())
        })?,
    )?;

    reach.set("ui", ui_table)?;
    Ok(())
}
