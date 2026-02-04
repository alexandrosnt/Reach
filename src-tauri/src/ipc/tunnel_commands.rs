use crate::state::{AppState, TunnelConfig, TunnelType};
use crate::tunnel::manager::TunnelManager;

/// Create a new tunnel configuration.
#[tauri::command]
pub async fn tunnel_create(
    state: tauri::State<'_, AppState>,
    tunnel_type: TunnelType,
    local_port: u16,
    remote_host: String,
    remote_port: u16,
    connection_id: String,
) -> Result<TunnelConfig, String> {
    let tunnel = TunnelManager::create_tunnel(
        tunnel_type,
        local_port,
        &remote_host,
        remote_port,
        &connection_id,
    );

    let mut tunnels = state.tunnels.write().await;
    tunnels.insert(tunnel.id.clone(), tunnel.clone());
    tracing::info!("Created tunnel: {}", tunnel.id);

    Ok(tunnel)
}

/// Start an existing tunnel, establishing the actual port forwarding.
#[tauri::command]
pub async fn tunnel_start(
    state: tauri::State<'_, AppState>,
    tunnel_id: String,
) -> Result<(), String> {
    // Get the SSH handle for this tunnel's connection
    let (handle, mut tunnel_config) = {
        let tunnels = state.tunnels.read().await;
        let tunnel = tunnels
            .get(&tunnel_id)
            .ok_or_else(|| format!("Tunnel not found: {}", tunnel_id))?;

        let ssh_manager = state.ssh_manager.lock().await;
        let handle = ssh_manager
            .get_handle(&tunnel.connection_id)
            .map_err(|e| e.to_string())?;

        (handle, tunnel.clone())
    };

    // Start the tunnel via TunnelManager
    {
        let mut tunnel_manager = state.tunnel_manager.lock().await;
        tunnel_manager
            .start_tunnel(&mut tunnel_config, &handle)
            .await
            .map_err(|e| e.to_string())?;
    }

    // Update tunnel config in state to mark it active
    let mut tunnels = state.tunnels.write().await;
    if let Some(t) = tunnels.get_mut(&tunnel_id) {
        t.active = true;
    }

    Ok(())
}

/// Stop an active tunnel.
#[tauri::command]
pub async fn tunnel_stop(
    state: tauri::State<'_, AppState>,
    tunnel_id: String,
) -> Result<(), String> {
    let mut tunnel_config = {
        let tunnels = state.tunnels.read().await;
        tunnels
            .get(&tunnel_id)
            .cloned()
            .ok_or_else(|| format!("Tunnel not found: {}", tunnel_id))?
    };

    {
        let mut tunnel_manager = state.tunnel_manager.lock().await;
        tunnel_manager
            .stop_tunnel(&mut tunnel_config)
            .await
            .map_err(|e| e.to_string())?;
    }

    // Update tunnel config in state to mark it inactive
    let mut tunnels = state.tunnels.write().await;
    if let Some(t) = tunnels.get_mut(&tunnel_id) {
        t.active = false;
    }

    Ok(())
}

/// List all configured tunnels.
#[tauri::command]
pub async fn tunnel_list(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<TunnelConfig>, String> {
    let tunnels = state.tunnels.read().await;
    Ok(tunnels.values().cloned().collect())
}
