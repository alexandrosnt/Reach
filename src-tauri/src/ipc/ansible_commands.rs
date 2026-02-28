use tauri::State;

use crate::ansible::project::AnsibleProjectManager;
use crate::ansible::runner;
use crate::ansible::types::{
    AnsibleCollection, AnsibleCommandRequest, AnsibleExecutionTarget, AnsibleInventoryGroup,
    AnsibleInventoryHost, AnsibleProject, AnsibleRole,
};
use crate::state::AppState;

#[tauri::command]
pub async fn ansible_list_projects(
    state: State<'_, AppState>,
) -> Result<Vec<AnsibleProject>, String> {
    let mut mgr = state.ansible_project_manager.lock().await;
    let mut vault_mgr = state.vault_manager.lock().await;
    mgr.ensure_loaded(&mut vault_mgr).await?;
    Ok(mgr.list_projects())
}

#[tauri::command]
pub async fn ansible_create_project(
    state: State<'_, AppState>,
    name: String,
    path: String,
    description: String,
) -> Result<AnsibleProject, String> {
    let id = uuid::Uuid::new_v4().to_string();
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

    AnsibleProjectManager::scaffold_project(&path, &name)?;

    let project = AnsibleProject {
        id,
        name,
        path,
        description,
        created_at: now.clone(),
        last_opened_at: now,
        inventory_hosts: vec![],
        inventory_groups: vec![],
        vault_password: None,
    };

    let mut mgr = state.ansible_project_manager.lock().await;
    let mut vault_mgr = state.vault_manager.lock().await;
    mgr.ensure_loaded(&mut vault_mgr).await?;
    mgr.add_project(project.clone(), &mut vault_mgr).await?;
    Ok(project)
}

#[tauri::command]
pub async fn ansible_delete_project(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<(), String> {
    let mut mgr = state.ansible_project_manager.lock().await;
    let mut vault_mgr = state.vault_manager.lock().await;
    mgr.ensure_loaded(&mut vault_mgr).await?;
    mgr.remove_project(&project_id, &mut vault_mgr).await
}

#[tauri::command]
pub async fn ansible_open_project(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<AnsibleProject, String> {
    let mut mgr = state.ansible_project_manager.lock().await;
    let mut vault_mgr = state.vault_manager.lock().await;
    mgr.ensure_loaded(&mut vault_mgr).await?;
    mgr.touch_project(&project_id, &mut vault_mgr).await?;
    mgr.get_project(&project_id)
        .cloned()
        .ok_or_else(|| "Project not found".to_string())
}

#[tauri::command]
pub async fn ansible_update_inventory(
    state: State<'_, AppState>,
    project_id: String,
    hosts: Vec<AnsibleInventoryHost>,
    groups: Vec<AnsibleInventoryGroup>,
) -> Result<AnsibleProject, String> {
    let mut mgr = state.ansible_project_manager.lock().await;
    let mut vault_mgr = state.vault_manager.lock().await;
    mgr.ensure_loaded(&mut vault_mgr).await?;
    let mut project = mgr
        .get_project(&project_id)
        .cloned()
        .ok_or_else(|| "Project not found".to_string())?;
    project.inventory_hosts = hosts;
    project.inventory_groups = groups;
    mgr.update_project(project.clone(), &mut vault_mgr).await?;
    Ok(project)
}

#[tauri::command]
pub async fn ansible_list_files(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Vec<String>, String> {
    let mgr = state.ansible_project_manager.lock().await;
    let project = mgr
        .get_project(&project_id)
        .ok_or_else(|| "Project not found".to_string())?;

    let dir = std::path::Path::new(&project.path);
    if !dir.exists() {
        return Ok(vec![]);
    }

    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Ok(ft) = entry.file_type() {
                if ft.is_file() {
                    if let Some(name) = entry.file_name().to_str() {
                        let ext = std::path::Path::new(name)
                            .extension()
                            .and_then(|e| e.to_str())
                            .unwrap_or("");
                        match ext {
                            "yml" | "yaml" | "ini" | "cfg" | "j2" | "json" | "txt" | "md" => {
                                files.push(name.to_string());
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    files.sort();
    Ok(files)
}

#[tauri::command]
pub async fn ansible_read_file(
    state: State<'_, AppState>,
    project_id: String,
    filename: String,
) -> Result<String, String> {
    let mgr = state.ansible_project_manager.lock().await;
    let project = mgr
        .get_project(&project_id)
        .ok_or_else(|| "Project not found".to_string())?;

    let file_path = std::path::Path::new(&project.path).join(&filename);
    std::fs::read_to_string(&file_path)
        .map_err(|e| format!("Failed to read file: {}", e))
}

#[tauri::command]
pub async fn ansible_write_file(
    state: State<'_, AppState>,
    project_id: String,
    filename: String,
    content: String,
) -> Result<(), String> {
    let mgr = state.ansible_project_manager.lock().await;
    let project = mgr
        .get_project(&project_id)
        .ok_or_else(|| "Project not found".to_string())?;

    let file_path = std::path::Path::new(&project.path).join(&filename);
    std::fs::write(&file_path, content)
        .map_err(|e| format!("Failed to write file: {}", e))
}

#[tauri::command]
pub async fn ansible_run_command(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
    request: AnsibleCommandRequest,
) -> Result<String, String> {
    let run_id = uuid::Uuid::new_v4().to_string();

    let mgr = state.ansible_project_manager.lock().await;
    let project = mgr
        .get_project(&request.project_id)
        .ok_or_else(|| "Project not found".to_string())?
        .clone();
    drop(mgr);

    let working_dir = project.path.clone();
    let (binary, args) = runner::build_command_args(&request);
    let target = request.target.clone();
    let rid = run_id.clone();

    let ssh_mgr = state.ssh_manager.clone();

    tokio::spawn(async move {
        match target {
            AnsibleExecutionTarget::Local => {
                let _ =
                    runner::run_local(&working_dir, &binary, &args, &rid, &app_handle).await;
            }
            AnsibleExecutionTarget::Ssh { connection_id } => {
                let mut ssh = ssh_mgr.lock().await;
                let _ = runner::run_remote(
                    &connection_id,
                    &working_dir,
                    &binary,
                    &args,
                    &rid,
                    &app_handle,
                    &mut ssh,
                )
                .await;
            }
        }
    });

    Ok(run_id)
}

#[tauri::command]
pub async fn ansible_generate_inventory(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<String, String> {
    let mgr = state.ansible_project_manager.lock().await;
    let project = mgr
        .get_project(&project_id)
        .ok_or_else(|| "Project not found".to_string())?;

    let mut ini = String::new();

    // Build a map of groups to their hosts
    let mut group_hosts: std::collections::HashMap<String, Vec<&AnsibleInventoryHost>> =
        std::collections::HashMap::new();
    let mut ungrouped: Vec<&AnsibleInventoryHost> = vec![];

    for host in &project.inventory_hosts {
        if host.groups.is_empty() {
            ungrouped.push(host);
        } else {
            for group in &host.groups {
                group_hosts.entry(group.clone()).or_default().push(host);
            }
        }
    }

    // Write ungrouped hosts
    if !ungrouped.is_empty() {
        ini.push_str("[all]\n");
        for host in &ungrouped {
            ini.push_str(&format_host_line(host));
        }
        ini.push('\n');
    }

    // Write grouped hosts
    for group in &project.inventory_groups {
        ini.push_str(&format!("[{}]\n", group.name));
        if let Some(hosts) = group_hosts.get(&group.name) {
            for host in hosts {
                ini.push_str(&format_host_line(host));
            }
        }
        ini.push('\n');

        // Group variables
        if !group.variables.is_empty() {
            ini.push_str(&format!("[{}:vars]\n", group.name));
            for (k, v) in &group.variables {
                ini.push_str(&format!("{}={}\n", k, v));
            }
            ini.push('\n');
        }

        // Group children
        if !group.children.is_empty() {
            ini.push_str(&format!("[{}:children]\n", group.name));
            for child in &group.children {
                ini.push_str(&format!("{}\n", child));
            }
            ini.push('\n');
        }
    }

    // Write any group hosts that don't have a matching group definition
    for (group_name, hosts) in &group_hosts {
        if !project
            .inventory_groups
            .iter()
            .any(|g| &g.name == group_name)
        {
            ini.push_str(&format!("[{}]\n", group_name));
            for host in hosts {
                ini.push_str(&format_host_line(host));
            }
            ini.push('\n');
        }
    }

    Ok(ini)
}

fn format_host_line(host: &AnsibleInventoryHost) -> String {
    let mut line = host.name.clone();
    if let Some(ref h) = host.ansible_host {
        line.push_str(&format!(" ansible_host={}", h));
    }
    if let Some(port) = host.ansible_port {
        line.push_str(&format!(" ansible_port={}", port));
    }
    if let Some(ref user) = host.ansible_user {
        line.push_str(&format!(" ansible_user={}", user));
    }
    for (k, v) in &host.variables {
        line.push_str(&format!(" {}={}", k, v));
    }
    line.push('\n');
    line
}

#[tauri::command]
pub async fn ansible_write_inventory(
    state: State<'_, AppState>,
    project_id: String,
    content: String,
    filename: Option<String>,
) -> Result<(), String> {
    let mgr = state.ansible_project_manager.lock().await;
    let project = mgr
        .get_project(&project_id)
        .ok_or_else(|| "Project not found".to_string())?;

    let fname = filename.unwrap_or_else(|| "inventory.ini".to_string());
    let file_path = std::path::Path::new(&project.path).join(&fname);
    std::fs::write(&file_path, content)
        .map_err(|e| format!("Failed to write inventory: {}", e))
}

#[tauri::command]
pub async fn ansible_list_roles(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Vec<AnsibleRole>, String> {
    let mgr = state.ansible_project_manager.lock().await;
    let project = mgr
        .get_project(&project_id)
        .ok_or_else(|| "Project not found".to_string())?;

    // Try to list roles from the project's roles directory
    let roles_dir = std::path::Path::new(&project.path).join("roles");
    let mut roles = Vec::new();
    if roles_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&roles_dir) {
            for entry in entries.flatten() {
                if let Ok(ft) = entry.file_type() {
                    if ft.is_dir() {
                        if let Some(name) = entry.file_name().to_str() {
                            roles.push(AnsibleRole {
                                name: name.to_string(),
                                version: None,
                            });
                        }
                    }
                }
            }
        }
    }
    roles.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(roles)
}

#[tauri::command]
pub async fn ansible_list_collections(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Vec<AnsibleCollection>, String> {
    let mgr = state.ansible_project_manager.lock().await;
    let project = mgr
        .get_project(&project_id)
        .ok_or_else(|| "Project not found".to_string())?;

    // Try to list collections from the project's collections directory
    let collections_dir = std::path::Path::new(&project.path).join("collections");
    let mut collections = Vec::new();
    if collections_dir.exists() {
        let ansible_collections = collections_dir.join("ansible_collections");
        if ansible_collections.exists() {
            if let Ok(namespaces) = std::fs::read_dir(&ansible_collections) {
                for ns in namespaces.flatten() {
                    if let Ok(ft) = ns.file_type() {
                        if ft.is_dir() {
                            if let Ok(colls) = std::fs::read_dir(ns.path()) {
                                for coll in colls.flatten() {
                                    if let Ok(ct) = coll.file_type() {
                                        if ct.is_dir() {
                                            let name = format!(
                                                "{}.{}",
                                                ns.file_name().to_string_lossy(),
                                                coll.file_name().to_string_lossy()
                                            );
                                            collections.push(AnsibleCollection {
                                                name,
                                                version: None,
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    collections.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(collections)
}

#[tauri::command]
pub async fn ansible_vault_view(
    state: State<'_, AppState>,
    project_id: String,
    vault_file: String,
) -> Result<String, String> {
    let mgr = state.ansible_project_manager.lock().await;
    let project = mgr
        .get_project(&project_id)
        .ok_or_else(|| "Project not found".to_string())?;

    let file_path = std::path::Path::new(&project.path).join(&vault_file);
    if !file_path.exists() {
        return Err("File not found".to_string());
    }

    // Read raw file content (viewing encrypted content)
    std::fs::read_to_string(&file_path)
        .map_err(|e| format!("Failed to read vault file: {}", e))
}
