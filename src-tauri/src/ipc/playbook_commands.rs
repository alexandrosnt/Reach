use crate::playbook::{engine, parser};
use crate::state::{AppState, PlaybookRun, SavedPlaybook};

async fn persist_playbooks(
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

    let playbooks = state.saved_playbooks.read().await;
    let store = crate::playbook::storage::PlaybookStore {
        playbooks: playbooks.values().cloned().collect(),
    };

    crate::playbook::storage::save_playbooks(&app_dir, &store)
        .await
        .map_err(|e| e.to_string())
}

/// Parse and execute a playbook from YAML content against a specific SSH connection.
/// Returns immediately with a Running status; execution happens in a background task.
#[tauri::command]
pub async fn playbook_run(
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
    yaml_content: String,
    connection_id: String,
) -> Result<PlaybookRun, String> {
    let playbook = parser::parse_yaml(&yaml_content).map_err(|e| e.to_string())?;

    // Get the SSH handle for the target connection
    let handle = {
        let ssh_manager = state.ssh_manager.lock().await;
        ssh_manager
            .get_handle(&connection_id)
            .map_err(|e| e.to_string())?
    };

    let run_id = uuid::Uuid::new_v4().to_string();

    // Create the initial run with Running status
    let run = PlaybookRun {
        id: run_id.clone(),
        playbook_name: playbook.name.clone(),
        status: crate::state::PlaybookStatus::Running,
        current_step: 0,
        total_steps: playbook.steps.len(),
    };

    // Store in state immediately
    {
        let mut runs = state.playbook_runs.write().await;
        runs.insert(run.id.clone(), run.clone());
    }

    // Spawn execution in background so the frontend can set up listeners first
    let bg_runs = state.playbook_runs.clone();
    let bg_app = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        match engine::execute(&playbook, &handle, &run_id, &bg_app).await {
            Ok(finished_run) => {
                let mut runs = bg_runs.write().await;
                runs.insert(finished_run.id.clone(), finished_run);
            }
            Err(e) => {
                tracing::error!("Playbook run {} failed: {}", run_id, e);
                let mut runs = bg_runs.write().await;
                if let Some(r) = runs.get_mut(&run_id) {
                    r.status = crate::state::PlaybookStatus::Failed;
                }
            }
        }
    });

    Ok(run)
}

/// Get a specific playbook run by ID.
#[tauri::command]
pub async fn playbook_get_run(
    state: tauri::State<'_, AppState>,
    run_id: String,
) -> Result<PlaybookRun, String> {
    let runs = state.playbook_runs.read().await;
    runs.get(&run_id)
        .cloned()
        .ok_or_else(|| format!("Playbook run not found: {}", run_id))
}

/// Stop a running playbook by run ID.
#[tauri::command]
pub async fn playbook_stop(
    state: tauri::State<'_, AppState>,
    run_id: String,
) -> Result<(), String> {
    engine::stop(&run_id)
        .await
        .map_err(|e| e.to_string())?;

    let mut runs = state.playbook_runs.write().await;
    if let Some(run) = runs.get_mut(&run_id) {
        run.status = crate::state::PlaybookStatus::Stopped;
    }

    Ok(())
}

/// List all playbook runs (active and completed).
#[tauri::command]
pub async fn playbook_list(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<PlaybookRun>, String> {
    let runs = state.playbook_runs.read().await;
    Ok(runs.values().cloned().collect())
}

/// Save a playbook definition to disk.
#[tauri::command]
pub async fn playbook_save(
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
    id: Option<String>,
    yaml_content: String,
) -> Result<SavedPlaybook, String> {
    // Validate YAML before saving
    let playbook = parser::parse_yaml(&yaml_content).map_err(|e| e.to_string())?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let saved = if let Some(existing_id) = id {
        let mut playbooks = state.saved_playbooks.write().await;
        if let Some(existing) = playbooks.get_mut(&existing_id) {
            existing.name = playbook.name.clone();
            existing.yaml_content = yaml_content;
            existing.updated_at = now;
            existing.clone()
        } else {
            return Err(format!("Playbook not found: {}", existing_id));
        }
    } else {
        let new_id = uuid::Uuid::new_v4().to_string();
        let saved = SavedPlaybook {
            id: new_id.clone(),
            name: playbook.name.clone(),
            yaml_content,
            created_at: now,
            updated_at: now,
        };
        let mut playbooks = state.saved_playbooks.write().await;
        playbooks.insert(new_id, saved.clone());
        saved
    };

    persist_playbooks(&app_handle, &state).await?;
    Ok(saved)
}

/// List all saved playbook definitions.
#[tauri::command]
pub async fn playbook_list_saved(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<SavedPlaybook>, String> {
    let playbooks = state.saved_playbooks.read().await;
    Ok(playbooks.values().cloned().collect())
}

/// Delete a saved playbook definition.
#[tauri::command]
pub async fn playbook_delete(
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
    id: String,
) -> Result<(), String> {
    let mut playbooks = state.saved_playbooks.write().await;
    playbooks.remove(&id).ok_or_else(|| format!("Playbook not found: {}", id))?;
    drop(playbooks);
    persist_playbooks(&app_handle, &state).await?;
    Ok(())
}
