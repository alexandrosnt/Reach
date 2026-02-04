use crate::state::{AppState, SystemStats};

/// Start monitoring a remote host.
#[tauri::command]
pub async fn monitoring_start(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    connection_id: String,
) -> Result<(), String> {
    tracing::info!("Starting monitoring for connection {}", connection_id);

    let handle = {
        let manager = state.ssh_manager.lock().await;
        manager.get_handle(&connection_id).map_err(|e| e.to_string())?
    };

    let mut collector = state.monitoring_collector.lock().await;
    collector.start(&connection_id, handle, app);

    Ok(())
}

/// Stop monitoring a remote host.
#[tauri::command]
pub async fn monitoring_stop(
    state: tauri::State<'_, AppState>,
    connection_id: String,
) -> Result<(), String> {
    tracing::info!("Stopping monitoring for connection {}", connection_id);

    let mut collector = state.monitoring_collector.lock().await;
    collector.stop(&connection_id);

    let mut monitoring = state.monitoring.write().await;
    monitoring.remove(&connection_id);

    Ok(())
}

/// Get the latest stats for a monitored connection.
#[tauri::command]
pub async fn monitoring_get_stats(
    state: tauri::State<'_, AppState>,
    connection_id: String,
) -> Result<SystemStats, String> {
    let monitoring = state.monitoring.read().await;
    monitoring
        .get(&connection_id)
        .cloned()
        .ok_or_else(|| format!("No monitoring data for connection: {}", connection_id))
}
