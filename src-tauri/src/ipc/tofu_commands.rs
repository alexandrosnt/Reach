use std::process::Stdio;

use crate::ssh::client::exec_on_connection;
use crate::state::AppState;
use crate::tofu::runner;
use crate::tofu::types::{
    BackendCatalogEntry, DataSourceCatalogEntry, DependencyGraph, GeneratedFile,
    HclGenerationResult, ProjectTemplate, ProviderCatalogEntry, ProviderFieldSchema,
    ProviderFieldType, ProviderSchema, ResourceCatalogEntry, TofuBackendConfig,
    TofuCommandRequest, TofuDataSource, TofuEnvironment, TofuExecutionTarget, TofuFmtResult,
    TofuLocal, TofuModuleConfig, TofuOutput, TofuOutputValue, TofuPlanSummary, TofuProject,
    TofuProviderConfig, TofuResourceConfig, TofuVariable, TofuWorkspaceInfo,
};
use tauri::State;
use uuid::Uuid;

/// List all saved tofu projects.
#[tauri::command]
pub async fn tofu_list_projects(state: State<'_, AppState>) -> Result<Vec<TofuProject>, String> {
    let mut tofu_mgr = state.tofu_project_manager.lock().await;
    let mut vault_mgr = state.vault_manager.lock().await;
    tofu_mgr.ensure_loaded(&mut vault_mgr).await?;
    Ok(tofu_mgr.list_projects())
}

/// Create a new tofu project: scaffold directory + persist to vault.
#[tauri::command]
pub async fn tofu_create_project(
    state: State<'_, AppState>,
    name: String,
    path: String,
    description: String,
) -> Result<TofuProject, String> {
    // Scaffold directory
    crate::tofu::project::TofuProjectManager::scaffold_project(&path, &name)?;

    let now = {
        let dur = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap();
        let secs = dur.as_secs();
        let days = secs / 86400;
        let time_secs = secs % 86400;
        let hours = time_secs / 3600;
        let minutes = (time_secs % 3600) / 60;
        let seconds = time_secs % 60;
        let mut y = 1970i64;
        let mut remaining = days as i64;
        loop {
            let year_days = if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) { 366 } else { 365 };
            if remaining < year_days { break; }
            remaining -= year_days;
            y += 1;
        }
        let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
        let month_days = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
        let mut m = 0usize;
        for &md in &month_days {
            if remaining < md { break; }
            remaining -= md;
            m += 1;
        }
        format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, m + 1, remaining + 1, hours, minutes, seconds)
    };
    let project = TofuProject {
        id: Uuid::new_v4().to_string(),
        name,
        path,
        description,
        created_at: now.clone(),
        last_opened_at: now,
        providers: vec![],
        variables: vec![],
        environments: vec![],
        active_environment: None,
        resources: vec![],
        outputs: vec![],
        backend: None,
        data_sources: vec![],
        locals: vec![],
        modules: vec![],
    };

    let mut tofu_mgr = state.tofu_project_manager.lock().await;
    let mut vault_mgr = state.vault_manager.lock().await;
    tofu_mgr.ensure_loaded(&mut vault_mgr).await?;
    tofu_mgr.add_project(project.clone(), &mut vault_mgr).await?;

    Ok(project)
}

/// Delete a tofu project from vault (does not delete files on disk).
#[tauri::command]
pub async fn tofu_delete_project(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<(), String> {
    let mut tofu_mgr = state.tofu_project_manager.lock().await;
    let mut vault_mgr = state.vault_manager.lock().await;
    tofu_mgr.ensure_loaded(&mut vault_mgr).await?;
    tofu_mgr.remove_project(&project_id, &mut vault_mgr).await
}

/// Open/load a project and update its last_opened_at timestamp.
#[tauri::command]
pub async fn tofu_open_project(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<TofuProject, String> {
    let mut tofu_mgr = state.tofu_project_manager.lock().await;
    let mut vault_mgr = state.vault_manager.lock().await;
    tofu_mgr.ensure_loaded(&mut vault_mgr).await?;
    tofu_mgr.touch_project(&project_id, &mut vault_mgr).await?;
    tofu_mgr
        .get_project(&project_id)
        .cloned()
        .ok_or_else(|| "Project not found".to_string())
}

/// Execute a tofu CLI command, stream output via events. Returns the run_id.
#[tauri::command]
pub async fn tofu_run_command(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
    request: TofuCommandRequest,
) -> Result<String, String> {
    let run_id = Uuid::new_v4().to_string();

    // Get project path
    let project_path = {
        let mut tofu_mgr = state.tofu_project_manager.lock().await;
        let mut vault_mgr = state.vault_manager.lock().await;
        tofu_mgr.ensure_loaded(&mut vault_mgr).await?;
        tofu_mgr
            .get_project(&request.project_id)
            .map(|p| p.path.clone())
            .ok_or_else(|| "Project not found".to_string())?
    };

    let args = runner::build_command_args(&request);
    let rid = run_id.clone();

    match request.target {
        TofuExecutionTarget::Local => {
            let path = project_path.clone();
            tokio::spawn(async move {
                let _ = runner::run_local(&path, &args, &rid, &app_handle).await;
            });
        }
        TofuExecutionTarget::Ssh { connection_id } => {
            let ssh_mgr = state.ssh_manager.clone();
            let path = project_path.clone();
            tokio::spawn(async move {
                let mut manager = ssh_mgr.lock().await;
                let _ = runner::run_remote(
                    &connection_id,
                    &path,
                    &args,
                    &rid,
                    &app_handle,
                    &mut manager,
                )
                .await;
            });
        }
    }

    Ok(run_id)
}

/// Read a file from a tofu project directory.
#[tauri::command]
pub async fn tofu_read_file(
    state: State<'_, AppState>,
    project_id: String,
    filename: String,
) -> Result<String, String> {
    let tofu_mgr = state.tofu_project_manager.lock().await;
    let project = tofu_mgr
        .get_project(&project_id)
        .ok_or_else(|| "Project not found".to_string())?;

    let file_path = std::path::Path::new(&project.path).join(&filename);
    std::fs::read_to_string(&file_path)
        .map_err(|e| format!("Failed to read {}: {}", filename, e))
}

/// Write a file to a tofu project directory.
#[tauri::command]
pub async fn tofu_write_file(
    state: State<'_, AppState>,
    project_id: String,
    filename: String,
    content: String,
) -> Result<(), String> {
    let tofu_mgr = state.tofu_project_manager.lock().await;
    let project = tofu_mgr
        .get_project(&project_id)
        .ok_or_else(|| "Project not found".to_string())?;

    let file_path = std::path::Path::new(&project.path).join(&filename);
    std::fs::write(&file_path, content)
        .map_err(|e| format!("Failed to write {}: {}", filename, e))
}

/// List .tf files in a tofu project directory.
#[tauri::command]
pub async fn tofu_list_files(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Vec<String>, String> {
    let tofu_mgr = state.tofu_project_manager.lock().await;
    let project = tofu_mgr
        .get_project(&project_id)
        .ok_or_else(|| "Project not found".to_string())?;

    let dir = std::path::Path::new(&project.path);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    let entries = std::fs::read_dir(dir)
        .map_err(|e| format!("Failed to read directory: {}", e))?;

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.ends_with(".tf") || name.ends_with(".tfvars") || name.ends_with(".tofu") {
            files.push(name);
        }
    }

    files.sort();
    Ok(files)
}

/// Return the static provider catalog.
#[tauri::command]
pub async fn tofu_get_provider_catalog() -> Result<Vec<ProviderCatalogEntry>, String> {
    Ok(crate::tofu::catalog::get_provider_catalog())
}

/// Update providers on a project and persist to vault.
#[tauri::command]
pub async fn tofu_update_providers(
    state: State<'_, AppState>,
    project_id: String,
    providers: Vec<TofuProviderConfig>,
) -> Result<TofuProject, String> {
    let mut tofu_mgr = state.tofu_project_manager.lock().await;
    let mut vault_mgr = state.vault_manager.lock().await;
    tofu_mgr.ensure_loaded(&mut vault_mgr).await?;

    let mut project = tofu_mgr
        .get_project(&project_id)
        .cloned()
        .ok_or_else(|| "Project not found".to_string())?;

    project.providers = providers;
    tofu_mgr.update_project(project.clone(), &mut vault_mgr).await?;
    Ok(project)
}

/// Update variables on a project and persist to vault.
#[tauri::command]
pub async fn tofu_update_variables(
    state: State<'_, AppState>,
    project_id: String,
    variables: Vec<TofuVariable>,
) -> Result<TofuProject, String> {
    let mut tofu_mgr = state.tofu_project_manager.lock().await;
    let mut vault_mgr = state.vault_manager.lock().await;
    tofu_mgr.ensure_loaded(&mut vault_mgr).await?;

    let mut project = tofu_mgr
        .get_project(&project_id)
        .cloned()
        .ok_or_else(|| "Project not found".to_string())?;

    project.variables = variables;
    tofu_mgr.update_project(project.clone(), &mut vault_mgr).await?;
    Ok(project)
}

/// Update environments and active environment on a project and persist to vault.
#[tauri::command]
pub async fn tofu_update_environments(
    state: State<'_, AppState>,
    project_id: String,
    environments: Vec<TofuEnvironment>,
    active_environment: Option<String>,
) -> Result<TofuProject, String> {
    let mut tofu_mgr = state.tofu_project_manager.lock().await;
    let mut vault_mgr = state.vault_manager.lock().await;
    tofu_mgr.ensure_loaded(&mut vault_mgr).await?;

    let mut project = tofu_mgr
        .get_project(&project_id)
        .cloned()
        .ok_or_else(|| "Project not found".to_string())?;

    project.environments = environments;
    project.active_environment = active_environment;
    tofu_mgr.update_project(project.clone(), &mut vault_mgr).await?;
    Ok(project)
}

/// Preview: generate HCL files from visual configuration, return without writing.
#[tauri::command]
pub async fn tofu_generate_hcl(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<HclGenerationResult, String> {
    let mut tofu_mgr = state.tofu_project_manager.lock().await;
    let mut vault_mgr = state.vault_manager.lock().await;
    tofu_mgr.ensure_loaded(&mut vault_mgr).await?;

    let project = tofu_mgr
        .get_project(&project_id)
        .cloned()
        .ok_or_else(|| "Project not found".to_string())?;

    let catalog = crate::tofu::catalog::get_provider_catalog();
    let resource_catalog = crate::tofu::resource_catalog::get_resource_catalog();
    let data_source_catalog = crate::tofu::data_catalog::get_data_source_catalog();
    let files = crate::tofu::hcl::generate_all(&project, &catalog, &resource_catalog, &data_source_catalog);

    Ok(HclGenerationResult { files })
}

/// Write generated files to the project directory on disk.
#[tauri::command]
pub async fn tofu_write_generated_files(
    state: State<'_, AppState>,
    project_id: String,
    files: Vec<GeneratedFile>,
) -> Result<(), String> {
    let tofu_mgr = state.tofu_project_manager.lock().await;
    let project = tofu_mgr
        .get_project(&project_id)
        .ok_or_else(|| "Project not found".to_string())?;

    let dir = std::path::Path::new(&project.path);
    for file in &files {
        let file_path = dir.join(&file.filename);
        std::fs::write(&file_path, &file.content)
            .map_err(|e| format!("Failed to write {}: {}", file.filename, e))?;
    }

    Ok(())
}

/// Return the static resource catalog.
#[tauri::command]
pub async fn tofu_get_resource_catalog() -> Result<Vec<ResourceCatalogEntry>, String> {
    Ok(crate::tofu::resource_catalog::get_resource_catalog())
}

/// Update resources on a project and persist to vault.
#[tauri::command]
pub async fn tofu_update_resources(
    state: State<'_, AppState>,
    project_id: String,
    resources: Vec<TofuResourceConfig>,
) -> Result<TofuProject, String> {
    let mut tofu_mgr = state.tofu_project_manager.lock().await;
    let mut vault_mgr = state.vault_manager.lock().await;
    tofu_mgr.ensure_loaded(&mut vault_mgr).await?;

    let mut project = tofu_mgr
        .get_project(&project_id)
        .cloned()
        .ok_or_else(|| "Project not found".to_string())?;

    project.resources = resources;
    tofu_mgr.update_project(project.clone(), &mut vault_mgr).await?;
    Ok(project)
}

/// List resources tracked in the tofu state file (non-streaming).
#[tauri::command]
pub async fn tofu_state_list_resources(
    state: State<'_, AppState>,
    project_id: String,
    target: TofuExecutionTarget,
) -> Result<Vec<String>, String> {
    // Get project path
    let project_path = {
        let mut tofu_mgr = state.tofu_project_manager.lock().await;
        let mut vault_mgr = state.vault_manager.lock().await;
        tofu_mgr.ensure_loaded(&mut vault_mgr).await?;
        tofu_mgr
            .get_project(&project_id)
            .map(|p| p.path.clone())
            .ok_or_else(|| "Project not found".to_string())?
    };

    let output = match target {
        TofuExecutionTarget::Local => {
            // Find tofu binary
            let binary = if which::which("tofu").is_ok() {
                "tofu"
            } else if which::which("terraform").is_ok() {
                "terraform"
            } else {
                return Err("OpenTofu CLI not found".to_string());
            };

            let result = tokio::process::Command::new(binary)
                .args(["state", "list"])
                .current_dir(&project_path)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .stdin(Stdio::null())
                .output()
                .await
                .map_err(|e| format!("Failed to run tofu state list: {}", e))?;

            String::from_utf8_lossy(&result.stdout).to_string()
        }
        TofuExecutionTarget::Ssh { connection_id } => {
            let ssh_mgr = state.ssh_manager.clone();
            let manager = ssh_mgr.lock().await;
            let handle = manager
                .get_handle(&connection_id)
                .map_err(|e| e.to_string())?;

            let cmd = format!(
                "cd {} && tofu state list",
                runner::shell_escape(&project_path)
            );
            exec_on_connection(&handle, &cmd)
                .await
                .map_err(|e| e.to_string())?
        }
    };

    let resources: Vec<String> = output
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    Ok(resources)
}

/// Update outputs on a project and persist to vault.
#[tauri::command]
pub async fn tofu_update_outputs(
    state: State<'_, AppState>,
    project_id: String,
    outputs: Vec<TofuOutput>,
) -> Result<TofuProject, String> {
    let mut tofu_mgr = state.tofu_project_manager.lock().await;
    let mut vault_mgr = state.vault_manager.lock().await;
    tofu_mgr.ensure_loaded(&mut vault_mgr).await?;

    let mut project = tofu_mgr
        .get_project(&project_id)
        .cloned()
        .ok_or_else(|| "Project not found".to_string())?;

    project.outputs = outputs;
    tofu_mgr.update_project(project.clone(), &mut vault_mgr).await?;
    Ok(project)
}

/// Get live output values by running `tofu output -json`.
#[tauri::command]
pub async fn tofu_get_output_values(
    state: State<'_, AppState>,
    project_id: String,
    target: TofuExecutionTarget,
) -> Result<Vec<TofuOutputValue>, String> {
    let project_path = {
        let mut tofu_mgr = state.tofu_project_manager.lock().await;
        let mut vault_mgr = state.vault_manager.lock().await;
        tofu_mgr.ensure_loaded(&mut vault_mgr).await?;
        tofu_mgr
            .get_project(&project_id)
            .map(|p| p.path.clone())
            .ok_or_else(|| "Project not found".to_string())?
    };

    let output = match target {
        TofuExecutionTarget::Local => {
            let binary = if which::which("tofu").is_ok() {
                "tofu"
            } else if which::which("terraform").is_ok() {
                "terraform"
            } else {
                return Err("OpenTofu CLI not found".to_string());
            };

            let result = tokio::process::Command::new(binary)
                .args(["output", "-json"])
                .current_dir(&project_path)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .stdin(Stdio::null())
                .output()
                .await
                .map_err(|e| format!("Failed to run tofu output: {}", e))?;

            String::from_utf8_lossy(&result.stdout).to_string()
        }
        TofuExecutionTarget::Ssh { connection_id } => {
            let ssh_mgr = state.ssh_manager.clone();
            let manager = ssh_mgr.lock().await;
            let handle = manager
                .get_handle(&connection_id)
                .map_err(|e| e.to_string())?;

            let cmd = format!(
                "cd {} && tofu output -json",
                runner::shell_escape(&project_path)
            );
            exec_on_connection(&handle, &cmd)
                .await
                .map_err(|e| e.to_string())?
        }
    };

    // Parse JSON: { "name": { "value": ..., "type": "...", "sensitive": bool } }
    let parsed: serde_json::Value = serde_json::from_str(&output)
        .map_err(|e| format!("Failed to parse output JSON: {}", e))?;

    let mut values = Vec::new();
    if let serde_json::Value::Object(map) = parsed {
        for (name, entry) in map {
            let value = entry.get("value").cloned().unwrap_or(serde_json::Value::Null);
            let output_type = entry
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let sensitive = entry
                .get("sensitive")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            values.push(TofuOutputValue {
                name,
                value,
                output_type,
                sensitive,
            });
        }
    }

    Ok(values)
}

/// Build the dependency graph for a project's resources.
#[tauri::command]
pub async fn tofu_get_dependency_graph(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<DependencyGraph, String> {
    let mut tofu_mgr = state.tofu_project_manager.lock().await;
    let mut vault_mgr = state.vault_manager.lock().await;
    tofu_mgr.ensure_loaded(&mut vault_mgr).await?;

    let project = tofu_mgr
        .get_project(&project_id)
        .ok_or_else(|| "Project not found".to_string())?;

    Ok(crate::tofu::graph::build_dependency_graph(&project.resources))
}

/// Return the static project template catalog.
#[tauri::command]
pub async fn tofu_get_templates() -> Result<Vec<ProjectTemplate>, String> {
    Ok(crate::tofu::templates::get_project_templates())
}

/// Apply a template to a project, replacing its providers/variables/resources/outputs.
#[tauri::command]
pub async fn tofu_apply_template(
    state: State<'_, AppState>,
    project_id: String,
    template_id: String,
) -> Result<TofuProject, String> {
    let templates = crate::tofu::templates::get_project_templates();
    let template = templates
        .iter()
        .find(|t| t.id == template_id)
        .ok_or_else(|| format!("Template '{}' not found", template_id))?;

    let mut tofu_mgr = state.tofu_project_manager.lock().await;
    let mut vault_mgr = state.vault_manager.lock().await;
    tofu_mgr.ensure_loaded(&mut vault_mgr).await?;

    let mut project = tofu_mgr
        .get_project(&project_id)
        .cloned()
        .ok_or_else(|| "Project not found".to_string())?;

    // Apply template data with fresh UUIDs for resource IDs
    project.providers = template.providers.clone();
    project.variables = template.variables.clone();
    project.outputs = template.outputs.clone();

    // Regenerate resource IDs so they're unique
    let mut resources = template.resources.clone();
    for res in &mut resources {
        res.id = Uuid::new_v4().to_string();
    }
    project.resources = resources;

    tofu_mgr
        .update_project(project.clone(), &mut vault_mgr)
        .await?;
    Ok(project)
}

/// Update backend config on a project and persist to vault.
#[tauri::command]
pub async fn tofu_update_backend(
    state: State<'_, AppState>,
    project_id: String,
    backend: Option<TofuBackendConfig>,
) -> Result<TofuProject, String> {
    let mut tofu_mgr = state.tofu_project_manager.lock().await;
    let mut vault_mgr = state.vault_manager.lock().await;
    tofu_mgr.ensure_loaded(&mut vault_mgr).await?;

    let mut project = tofu_mgr
        .get_project(&project_id)
        .cloned()
        .ok_or_else(|| "Project not found".to_string())?;

    project.backend = backend;
    tofu_mgr.update_project(project.clone(), &mut vault_mgr).await?;
    Ok(project)
}

/// Update data sources on a project and persist to vault.
#[tauri::command]
pub async fn tofu_update_data_sources(
    state: State<'_, AppState>,
    project_id: String,
    data_sources: Vec<TofuDataSource>,
) -> Result<TofuProject, String> {
    let mut tofu_mgr = state.tofu_project_manager.lock().await;
    let mut vault_mgr = state.vault_manager.lock().await;
    tofu_mgr.ensure_loaded(&mut vault_mgr).await?;

    let mut project = tofu_mgr
        .get_project(&project_id)
        .cloned()
        .ok_or_else(|| "Project not found".to_string())?;

    project.data_sources = data_sources;
    tofu_mgr.update_project(project.clone(), &mut vault_mgr).await?;
    Ok(project)
}

/// Return the static data source catalog.
#[tauri::command]
pub async fn tofu_get_data_source_catalog() -> Result<Vec<DataSourceCatalogEntry>, String> {
    Ok(crate::tofu::data_catalog::get_data_source_catalog())
}

/// Update locals on a project and persist to vault.
#[tauri::command]
pub async fn tofu_update_locals(
    state: State<'_, AppState>,
    project_id: String,
    locals: Vec<TofuLocal>,
) -> Result<TofuProject, String> {
    let mut tofu_mgr = state.tofu_project_manager.lock().await;
    let mut vault_mgr = state.vault_manager.lock().await;
    tofu_mgr.ensure_loaded(&mut vault_mgr).await?;

    let mut project = tofu_mgr
        .get_project(&project_id)
        .cloned()
        .ok_or_else(|| "Project not found".to_string())?;

    project.locals = locals;
    tofu_mgr.update_project(project.clone(), &mut vault_mgr).await?;
    Ok(project)
}

/// Update modules on a project and persist to vault.
#[tauri::command]
pub async fn tofu_update_modules(
    state: State<'_, AppState>,
    project_id: String,
    modules: Vec<TofuModuleConfig>,
) -> Result<TofuProject, String> {
    let mut tofu_mgr = state.tofu_project_manager.lock().await;
    let mut vault_mgr = state.vault_manager.lock().await;
    tofu_mgr.ensure_loaded(&mut vault_mgr).await?;

    let mut project = tofu_mgr
        .get_project(&project_id)
        .cloned()
        .ok_or_else(|| "Project not found".to_string())?;

    project.modules = modules;
    tofu_mgr.update_project(project.clone(), &mut vault_mgr).await?;
    Ok(project)
}

/// Return the static backend catalog.
#[tauri::command]
pub async fn tofu_get_backend_catalog() -> Result<Vec<BackendCatalogEntry>, String> {
    Ok(crate::tofu::backend_catalog::get_backend_catalog())
}

/// List workspaces and current workspace.
#[tauri::command]
pub async fn tofu_workspace_list(
    state: State<'_, AppState>,
    project_id: String,
    target: TofuExecutionTarget,
) -> Result<TofuWorkspaceInfo, String> {
    let project_path = {
        let mut tofu_mgr = state.tofu_project_manager.lock().await;
        let mut vault_mgr = state.vault_manager.lock().await;
        tofu_mgr.ensure_loaded(&mut vault_mgr).await?;
        tofu_mgr
            .get_project(&project_id)
            .map(|p| p.path.clone())
            .ok_or_else(|| "Project not found".to_string())?
    };

    let output = run_tofu_sync(&state, &target, &project_path, &["workspace", "list"]).await?;

    let mut workspaces = Vec::new();
    let mut current = String::from("default");

    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(name) = trimmed.strip_prefix("* ") {
            current = name.trim().to_string();
            workspaces.push(name.trim().to_string());
        } else {
            workspaces.push(trimmed.to_string());
        }
    }

    if workspaces.is_empty() {
        workspaces.push("default".to_string());
    }

    Ok(TofuWorkspaceInfo {
        workspaces,
        current,
    })
}

/// Create a new workspace.
#[tauri::command]
pub async fn tofu_workspace_new(
    state: State<'_, AppState>,
    project_id: String,
    target: TofuExecutionTarget,
    name: String,
) -> Result<(), String> {
    let project_path = {
        let mut tofu_mgr = state.tofu_project_manager.lock().await;
        let mut vault_mgr = state.vault_manager.lock().await;
        tofu_mgr.ensure_loaded(&mut vault_mgr).await?;
        tofu_mgr
            .get_project(&project_id)
            .map(|p| p.path.clone())
            .ok_or_else(|| "Project not found".to_string())?
    };

    run_tofu_sync(&state, &target, &project_path, &["workspace", "new", &name]).await?;
    Ok(())
}

/// Select (switch to) a workspace.
#[tauri::command]
pub async fn tofu_workspace_select(
    state: State<'_, AppState>,
    project_id: String,
    target: TofuExecutionTarget,
    name: String,
) -> Result<(), String> {
    let project_path = {
        let mut tofu_mgr = state.tofu_project_manager.lock().await;
        let mut vault_mgr = state.vault_manager.lock().await;
        tofu_mgr.ensure_loaded(&mut vault_mgr).await?;
        tofu_mgr
            .get_project(&project_id)
            .map(|p| p.path.clone())
            .ok_or_else(|| "Project not found".to_string())?
    };

    run_tofu_sync(&state, &target, &project_path, &["workspace", "select", &name]).await?;
    Ok(())
}

/// Delete a workspace.
#[tauri::command]
pub async fn tofu_workspace_delete(
    state: State<'_, AppState>,
    project_id: String,
    target: TofuExecutionTarget,
    name: String,
) -> Result<(), String> {
    let project_path = {
        let mut tofu_mgr = state.tofu_project_manager.lock().await;
        let mut vault_mgr = state.vault_manager.lock().await;
        tofu_mgr.ensure_loaded(&mut vault_mgr).await?;
        tofu_mgr
            .get_project(&project_id)
            .map(|p| p.path.clone())
            .ok_or_else(|| "Project not found".to_string())?
    };

    run_tofu_sync(
        &state,
        &target,
        &project_path,
        &["workspace", "delete", "-force", &name],
    )
    .await?;
    Ok(())
}

/// Format HCL files.
#[tauri::command]
pub async fn tofu_fmt(
    state: State<'_, AppState>,
    project_id: String,
    target: TofuExecutionTarget,
    check_only: bool,
) -> Result<TofuFmtResult, String> {
    let project_path = {
        let mut tofu_mgr = state.tofu_project_manager.lock().await;
        let mut vault_mgr = state.vault_manager.lock().await;
        tofu_mgr.ensure_loaded(&mut vault_mgr).await?;
        tofu_mgr
            .get_project(&project_id)
            .map(|p| p.path.clone())
            .ok_or_else(|| "Project not found".to_string())?
    };

    let args = if check_only {
        vec!["fmt", "-check", "-list=true"]
    } else {
        vec!["fmt", "-list=true"]
    };

    let output = run_tofu_sync_full(&state, &target, &project_path, &args).await;

    match output {
        Ok(stdout) => {
            let changed_files: Vec<String> = stdout
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect();
            Ok(TofuFmtResult {
                success: true,
                changed_files,
                error: None,
            })
        }
        Err(e) => Ok(TofuFmtResult {
            success: false,
            changed_files: vec![],
            error: Some(e),
        }),
    }
}

/// Show plan JSON and parse into structured summary.
#[tauri::command]
pub async fn tofu_show_plan_json(
    state: State<'_, AppState>,
    project_id: String,
    target: TofuExecutionTarget,
) -> Result<TofuPlanSummary, String> {
    let project_path = {
        let mut tofu_mgr = state.tofu_project_manager.lock().await;
        let mut vault_mgr = state.vault_manager.lock().await;
        tofu_mgr.ensure_loaded(&mut vault_mgr).await?;
        tofu_mgr
            .get_project(&project_id)
            .map(|p| p.path.clone())
            .ok_or_else(|| "Project not found".to_string())?
    };

    let output =
        run_tofu_sync(&state, &target, &project_path, &["show", "-json", ".reach-plan"]).await?;

    crate::tofu::plan_parser::parse_plan_json(&output)
}

/// Fetch provider schemas by running `tofu providers schema -json`, cache results.
#[tauri::command]
pub async fn tofu_fetch_schema(
    state: State<'_, AppState>,
    project_id: String,
    target: TofuExecutionTarget,
) -> Result<Vec<ProviderSchema>, String> {
    let project_path = {
        let mut tofu_mgr = state.tofu_project_manager.lock().await;
        let mut vault_mgr = state.vault_manager.lock().await;
        tofu_mgr.ensure_loaded(&mut vault_mgr).await?;
        tofu_mgr
            .get_project(&project_id)
            .map(|p| p.path.clone())
            .ok_or_else(|| "Project not found".to_string())?
    };

    let output =
        run_tofu_sync(&state, &target, &project_path, &["providers", "schema", "-json"]).await?;

    let schemas = crate::tofu::schema::parse_schema_json(&output)?;

    // Cache the result
    let mut cache = state.tofu_schema_cache.lock().await;
    cache.cache.insert(project_id, schemas.clone());

    Ok(schemas)
}

/// Return cached provider schemas for a project (no CLI call).
#[tauri::command]
pub async fn tofu_get_cached_schema(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Option<Vec<ProviderSchema>>, String> {
    let cache = state.tofu_schema_cache.lock().await;
    Ok(cache.cache.get(&project_id).cloned())
}

/// Get resource fields from dynamic schema, falling back to static catalog.
#[tauri::command]
pub async fn tofu_get_schema_resource_fields(
    state: State<'_, AppState>,
    project_id: String,
    resource_type: String,
) -> Result<Vec<ProviderFieldSchema>, String> {
    // Try dynamic schema first
    let cache = state.tofu_schema_cache.lock().await;
    if let Some(schemas) = cache.cache.get(&project_id) {
        for provider in schemas {
            for rs in &provider.resource_schemas {
                if rs.resource_type == resource_type {
                    // Convert SchemaAttribute → ProviderFieldSchema
                    let fields: Vec<ProviderFieldSchema> = rs
                        .attributes
                        .iter()
                        .map(|attr| {
                            let field_type = if attr.sensitive {
                                ProviderFieldType::Sensitive
                            } else {
                                match attr.attr_type.as_str() {
                                    "number" => ProviderFieldType::Number,
                                    "bool" => ProviderFieldType::Bool,
                                    _ => ProviderFieldType::String,
                                }
                            };
                            ProviderFieldSchema {
                                name: attr.name.clone(),
                                label: attr.name.replace('_', " "),
                                field_type,
                                required: attr.required,
                                default_value: None,
                                options: vec![],
                                help_text: if attr.description.is_empty() {
                                    None
                                } else {
                                    Some(attr.description.clone())
                                },
                            }
                        })
                        .collect();
                    return Ok(fields);
                }
            }
        }
    }
    drop(cache);

    // Fall back to static catalog
    let static_catalog = crate::tofu::resource_catalog::get_resource_catalog();
    if let Some(entry) = static_catalog.iter().find(|e| e.resource_type == resource_type) {
        return Ok(entry.fields.clone());
    }

    Ok(vec![])
}

/// Helper: run a tofu command synchronously and return stdout.
async fn run_tofu_sync(
    state: &State<'_, AppState>,
    target: &TofuExecutionTarget,
    working_dir: &str,
    args: &[&str],
) -> Result<String, String> {
    match target {
        TofuExecutionTarget::Local => {
            let binary = if which::which("tofu").is_ok() {
                "tofu"
            } else if which::which("terraform").is_ok() {
                "terraform"
            } else {
                return Err("OpenTofu CLI not found".to_string());
            };

            let result = tokio::process::Command::new(binary)
                .args(args)
                .current_dir(working_dir)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .stdin(Stdio::null())
                .output()
                .await
                .map_err(|e| format!("Failed to run tofu: {}", e))?;

            if !result.status.success() {
                let stderr = String::from_utf8_lossy(&result.stderr).to_string();
                return Err(format!(
                    "Command failed (exit {}): {}",
                    result.status.code().unwrap_or(-1),
                    stderr
                ));
            }

            Ok(String::from_utf8_lossy(&result.stdout).to_string())
        }
        TofuExecutionTarget::Ssh { connection_id } => {
            let ssh_mgr = state.ssh_manager.clone();
            let manager = ssh_mgr.lock().await;
            let handle = manager
                .get_handle(connection_id)
                .map_err(|e| e.to_string())?;

            let cmd = format!(
                "cd {} && tofu {}",
                runner::shell_escape(working_dir),
                args.join(" ")
            );
            exec_on_connection(&handle, &cmd)
                .await
                .map_err(|e| e.to_string())
        }
    }
}

/// Helper: run a tofu command synchronously, return stdout on success, error with stderr on failure.
async fn run_tofu_sync_full(
    state: &State<'_, AppState>,
    target: &TofuExecutionTarget,
    working_dir: &str,
    args: &[&str],
) -> Result<String, String> {
    match target {
        TofuExecutionTarget::Local => {
            let binary = if which::which("tofu").is_ok() {
                "tofu"
            } else if which::which("terraform").is_ok() {
                "terraform"
            } else {
                return Err("OpenTofu CLI not found".to_string());
            };

            let result = tokio::process::Command::new(binary)
                .args(args)
                .current_dir(working_dir)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .stdin(Stdio::null())
                .output()
                .await
                .map_err(|e| format!("Failed to run tofu: {}", e))?;

            let stdout = String::from_utf8_lossy(&result.stdout).to_string();
            let stderr = String::from_utf8_lossy(&result.stderr).to_string();

            // fmt returns exit code 3 when files need formatting (check mode)
            if result.status.success() || result.status.code() == Some(3) {
                Ok(stdout)
            } else {
                Err(format!("{}{}", stderr, stdout))
            }
        }
        TofuExecutionTarget::Ssh { connection_id } => {
            let ssh_mgr = state.ssh_manager.clone();
            let manager = ssh_mgr.lock().await;
            let handle = manager
                .get_handle(connection_id)
                .map_err(|e| e.to_string())?;

            let cmd = format!(
                "cd {} && tofu {}",
                runner::shell_escape(working_dir),
                args.join(" ")
            );
            exec_on_connection(&handle, &cmd)
                .await
                .map_err(|e| e.to_string())
        }
    }
}
