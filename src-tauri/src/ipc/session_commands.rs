use crate::state::{AppState, AuthMethod, Folder, SessionConfig};

/// Persist the current in-memory sessions and folders to disk.
async fn persist_sessions(
    app: &tauri::AppHandle,
    state: &tauri::State<'_, AppState>,
) -> Result<(), String> {
    use tauri::Manager;
    let app_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;
    std::fs::create_dir_all(&app_dir)
        .map_err(|e| format!("Failed to create app data dir: {}", e))?;

    let sessions = state.sessions.read().await;
    let folders = state.folders.read().await;

    let store = crate::session::storage::SessionStore {
        sessions: sessions.values().cloned().collect(),
        folders: folders.values().cloned().collect(),
    };

    crate::session::storage::save_sessions(&app_dir, &store)
        .await
        .map_err(|e| e.to_string())
}

/// List all saved sessions.
#[tauri::command]
pub async fn session_list(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<SessionConfig>, String> {
    let sessions = state.sessions.read().await;
    Ok(sessions.values().cloned().collect())
}

/// Get a specific session by ID.
#[tauri::command]
pub async fn session_get(
    state: tauri::State<'_, AppState>,
    session_id: String,
) -> Result<SessionConfig, String> {
    let sessions = state.sessions.read().await;
    sessions
        .get(&session_id)
        .cloned()
        .ok_or_else(|| format!("Session not found: {}", session_id))
}

/// Create a new session configuration.
#[tauri::command]
pub async fn session_create(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    name: String,
    host: String,
    port: u16,
    username: String,
    auth_method: AuthMethod,
    folder_id: Option<String>,
    tags: Vec<String>,
) -> Result<SessionConfig, String> {
    let session = SessionConfig {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        host,
        port,
        username,
        auth_method,
        folder_id,
        tags,
        detected_os: None,
    };

    let mut sessions = state.sessions.write().await;
    sessions.insert(session.id.clone(), session.clone());
    drop(sessions);
    tracing::info!("Created session: {}", session.id);

    persist_sessions(&app, &state).await?;
    Ok(session)
}

/// Update an existing session configuration.
#[tauri::command]
pub async fn session_update(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    session: SessionConfig,
) -> Result<SessionConfig, String> {
    let mut sessions = state.sessions.write().await;
    if !sessions.contains_key(&session.id) {
        return Err(format!("Session not found: {}", session.id));
    }
    sessions.insert(session.id.clone(), session.clone());
    drop(sessions);
    tracing::info!("Updated session: {}", session.id);

    persist_sessions(&app, &state).await?;
    Ok(session)
}

/// Delete a session by ID.
#[tauri::command]
pub async fn session_delete(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    session_id: String,
) -> Result<(), String> {
    let mut sessions = state.sessions.write().await;
    sessions
        .remove(&session_id)
        .ok_or_else(|| format!("Session not found: {}", session_id))?;
    drop(sessions);
    tracing::info!("Deleted session: {}", session_id);

    persist_sessions(&app, &state).await?;
    Ok(())
}

/// List all session folders.
#[tauri::command]
pub async fn session_list_folders(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<Folder>, String> {
    let folders = state.folders.read().await;
    Ok(folders.values().cloned().collect())
}

/// Create a new session folder.
#[tauri::command]
pub async fn session_create_folder(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    name: String,
    parent_id: Option<String>,
) -> Result<Folder, String> {
    let folder = Folder {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        parent_id,
    };

    let mut folders = state.folders.write().await;
    folders.insert(folder.id.clone(), folder.clone());
    drop(folders);
    tracing::info!("Created folder: {}", folder.id);

    persist_sessions(&app, &state).await?;
    Ok(folder)
}

/// Delete a session folder by ID.
#[tauri::command]
pub async fn session_delete_folder(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    folder_id: String,
) -> Result<(), String> {
    let mut folders = state.folders.write().await;
    folders
        .remove(&folder_id)
        .ok_or_else(|| format!("Folder not found: {}", folder_id))?;
    drop(folders);
    tracing::info!("Deleted folder: {}", folder_id);

    persist_sessions(&app, &state).await?;
    Ok(())
}
