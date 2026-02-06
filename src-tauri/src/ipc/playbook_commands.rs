//! Playbook commands using encrypted vault storage.
//!
//! Saved playbooks are stored encrypted in SQLite using XChaCha20-Poly1305.
//! Playbook runs are ephemeral (in-memory only during execution).

use crate::playbook::{engine, parser};
use crate::state::{AppState, PlaybookRun, PlaybookStatus, SavedPlaybook};
use crate::vault::types::SecretCategory;
use secrecy::SecretBox;
use tauri::State;

const PLAYBOOKS_VAULT_NAME: &str = "__playbooks__";

/// Parse and execute a playbook from YAML content against a specific SSH connection.
/// Returns immediately with a Running status; execution happens in a background task.
#[tauri::command]
#[tracing::instrument(skip(state, yaml_content))]
pub async fn playbook_run(
    state: State<'_, AppState>,
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
        status: PlaybookStatus::Running,
        current_step: 0,
        total_steps: playbook.steps.len(),
    };

    // Store in state immediately (ephemeral, in-memory)
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
                    r.status = PlaybookStatus::Failed;
                }
            }
        }
    });

    Ok(run)
}

/// Get a specific playbook run by ID. O(1) lookup.
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

/// Stop a running playbook by run ID.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn playbook_stop(
    state: State<'_, AppState>,
    run_id: String,
) -> Result<(), String> {
    engine::stop(&run_id)
        .await
        .map_err(|e| e.to_string())?;

    let mut runs = state.playbook_runs.write().await;
    if let Some(run) = runs.get_mut(&run_id) {
        run.status = PlaybookStatus::Stopped;
    }

    Ok(())
}

/// List all playbook runs (active and completed). O(n).
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn playbook_list(state: State<'_, AppState>) -> Result<Vec<PlaybookRun>, String> {
    let runs = state.playbook_runs.read().await;
    Ok(runs.values().cloned().collect())
}

/// Save a playbook definition to encrypted vault. O(1) upsert.
#[tauri::command]
#[tracing::instrument(skip(state, yaml_content))]
pub async fn playbook_save(
    state: State<'_, AppState>,
    id: Option<String>,
    yaml_content: String,
) -> Result<SavedPlaybook, String> {
    // Validate YAML before saving
    let playbook = parser::parse_yaml(&yaml_content).map_err(|e| e.to_string())?;

    let mut manager = state.vault_manager.lock().await;

    if manager.is_locked() {
        return Err("Vault is locked. Set a master password first.".to_string());
    }

    let vault_id = ensure_playbooks_vault(&mut manager).await?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let saved = if let Some(existing_id) = id {
        // Check if exists (O(1))
        if !manager.secret_exists(&vault_id, &existing_id).await {
            return Err(format!("Playbook not found: {}", existing_id));
        }

        // Read existing to get created_at
        let existing_plaintext = manager
            .read_secret(&vault_id, &existing_id)
            .await
            .map_err(|e| e.to_string())?;

        use secrecy::ExposeSecret;
        let existing_json = String::from_utf8(existing_plaintext.expose_secret().clone())
            .map_err(|e| e.to_string())?;
        let existing: SavedPlaybook =
            serde_json::from_str(&existing_json).map_err(|e| e.to_string())?;

        let updated = SavedPlaybook {
            id: existing_id.clone(),
            name: playbook.name.clone(),
            yaml_content,
            created_at: existing.created_at,
            updated_at: now,
        };

        let json = serde_json::to_string(&updated).map_err(|e| e.to_string())?;
        let plaintext = SecretBox::new(Box::new(json.into_bytes()));

        // O(1) update
        manager
            .update_secret(&vault_id, &existing_id, plaintext)
            .await
            .map_err(|e| e.to_string())?;

        updated
    } else {
        let new_id = uuid::Uuid::new_v4().to_string();
        let saved = SavedPlaybook {
            id: new_id.clone(),
            name: playbook.name.clone(),
            yaml_content,
            created_at: now,
            updated_at: now,
        };

        let json = serde_json::to_string(&saved).map_err(|e| e.to_string())?;
        let plaintext = SecretBox::new(Box::new(json.into_bytes()));

        // O(1) insert
        manager
            .create_secret_with_id(
                &vault_id,
                &new_id,
                &saved.name,
                SecretCategory::Custom("playbook".to_string()),
                plaintext,
            )
            .await
            .map_err(|e| e.to_string())?;

        saved
    };

    tracing::info!("Saved playbook: {}", saved.id);
    Ok(saved)
}

/// List all saved playbook definitions. O(n).
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn playbook_list_saved(state: State<'_, AppState>) -> Result<Vec<SavedPlaybook>, String> {
    let manager = state.vault_manager.lock().await;

    if manager.is_locked() {
        return Ok(Vec::new());
    }

    let vault_id = match get_playbooks_vault_id_if_exists(&manager) {
        Some(id) => id,
        None => return Ok(Vec::new()),
    };

    let secrets = manager
        .list_secrets(&vault_id)
        .await
        .map_err(|e| e.to_string())?;

    let mut playbooks = Vec::with_capacity(secrets.len());
    for secret in secrets {
        if let Ok(plaintext) = manager.read_secret(&vault_id, &secret.id).await {
            use secrecy::ExposeSecret;
            if let Ok(json) = String::from_utf8(plaintext.expose_secret().clone()) {
                if let Ok(pb) = serde_json::from_str::<SavedPlaybook>(&json) {
                    playbooks.push(pb);
                }
            }
        }
    }

    Ok(playbooks)
}

/// Delete a saved playbook definition. O(1) delete.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn playbook_delete(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let manager = state.vault_manager.lock().await;

    if manager.is_locked() {
        return Err("Vault is locked".to_string());
    }

    let vault_id = match get_playbooks_vault_id_if_exists(&manager) {
        Some(id) => id,
        None => return Ok(()),
    };

    // O(1) delete by primary key
    manager
        .delete_secret(&vault_id, &id)
        .await
        .map_err(|e| e.to_string())?;

    tracing::info!("Deleted playbook: {}", id);
    Ok(())
}

// --- Helper functions (all O(1)) ---

/// Ensure the playbooks vault exists. O(1).
async fn ensure_playbooks_vault(
    manager: &mut crate::vault::VaultManager,
) -> Result<String, String> {
    if let Some(vault_id) = manager.get_vault_id_by_name(PLAYBOOKS_VAULT_NAME) {
        let _ = manager.open_vault(&vault_id, None, None).await;
        manager
            .unlock_vault(&vault_id)
            .await
            .map_err(|e| e.to_string())?;
        Ok(vault_id)
    } else {
        let vault = manager
            .create_vault(PLAYBOOKS_VAULT_NAME, crate::vault::types::VaultType::Private, None, None)
            .await
            .map_err(|e| e.to_string())?;
        Ok(vault.id)
    }
}

/// Get playbooks vault ID if exists. O(1).
fn get_playbooks_vault_id_if_exists(manager: &crate::vault::VaultManager) -> Option<String> {
    manager.get_vault_id_by_name(PLAYBOOKS_VAULT_NAME)
}
