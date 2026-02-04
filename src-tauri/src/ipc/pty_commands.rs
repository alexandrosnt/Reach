use crate::state::AppState;

/// Spawn a new local PTY session.
///
/// Creates a pseudo-terminal running the specified shell (or the platform
/// default). The PTY output is streamed to the frontend via `pty-data-{id}`
/// events, and `pty-exit-{id}` is emitted when the process exits.
#[tauri::command]
pub async fn pty_spawn(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    id: String,
    shell: Option<String>,
    cols: u16,
    rows: u16,
) -> Result<String, String> {
    tracing::info!("IPC pty_spawn: id={}, shell={:?}, {}x{}", id, shell, cols, rows);

    let mut manager = state
        .pty_manager
        .lock()
        .map_err(|e| format!("Failed to lock PTY manager: {}", e))?;

    manager
        .spawn(&id, shell, cols, rows, app)
        .map_err(|e| e.to_string())
}

/// Write data (user keystrokes) to a PTY session.
#[tauri::command]
pub async fn pty_write(
    state: tauri::State<'_, AppState>,
    id: String,
    data: Vec<u8>,
) -> Result<(), String> {
    let mut manager = state
        .pty_manager
        .lock()
        .map_err(|e| format!("Failed to lock PTY manager: {}", e))?;

    manager
        .write(&id, &data)
        .map_err(|e| e.to_string())
}

/// Resize an existing PTY session.
#[tauri::command]
pub async fn pty_resize(
    state: tauri::State<'_, AppState>,
    id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    tracing::debug!("IPC pty_resize: id={}, {}x{}", id, cols, rows);

    let mut manager = state
        .pty_manager
        .lock()
        .map_err(|e| format!("Failed to lock PTY manager: {}", e))?;

    manager
        .resize(&id, cols, rows)
        .map_err(|e| e.to_string())
}

/// Close a PTY session, killing the child process.
#[tauri::command]
pub async fn pty_close(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    tracing::info!("IPC pty_close: id={}", id);

    let mut manager = state
        .pty_manager
        .lock()
        .map_err(|e| format!("Failed to lock PTY manager: {}", e))?;

    manager
        .close(&id)
        .map_err(|e| e.to_string())
}
