//! IPC for peer-to-peer sharing configuration.
//!
//! Same persistence shape as the marketplace URL: a runtime value that always
//! applies, mirrored into the settings vault whenever the vault is unlocked.
//! A locked vault blocks persistence, never the setting itself — you can turn
//! sharing on before unlocking; it just will not remember that next launch.
//!
//! Nothing here starts a connection. These commands read and write a config
//! struct. The first packet sharing sends is in response to the user clicking
//! Share, and that happens in the webview.

use crate::ipc::settings_commands::{delete_setting_by_key, read_setting_by_key, write_setting_by_key};
use crate::share::{self, IceServer, ShareConfig, ENABLED_KEY, ICE_KEY};
use crate::state::AppState;

/// The configuration currently in effect.
#[tauri::command]
pub async fn share_get_config(state: tauri::State<'_, AppState>) -> Result<ShareConfig, String> {
    Ok(state.share.read().await.clone())
}

/// Flip the master switch.
///
/// Off is the shipped default. Turning it off does not tear down an active
/// share — the webview owns the connection and is told separately — but it
/// does mean the next launch loads none of the sharing code.
#[tauri::command]
pub async fn share_set_enabled(
    state: tauri::State<'_, AppState>,
    enabled: bool,
) -> Result<ShareConfig, String> {
    state.share.write().await.enabled = enabled;

    let mut manager = state.vault_manager.lock().await;
    if !manager.is_locked() {
        if enabled {
            write_setting_by_key(&mut manager, ENABLED_KEY, "true").await?;
        } else {
            // Absent means off. Deleting rather than writing "false" keeps the
            // vault free of a key whose only value is the default.
            delete_setting_by_key(&mut manager, ENABLED_KEY).await?;
        }
        tracing::info!("Persisted sharing enabled={enabled}");
    }
    Ok(state.share.read().await.clone())
}

/// Replace the ICE server list.
///
/// Validated before anything is stored: a `https://` entry does not error in
/// the browser, it silently contributes no candidates, and a TURN server
/// without credentials refuses every allocation. Both would leave the user
/// with a relay that looks configured and does nothing.
#[tauri::command]
pub async fn share_set_ice_servers(
    state: tauri::State<'_, AppState>,
    servers: Vec<IceServer>,
) -> Result<ShareConfig, String> {
    let cleaned: Vec<IceServer> = servers
        .into_iter()
        .map(|s| IceServer {
            urls: s.urls.into_iter().map(|u| u.trim().to_string()).filter(|u| !u.is_empty()).collect(),
            username: s.username.map(|u| u.trim().to_string()).filter(|u| !u.is_empty()),
            credential: s.credential.map(|c| c.trim().to_string()).filter(|c| !c.is_empty()),
        })
        .collect();
    share::ice::validate_all(&cleaned).map_err(|e| e.to_string())?;

    state.share.write().await.ice_servers = cleaned.clone();

    let mut manager = state.vault_manager.lock().await;
    if !manager.is_locked() {
        let json = share::ice::to_json(&cleaned)?;
        write_setting_by_key(&mut manager, ICE_KEY, &json).await?;
        tracing::info!("Persisted {} ICE server(s)", cleaned.len());
    }
    Ok(state.share.read().await.clone())
}

/// Back to the shipped STUN defaults, forgetting any persisted list.
#[tauri::command]
pub async fn share_reset_ice_servers(
    state: tauri::State<'_, AppState>,
) -> Result<ShareConfig, String> {
    state.share.write().await.ice_servers = share::default_servers();

    let mut manager = state.vault_manager.lock().await;
    if !manager.is_locked() {
        delete_setting_by_key(&mut manager, ICE_KEY).await?;
    }
    Ok(state.share.read().await.clone())
}

/// Apply whatever the vault holds. Call once the vault is unlocked.
///
/// Reading before unlock finds nothing and would silently reset a user's
/// configuration to the defaults on every launch — the same trap the MCP
/// settings fell into before `mcp_restore` was moved behind the unlock.
#[tauri::command]
pub async fn share_load(state: tauri::State<'_, AppState>) -> Result<ShareConfig, String> {
    let (enabled, ice) = {
        let manager = state.vault_manager.lock().await;
        if manager.is_locked() {
            (None, None)
        } else {
            (
                read_setting_by_key(&manager, ENABLED_KEY).await,
                read_setting_by_key(&manager, ICE_KEY).await,
            )
        }
    };

    let mut cfg = state.share.write().await;
    if let Some(raw) = enabled {
        cfg.enabled = raw == "true";
    }
    if let Some(raw) = ice {
        cfg.ice_servers = share::ice::from_json(&raw);
    }
    Ok(cfg.clone())
}
