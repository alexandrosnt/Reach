use crate::plugin::marketplace::{
    fetch_index, install_entry, uninstall_entry, MarketplaceEntry,
};
use crate::state::AppState;

/// Fetch the registry from the configured URL.
#[tauri::command]
pub async fn marketplace_fetch(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<MarketplaceEntry>, String> {
    let url = state.marketplace_index_url.read().await.clone();
    fetch_index(&url).await
}

/// Install a plugin from a marketplace entry: download, SHA-256 verify,
/// extract, and load it into the plugin manager.
#[tauri::command]
pub async fn marketplace_install(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    entry: MarketplaceEntry,
) -> Result<crate::plugin::schema::PluginInfo, String> {
    let plugin_id = entry.id.clone();

    // Resolve the plugins dir without holding the manager lock across the
    // network/IO-heavy install.
    let plugins_dir = {
        let mgr = state.plugin_manager.lock().await;
        mgr.get_plugins_dir()
    };

    install_entry(&plugins_dir, &entry).await?;

    // Load the freshly installed plugin disabled — the user must explicitly
    // grant permissions to enable it.
    let config = crate::plugin::schema::PluginConfig {
        id: plugin_id.clone(),
        enabled: false,
        granted_permissions: Vec::new(),
        version_at_grant: None,
    };

    let ssh = state.ssh_manager.clone();
    let tunnel = state.tunnel_manager.clone();
    let vault = state.vault_manager.clone();

    let mut manager = state.plugin_manager.lock().await;
    manager.load_plugin(&plugin_id, config, ssh, tunnel, vault, Some(app))
}

/// Uninstall a plugin: unload its VM, then remove its directory.
#[tauri::command]
pub async fn marketplace_uninstall(
    state: tauri::State<'_, AppState>,
    plugin_id: String,
) -> Result<(), String> {
    let plugins_dir = {
        let mut mgr = state.plugin_manager.lock().await;
        let _ = mgr.unload_plugin(&plugin_id);
        mgr.get_plugins_dir()
    };
    uninstall_entry(&plugins_dir, &plugin_id)
}

/// Get the current marketplace registry URL.
#[tauri::command]
pub async fn marketplace_get_url(state: tauri::State<'_, AppState>) -> Result<String, String> {
    Ok(state.marketplace_index_url.read().await.clone())
}

/// Override the marketplace registry URL (does not persist across restart yet
/// — settings persistence is a follow-up).
#[tauri::command]
pub async fn marketplace_set_url(
    state: tauri::State<'_, AppState>,
    url: String,
) -> Result<(), String> {
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err("Marketplace URL must be http(s)".into());
    }
    *state.marketplace_index_url.write().await = url;
    Ok(())
}
