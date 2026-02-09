use crate::plugin::schema::*;
use crate::plugin::storage;
use crate::state::AppState;

/// Scan the plugins directory for plugin.toml manifests.
#[tauri::command]
pub async fn plugin_discover(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<PluginManifest>, String> {
    let manager = state.plugin_manager.lock().await;
    manager.discover_plugins()
}

/// Load a plugin by ID.
#[tauri::command]
pub async fn plugin_load(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    plugin_id: String,
) -> Result<PluginInfo, String> {
    // Load or create config
    let config = {
        let vault_mgr = state.vault_manager.lock().await;
        storage::load_plugin_config(&vault_mgr, &plugin_id)
            .await?
            .unwrap_or(PluginConfig {
                id: plugin_id.clone(),
                enabled: true,
                granted_permissions: Vec::new(),
            })
    };

    let ssh = state.ssh_manager.clone();
    let tunnel = state.tunnel_manager.clone();
    let vault = state.vault_manager.clone();

    let mut manager = state.plugin_manager.lock().await;
    manager.load_plugin(
        &plugin_id,
        config,
        ssh,
        tunnel,
        vault,
        Some(app),
    )
}

/// Unload a plugin.
#[tauri::command]
pub async fn plugin_unload(
    state: tauri::State<'_, AppState>,
    plugin_id: String,
) -> Result<(), String> {
    let mut manager = state.plugin_manager.lock().await;
    manager.unload_plugin(&plugin_id)
}

/// Reload a plugin.
#[tauri::command]
pub async fn plugin_reload(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    plugin_id: String,
) -> Result<PluginInfo, String> {
    let config = {
        let vault_mgr = state.vault_manager.lock().await;
        storage::load_plugin_config(&vault_mgr, &plugin_id)
            .await?
            .unwrap_or(PluginConfig {
                id: plugin_id.clone(),
                enabled: true,
                granted_permissions: Vec::new(),
            })
    };

    let ssh = state.ssh_manager.clone();
    let tunnel = state.tunnel_manager.clone();
    let vault = state.vault_manager.clone();

    let mut manager = state.plugin_manager.lock().await;
    manager.reload_plugin(
        &plugin_id,
        config,
        ssh,
        tunnel,
        vault,
        Some(app),
    )
}

/// List all loaded plugins.
#[tauri::command]
pub async fn plugin_list(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<PluginInfo>, String> {
    let manager = state.plugin_manager.lock().await;
    Ok(manager.list_plugins())
}

/// Call an action on a plugin.
#[tauri::command]
pub async fn plugin_call_action(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    plugin_id: String,
    action: String,
    params: serde_json::Value,
) -> Result<Option<PluginUiState>, String> {
    let mut manager = state.plugin_manager.lock().await;
    let result = manager.call_action(&plugin_id, &action, params).await?;

    // Emit UI update event if there's a new UI state
    if let Some(ref ui_state) = result {
        use tauri::Emitter;
        let _ = app.emit("plugin-ui-update", ui_state);
    }

    Ok(result)
}

/// Get current UI state for a plugin.
#[tauri::command]
pub async fn plugin_get_ui(
    state: tauri::State<'_, AppState>,
    plugin_id: String,
) -> Result<Option<PluginUiState>, String> {
    let manager = state.plugin_manager.lock().await;
    Ok(manager.get_ui_state(&plugin_id))
}

/// Get the persisted config for a plugin.
#[tauri::command]
pub async fn plugin_get_config(
    state: tauri::State<'_, AppState>,
    plugin_id: String,
) -> Result<Option<PluginConfig>, String> {
    let vault_mgr = state.vault_manager.lock().await;
    storage::load_plugin_config(&vault_mgr, &plugin_id).await
}

/// Save plugin configuration (enabled state, granted permissions).
#[tauri::command]
pub async fn plugin_set_config(
    state: tauri::State<'_, AppState>,
    config: PluginConfig,
) -> Result<(), String> {
    let mut vault_mgr = state.vault_manager.lock().await;
    storage::save_plugin_config(&mut vault_mgr, &config).await
}

/// Get the plugins directory path.
#[tauri::command]
pub async fn plugin_get_dir(
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let manager = state.plugin_manager.lock().await;
    Ok(manager.get_plugins_dir().to_string_lossy().to_string())
}

/// Set the plugins directory path.
#[tauri::command]
pub async fn plugin_set_dir(
    state: tauri::State<'_, AppState>,
    dir: String,
) -> Result<(), String> {
    let mut manager = state.plugin_manager.lock().await;
    manager.set_plugins_dir(std::path::PathBuf::from(dir));
    Ok(())
}

/// Manually dispatch a hook event (for testing).
#[tauri::command]
pub async fn plugin_dispatch_hook(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    event_name: String,
    data: serde_json::Value,
) -> Result<(), String> {
    let event = HookEvent { event_name, data };
    let mut manager = state.plugin_manager.lock().await;
    manager.dispatch_hook(&event, Some(&app)).await;
    Ok(())
}
