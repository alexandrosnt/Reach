use crate::playbook::engine;
use crate::playbook::parser::{extract_tasks, parse_playbook};
use crate::playbook::schema::{
    PlaybookCompleteEvent, PlaybookRun, PlaybookRunStatus, PlaybookValidation,
    SavedPlaybookProject,
};
use crate::playbook::storage;
use crate::state::AppState;
use tauri::{Emitter, State};

/// Start a playbook run over SSH.
/// Returns immediately with a Running status; execution happens in a background task.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn playbook_run(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
    run_id: Option<String>,
    playbook_content: String,
    connection_id: String,
    use_become: Option<bool>,
    extra_vars: Option<String>,
) -> Result<PlaybookRun, String> {
    let run_id = run_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    // Validate that the content parses before starting the background task
    let plays = parse_playbook(&playbook_content)?;
    let first_name = plays
        .first()
        .and_then(|p| p.name.clone());

    let run = PlaybookRun {
        id: run_id.clone(),
        name: first_name,
        connection_id: connection_id.clone(),
        status: PlaybookRunStatus::Running,
    };

    {
        let mut runs = state.playbook_runs.write().await;
        runs.insert(run.id.clone(), run.clone());
    }

    // Create cancellation token
    let cancel_token = tokio_util::sync::CancellationToken::new();
    {
        let mut tokens = state.playbook_cancel_tokens.lock().await;
        tokens.insert(run_id.clone(), cancel_token.clone());
    }

    let bg_runs = state.playbook_runs.clone();
    let bg_tokens = state.playbook_cancel_tokens.clone();
    let ssh_manager = state.ssh_manager.clone();
    let bg_app = app_handle.clone();
    let use_become = use_become.unwrap_or(false);

    tauri::async_runtime::spawn(async move {
        let complete_event = format!("playbook-complete-{}", run_id);

        // Get SSH handle
        let handle = {
            let mgr = ssh_manager.lock().await;
            match mgr.get_handle(&connection_id) {
                Ok(h) => h,
                Err(e) => {
                    tracing::error!("Playbook run {} SSH error: {}", run_id, e);
                    let mut runs = bg_runs.write().await;
                    if let Some(r) = runs.get_mut(&run_id) {
                        r.status = PlaybookRunStatus::Failed;
                    }
                    let _ = bg_app.emit(
                        &complete_event,
                        PlaybookCompleteEvent {
                            run_id: run_id.clone(),
                            status: PlaybookRunStatus::Failed,
                            exit_code: None,
                            tasks_ok: 0,
                            tasks_failed: 0,
                        },
                    );
                    return;
                }
            }
        };

        let result = engine::run_playbook(
            &playbook_content,
            &run_id,
            &handle,
            use_become,
            extra_vars.as_deref(),
            &cancel_token,
            &bg_app,
        )
        .await;

        // Clean up cancel token
        {
            let mut tokens = bg_tokens.lock().await;
            tokens.remove(&run_id);
        }

        // Update run status
        let mut runs = bg_runs.write().await;
        if let Some(r) = runs.get_mut(&run_id) {
            match &result {
                Ok((_, failed)) if *failed == 0 => {
                    r.status = PlaybookRunStatus::Completed;
                }
                Ok(_) => {
                    r.status = PlaybookRunStatus::Failed;
                }
                Err(ref e) => {
                    tracing::error!("Playbook run {} error: {}", run_id, e);
                    r.status = PlaybookRunStatus::Failed;
                    // Engine may not have emitted complete event on error
                    let _ = bg_app.emit(
                        &complete_event,
                        PlaybookCompleteEvent {
                            run_id: run_id.clone(),
                            status: PlaybookRunStatus::Failed,
                            exit_code: None,
                            tasks_ok: 0,
                            tasks_failed: 0,
                        },
                    );
                }
            }
        }
    });

    Ok(run)
}

/// Cancel a running playbook via CancellationToken.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn playbook_cancel(
    state: State<'_, AppState>,
    run_id: String,
) -> Result<(), String> {
    {
        let tokens = state.playbook_cancel_tokens.lock().await;
        if let Some(token) = tokens.get(&run_id) {
            token.cancel();
        }
    }

    let mut runs = state.playbook_runs.write().await;
    if let Some(r) = runs.get_mut(&run_id) {
        r.status = PlaybookRunStatus::Cancelled;
    }

    Ok(())
}

/// Get a specific playbook run by ID.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn playbook_get_run(
    state: State<'_, AppState>,
    run_id: String,
) -> Result<PlaybookRun, String> {
    let runs = state.playbook_runs.read().await;
    runs.get(&run_id)
        .cloned()
        .ok_or_else(|| format!("Playbook run not found: {}", run_id))
}

/// Validate a playbook (parse only, no execution).
/// Returns the list of task names and whether the playbook is valid.
#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn playbook_validate(
    playbook_content: String,
) -> Result<PlaybookValidation, String> {
    match parse_playbook(&playbook_content) {
        Ok(plays) => {
            let mut task_names = Vec::new();
            for play in &plays {
                match extract_tasks(&play.tasks) {
                    Ok(tasks) => {
                        for task in &tasks {
                            let name = task
                                .name
                                .as_deref()
                                .unwrap_or(&task.module);
                            task_names.push(name.to_string());
                        }
                    }
                    Err(e) => {
                        return Ok(PlaybookValidation {
                            valid: false,
                            tasks: task_names,
                            error: Some(e),
                        });
                    }
                }
            }
            Ok(PlaybookValidation {
                valid: true,
                tasks: task_names,
                error: None,
            })
        }
        Err(e) => Ok(PlaybookValidation {
            valid: false,
            tasks: Vec::new(),
            error: Some(e),
        }),
    }
}

/// Save a playbook project configuration to the vault.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn playbook_save_project(
    state: State<'_, AppState>,
    id: Option<String>,
    name: String,
    playbook_content: String,
    connection_id: Option<String>,
    use_become: Option<bool>,
) -> Result<SavedPlaybookProject, String> {
    let mut manager = state.vault_manager.lock().await;

    if manager.is_locked() {
        return Err("Vault is locked. Set a master password first.".to_string());
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    if let Some(ref existing_id) = id {
        // Update existing project
        let vault_id = storage::ensure_playbook_vault(&mut manager).await?;

        if !manager.secret_exists(&vault_id, existing_id).await {
            return Err(format!("Project not found: {}", existing_id));
        }

        let existing_plaintext = manager
            .read_secret(&vault_id, existing_id)
            .await
            .map_err(|e| e.to_string())?;

        use secrecy::ExposeSecret;
        let existing_json = String::from_utf8(existing_plaintext.expose_secret().clone())
            .map_err(|e| e.to_string())?;
        let existing: SavedPlaybookProject =
            serde_json::from_str(&existing_json).map_err(|e| e.to_string())?;

        let updated = SavedPlaybookProject {
            id: existing_id.clone(),
            name,
            playbook_content,
            connection_id,
            use_become: use_become.unwrap_or(false),
            created_at: existing.created_at,
            updated_at: now,
        };

        storage::save_project(&mut manager, &updated, Some(existing_id)).await?;
        tracing::info!("Updated playbook project: {}", updated.id);
        Ok(updated)
    } else {
        // Create new project
        let new_id = uuid::Uuid::new_v4().to_string();
        let project = SavedPlaybookProject {
            id: new_id,
            name: name.clone(),
            playbook_content,
            connection_id,
            use_become: use_become.unwrap_or(false),
            created_at: now,
            updated_at: now,
        };

        storage::save_project(&mut manager, &project, None).await?;
        tracing::info!("Saved playbook project: {}", project.id);
        Ok(project)
    }
}

/// List saved playbook project configurations.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn playbook_list_projects(
    state: State<'_, AppState>,
) -> Result<Vec<SavedPlaybookProject>, String> {
    let manager = state.vault_manager.lock().await;

    if manager.is_locked() {
        return Ok(Vec::new());
    }

    storage::list_projects(&manager).await
}

/// Delete a saved playbook project configuration.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn playbook_delete_project(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let manager = state.vault_manager.lock().await;

    if manager.is_locked() {
        return Err("Vault is locked".to_string());
    }

    storage::delete_project(&manager, &id).await?;
    tracing::info!("Deleted playbook project: {}", id);
    Ok(())
}
