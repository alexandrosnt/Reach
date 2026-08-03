use crate::ipc::settings_commands::{
    delete_setting_by_key, read_setting_by_key, write_setting_by_key,
};
use crate::plugin::marketplace::{
    fetch_index, install_entry, uninstall_entry, MarketplaceEntry, DEFAULT_INDEX_URL,
};
use crate::state::AppState;

/// Settings key under which a user-overridden registry URL is persisted.
const MARKETPLACE_URL_KEY: &str = "marketplace_url";

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

/// Override the marketplace registry URL and persist it in the settings
/// vault so it survives restarts. Requires an unlocked vault; when the vault
/// is locked the runtime override still applies for this session only.
#[tauri::command]
pub async fn marketplace_set_url(
    state: tauri::State<'_, AppState>,
    url: String,
) -> Result<(), String> {
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err("Marketplace URL must be http(s)".into());
    }
    *state.marketplace_index_url.write().await = url.clone();

    let mut manager = state.vault_manager.lock().await;
    if !manager.is_locked() {
        write_setting_by_key(&mut manager, MARKETPLACE_URL_KEY, &url).await?;
        tracing::info!("Persisted marketplace registry URL");
    }
    Ok(())
}

/// Restore the default registry URL: reset the runtime value and remove the
/// persisted override. Vault lock only blocks the persisted delete; the
/// runtime value always resets.
#[tauri::command]
pub async fn marketplace_reset_url(state: tauri::State<'_, AppState>) -> Result<(), String> {
    *state.marketplace_index_url.write().await = DEFAULT_INDEX_URL.to_string();

    let mut manager = state.vault_manager.lock().await;
    if !manager.is_locked() {
        delete_setting_by_key(&mut manager, MARKETPLACE_URL_KEY).await?;
    }
    Ok(())
}

/// Load the persisted registry URL from the settings vault and apply it.
/// Call after vault unlock (e.g. when the marketplace panel opens). Returns
/// the URL now in effect. When the vault is locked or nothing was persisted,
/// the current runtime value (default or a previous session override) is
/// returned unchanged.
#[tauri::command]
pub async fn marketplace_load_url(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let persisted = {
        let manager = state.vault_manager.lock().await;
        if manager.is_locked() {
            None
        } else {
            read_setting_by_key(&manager, MARKETPLACE_URL_KEY).await
        }
    };

    if let Some(url) = persisted {
        if url.starts_with("https://") || url.starts_with("http://") {
            *state.marketplace_index_url.write().await = url.clone();
            return Ok(url);
        }
        tracing::warn!("Persisted marketplace URL rejected, keeping current value");
    }

    Ok(state.marketplace_index_url.read().await.clone())
}
