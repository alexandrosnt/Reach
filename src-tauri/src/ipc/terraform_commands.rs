use crate::state::AppState;
use crate::terraform::engine;
use crate::terraform::types::{
    SavedTerraformWorkspace, TerraformExecMode, TerraformRun, TerraformRunStatus,
};
use crate::vault::types::SecretCategory;
use secrecy::SecretBox;
use tauri::State;

const TERRAFORM_VAULT_NAME: &str = "__terraform__";

/// Start a terraform operation (init/plan/apply/destroy).
/// Returns immediately with a Running status; execution happens in a background task.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn terraform_run(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
    action: String,
    working_dir: String,
    exec_mode: TerraformExecMode,
    connection_id: Option<String>,
) -> Result<TerraformRun, String> {
    let run_id = uuid::Uuid::new_v4().to_string();

    let run = TerraformRun {
        id: run_id.clone(),
        action: action.clone(),
        working_dir: working_dir.clone(),
        exec_mode: exec_mode.clone(),
        connection_id: connection_id.clone(),
        status: TerraformRunStatus::Running,
    };

    {
        let mut runs = state.terraform_runs.write().await;
        runs.insert(run.id.clone(), run.clone());
    }

    let bg_runs = state.terraform_runs.clone();
    let bg_processes = state.terraform_processes.clone();
    let ssh_manager = state.ssh_manager.clone();
    let bg_app = app_handle.clone();

    tauri::async_runtime::spawn(async move {
        let args = engine::build_args(&action);
        let args_refs: Vec<&str> = args.iter().map(|s| s.as_ref()).collect();

        let result = match exec_mode {
            TerraformExecMode::Local => {
                engine::run_local(&args_refs, &working_dir, &run_id, &bg_app).await
            }
            TerraformExecMode::Remote => {
                let cid = match connection_id {
                    Some(ref id) => id.clone(),
                    None => {
                        let mut runs = bg_runs.write().await;
                        if let Some(r) = runs.get_mut(&run_id) {
                            r.status = TerraformRunStatus::Failed;
                        }
                        return;
                    }
                };

                let handle = {
                    let mgr = ssh_manager.lock().await;
                    match mgr.get_handle(&cid) {
                        Ok(h) => h,
                        Err(e) => {
                            tracing::error!("Terraform run {} SSH error: {}", run_id, e);
                            let mut runs = bg_runs.write().await;
                            if let Some(r) = runs.get_mut(&run_id) {
                                r.status = TerraformRunStatus::Failed;
                            }
                            return;
                        }
                    }
                };

                engine::run_remote(&args_refs, &working_dir, &run_id, &handle, &bg_app).await
            }
        };

        // Clean up process handle
        {
            let mut procs = bg_processes.lock().await;
            procs.remove(&run_id);
        }

        // Update run status
        let mut runs = bg_runs.write().await;
        if let Some(r) = runs.get_mut(&run_id) {
            match result {
                Ok(code) if code == 0 => r.status = TerraformRunStatus::Completed,
                Ok(_) => r.status = TerraformRunStatus::Failed,
                Err(e) => {
                    tracing::error!("Terraform run {} error: {}", run_id, e);
                    r.status = TerraformRunStatus::Failed;
                }
            }
        }
    });

    Ok(run)
}

/// Cancel a running terraform operation.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn terraform_cancel(
    state: State<'_, AppState>,
    run_id: String,
) -> Result<(), String> {
    // Kill local process if exists
    {
        let mut procs = state.terraform_processes.lock().await;
        if let Some(mut child) = procs.remove(&run_id) {
            let _ = child.kill().await;
        }
    }

    // Update status
    let mut runs = state.terraform_runs.write().await;
    if let Some(r) = runs.get_mut(&run_id) {
        r.status = TerraformRunStatus::Cancelled;
    }

    Ok(())
}

/// Get a specific terraform run by ID.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn terraform_get_run(
    state: State<'_, AppState>,
    run_id: String,
) -> Result<TerraformRun, String> {
    let runs = state.terraform_runs.read().await;
    runs.get(&run_id)
        .cloned()
        .ok_or_else(|| format!("Terraform run not found: {}", run_id))
}

/// List terraform state resources (synchronous).
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn terraform_state_list(
    state: State<'_, AppState>,
    working_dir: String,
    exec_mode: TerraformExecMode,
    connection_id: Option<String>,
) -> Result<Vec<String>, String> {
    let args = ["state", "list"];

    let output = match exec_mode {
        TerraformExecMode::Local => {
            engine::run_local_sync(&args, &working_dir).await?
        }
        TerraformExecMode::Remote => {
            let cid = connection_id.ok_or("No connection ID for remote mode")?;
            let mgr = state.ssh_manager.lock().await;
            let handle = mgr.get_handle(&cid).map_err(|e| e.to_string())?;
            engine::run_remote_sync(&args, &working_dir, &handle).await?
        }
    };

    Ok(output
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect())
}

/// Show a specific resource in terraform state (synchronous).
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn terraform_state_show(
    state: State<'_, AppState>,
    working_dir: String,
    resource: String,
    exec_mode: TerraformExecMode,
    connection_id: Option<String>,
) -> Result<String, String> {
    let args = vec!["state", "show", &resource];

    match exec_mode {
        TerraformExecMode::Local => {
            engine::run_local_sync(&args, &working_dir).await
        }
        TerraformExecMode::Remote => {
            let cid = connection_id.ok_or("No connection ID for remote mode")?;
            let mgr = state.ssh_manager.lock().await;
            let handle = mgr.get_handle(&cid).map_err(|e| e.to_string())?;
            engine::run_remote_sync(&args, &working_dir, &handle).await
        }
    }
}

/// Get terraform outputs as JSON (synchronous).
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn terraform_output(
    state: State<'_, AppState>,
    working_dir: String,
    exec_mode: TerraformExecMode,
    connection_id: Option<String>,
) -> Result<serde_json::Value, String> {
    let args = ["output", "-json"];

    let output = match exec_mode {
        TerraformExecMode::Local => {
            engine::run_local_sync(&args, &working_dir).await?
        }
        TerraformExecMode::Remote => {
            let cid = connection_id.ok_or("No connection ID for remote mode")?;
            let mgr = state.ssh_manager.lock().await;
            let handle = mgr.get_handle(&cid).map_err(|e| e.to_string())?;
            engine::run_remote_sync(&args, &working_dir, &handle).await?
        }
    };

    serde_json::from_str(&output).map_err(|e| format!("Failed to parse terraform output: {}", e))
}

/// Check terraform version.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn terraform_check(
    state: State<'_, AppState>,
    exec_mode: TerraformExecMode,
    connection_id: Option<String>,
) -> Result<String, String> {
    let args = ["version"];

    match exec_mode {
        TerraformExecMode::Local => {
            engine::run_local_sync(&args, ".").await
        }
        TerraformExecMode::Remote => {
            let cid = connection_id.ok_or("No connection ID for remote mode")?;
            let mgr = state.ssh_manager.lock().await;
            let handle = mgr.get_handle(&cid).map_err(|e| e.to_string())?;
            engine::run_remote_sync(&args, ".", &handle).await
        }
    }
}

/// Save a workspace configuration to the vault.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn terraform_save_workspace(
    state: State<'_, AppState>,
    id: Option<String>,
    name: String,
    working_dir: String,
    exec_mode: TerraformExecMode,
    connection_id: Option<String>,
) -> Result<SavedTerraformWorkspace, String> {
    let mut manager = state.vault_manager.lock().await;

    if manager.is_locked() {
        return Err("Vault is locked. Set a master password first.".to_string());
    }

    let vault_id = ensure_terraform_vault(&mut manager).await?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let saved = if let Some(existing_id) = id {
        if !manager.secret_exists(&vault_id, &existing_id).await {
            return Err(format!("Workspace not found: {}", existing_id));
        }

        let existing_plaintext = manager
            .read_secret(&vault_id, &existing_id)
            .await
            .map_err(|e| e.to_string())?;

        use secrecy::ExposeSecret;
        let existing_json = String::from_utf8(existing_plaintext.expose_secret().clone())
            .map_err(|e| e.to_string())?;
        let existing: SavedTerraformWorkspace =
            serde_json::from_str(&existing_json).map_err(|e| e.to_string())?;

        let updated = SavedTerraformWorkspace {
            id: existing_id.clone(),
            name,
            working_dir,
            exec_mode,
            connection_id,
            created_at: existing.created_at,
            updated_at: now,
        };

        let json = serde_json::to_string(&updated).map_err(|e| e.to_string())?;
        let plaintext = SecretBox::new(Box::new(json.into_bytes()));

        manager
            .update_secret(&vault_id, &existing_id, plaintext)
            .await
            .map_err(|e| e.to_string())?;

        updated
    } else {
        let new_id = uuid::Uuid::new_v4().to_string();
        let saved = SavedTerraformWorkspace {
            id: new_id.clone(),
            name: name.clone(),
            working_dir,
            exec_mode,
            connection_id,
            created_at: now,
            updated_at: now,
        };

        let json = serde_json::to_string(&saved).map_err(|e| e.to_string())?;
        let plaintext = SecretBox::new(Box::new(json.into_bytes()));

        manager
            .create_secret_with_id(
                &vault_id,
                &new_id,
                &name,
                SecretCategory::Custom("terraform".to_string()),
                plaintext,
            )
            .await
            .map_err(|e| e.to_string())?;

        saved
    };

    tracing::info!("Saved terraform workspace: {}", saved.id);
    Ok(saved)
}

/// List saved workspace configurations.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn terraform_list_workspaces(
    state: State<'_, AppState>,
) -> Result<Vec<SavedTerraformWorkspace>, String> {
    let manager = state.vault_manager.lock().await;

    if manager.is_locked() {
        return Ok(Vec::new());
    }

    let vault_id = match get_terraform_vault_id_if_exists(&manager) {
        Some(id) => id,
        None => return Ok(Vec::new()),
    };

    let secrets = manager
        .list_secrets(&vault_id)
        .await
        .map_err(|e| e.to_string())?;

    let mut workspaces = Vec::with_capacity(secrets.len());
    for secret in secrets {
        if let Ok(plaintext) = manager.read_secret(&vault_id, &secret.id).await {
            use secrecy::ExposeSecret;
            if let Ok(json) = String::from_utf8(plaintext.expose_secret().clone()) {
                if let Ok(ws) = serde_json::from_str::<SavedTerraformWorkspace>(&json) {
                    workspaces.push(ws);
                }
            }
        }
    }

    Ok(workspaces)
}

/// Delete a saved workspace configuration.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn terraform_delete_workspace(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let manager = state.vault_manager.lock().await;

    if manager.is_locked() {
        return Err("Vault is locked".to_string());
    }

    let vault_id = match get_terraform_vault_id_if_exists(&manager) {
        Some(id) => id,
        None => return Ok(()),
    };

    manager
        .delete_secret(&vault_id, &id)
        .await
        .map_err(|e| e.to_string())?;

    tracing::info!("Deleted terraform workspace: {}", id);
    Ok(())
}

// --- Helper functions ---

async fn ensure_terraform_vault(
    manager: &mut crate::vault::VaultManager,
) -> Result<String, String> {
    if let Some(vault_id) = manager.get_vault_id_by_name(TERRAFORM_VAULT_NAME) {
        let _ = manager.open_vault(&vault_id, None, None).await;
        manager
            .unlock_vault(&vault_id)
            .await
            .map_err(|e| e.to_string())?;
        Ok(vault_id)
    } else {
        let vault = manager
            .create_vault(
                TERRAFORM_VAULT_NAME,
                crate::vault::types::VaultType::Private,
                None,
                None,
            )
            .await
            .map_err(|e| e.to_string())?;
        Ok(vault.id)
    }
}

fn get_terraform_vault_id_if_exists(manager: &crate::vault::VaultManager) -> Option<String> {
    manager.get_vault_id_by_name(TERRAFORM_VAULT_NAME)
}
