//! The Containers workspace: Docker and Podman engines and Compose projects,
//! on a server reached through SSH or on this computer. Every command checks
//! the DevOps switch first, so a workspace that is off does nothing.

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::State;

use crate::container::compose::{ComposeAction, Project};
use crate::container::{self, Action, ContainerRow, Engine, HostInfo, ImageRow, LogView, NetworkRow, VolumeRow};
use crate::logfiles::{self, FoundFile, LogRule};
use crate::db::types::Route;
use crate::devops::{self, Tool};
use crate::devops_store::ContainerHost;
use crate::state::AppState;

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Saved hosts

#[tauri::command]
pub async fn ctr_hosts(state: State<'_, AppState>) -> Result<Vec<ContainerHost>, String> {
    devops::require(Tool::Containers)?;
    let mut vault = state.vault_manager.lock().await;
    let mut store = state.devops_store.lock().await;
    store.ensure_loaded(&mut vault).await?;
    Ok(store.hosts())
}

/// Save a host; an empty id makes a new one. Returns it as saved.
#[tauri::command]
pub async fn ctr_host_save(state: State<'_, AppState>, mut host: ContainerHost) -> Result<ContainerHost, String> {
    devops::require(Tool::Containers)?;
    if host.name.trim().is_empty() {
        return Err("Give the host a name".into());
    }
    if matches!(host.route, Route::Live { .. }) {
        return Err("A terminal tab's connection ends with the tab; save the session instead".into());
    }
    if host.id.is_empty() {
        host.id = uuid::Uuid::new_v4().to_string();
    }
    host.name = host.name.trim().to_string();
    let mut vault = state.vault_manager.lock().await;
    let mut store = state.devops_store.lock().await;
    store.ensure_loaded(&mut vault).await?;
    store.save_host(host.clone(), &mut vault).await?;
    Ok(host)
}

#[tauri::command]
pub async fn ctr_host_delete(state: State<'_, AppState>, id: String) -> Result<(), String> {
    devops::require(Tool::Containers)?;
    state.containers.lock().await.close(&id);
    let mut vault = state.vault_manager.lock().await;
    let mut store = state.devops_store.lock().await;
    store.ensure_loaded(&mut vault).await?;
    store.delete(&id, &mut vault).await
}

// ---------------------------------------------------------------------------
// Connecting

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Opened {
    pub key: String,
    pub info: HostInfo,
    pub read_only: bool,
}

/// Connect to a saved host. Its route, engine and read-only setting come
/// from the vault, never from the page. Reconnecting replaces the previous
/// connection to the same host.
#[tauri::command]
pub async fn ctr_open(app: tauri::AppHandle, state: State<'_, AppState>, host_id: String) -> Result<Opened, String> {
    devops::require(Tool::Containers)?;
    let saved = {
        let mut vault = state.vault_manager.lock().await;
        let mut store = state.devops_store.lock().await;
        store.ensure_loaded(&mut vault).await?;
        store.host(&host_id).cloned().ok_or("That host is no longer saved")?
    };
    let ssh = crate::ipc::db_commands::ssh_for_route(&app, &state, &saved.route).await?;
    let mut host = container::open(saved.engine, ssh).await?;
    host.read_only = saved.read_only;
    let read_only = saved.read_only;
    let info = state.containers.lock().await.insert(host_id.clone(), host);
    {
        let mut vault = state.vault_manager.lock().await;
        let mut store = state.devops_store.lock().await;
        let _ = store.save_host(ContainerHost { last_used_at: now_ms(), ..saved }, &mut vault).await;
    }
    Ok(Opened { key: host_id, info, read_only })
}

/// Containers on a terminal tab's own server, without saving anything.
#[tauri::command]
pub async fn ctr_open_live(app: tauri::AppHandle, state: State<'_, AppState>, connection_id: String, engine: Engine) -> Result<Opened, String> {
    devops::require(Tool::Containers)?;
    let route = Route::Live { connection_id: connection_id.clone() };
    let ssh = crate::ipc::db_commands::ssh_for_route(&app, &state, &route).await?;
    let host = container::open(engine, ssh).await?;
    let key = format!("live:{}:{connection_id}", engine.cli());
    let info = state.containers.lock().await.insert(key.clone(), host);
    Ok(Opened { key, info, read_only: false })
}

#[tauri::command]
pub async fn ctr_close(state: State<'_, AppState>, key: String) -> Result<(), String> {
    state.containers.lock().await.close(&key);
    Ok(())
}

async fn host(state: &State<'_, AppState>, key: &str) -> Result<std::sync::Arc<container::Host>, String> {
    devops::require(Tool::Containers)?;
    state.containers.lock().await.get(key)
}

// ---------------------------------------------------------------------------
// Reading

#[tauri::command]
pub async fn ctr_containers(state: State<'_, AppState>, key: String) -> Result<Vec<ContainerRow>, String> {
    host(&state, &key).await?.containers().await
}

#[tauri::command]
pub async fn ctr_images(state: State<'_, AppState>, key: String) -> Result<Vec<ImageRow>, String> {
    host(&state, &key).await?.images().await
}

#[tauri::command]
pub async fn ctr_volumes(state: State<'_, AppState>, key: String) -> Result<Vec<VolumeRow>, String> {
    host(&state, &key).await?.volumes().await
}

#[tauri::command]
pub async fn ctr_networks(state: State<'_, AppState>, key: String) -> Result<Vec<NetworkRow>, String> {
    host(&state, &key).await?.networks().await
}

#[tauri::command]
pub async fn ctr_projects(state: State<'_, AppState>, key: String) -> Result<Vec<Project>, String> {
    host(&state, &key).await?.projects().await
}

#[tauri::command]
pub async fn ctr_inspect(state: State<'_, AppState>, key: String, kind: String, id: String) -> Result<String, String> {
    host(&state, &key).await?.inspect(&kind, &id).await
}

// ---------------------------------------------------------------------------
// Changing (each refuses on a read-only host)

#[tauri::command]
pub async fn ctr_act(state: State<'_, AppState>, key: String, id: String, action: Action) -> Result<(), String> {
    host(&state, &key).await?.act(&id, action).await
}

#[tauri::command]
pub async fn ctr_remove(state: State<'_, AppState>, key: String, kind: String, id: String) -> Result<(), String> {
    let h = host(&state, &key).await?;
    match kind.as_str() {
        "image" => h.remove_image(&id).await,
        "volume" => h.remove_volume(&id).await,
        "network" => h.remove_network(&id).await,
        other => Err(format!("Unknown kind {other}")),
    }
}

#[tauri::command]
pub async fn ctr_compose(state: State<'_, AppState>, key: String, project: String, action: ComposeAction) -> Result<String, String> {
    host(&state, &key).await?.compose(&project, action).await
}

// ---------------------------------------------------------------------------
// Logs and shell

/// Follow a container's log into `on_data` until [`ctr_logs_stop`].
#[tauri::command]
pub async fn ctr_logs(
    state: State<'_, AppState>,
    key: String,
    id: String,
    stream_id: String,
    tail: Option<u32>,
    view: Option<LogView>,
    on_data: Channel<String>,
) -> Result<(), String> {
    let h = host(&state, &key).await?;
    let view = view.unwrap_or(LogView { tail: tail.unwrap_or(500), since_secs: None, timestamps: false });
    state.containers.lock().await.follow_logs(stream_id, h, id, view, on_data);
    Ok(())
}

/// The log files in a running container, for the file picker (#101).
#[tauri::command]
pub async fn ctr_log_files(state: State<'_, AppState>, key: String, id: String) -> Result<Vec<FoundFile>, String> {
    host(&state, &key).await?.find_log_files(&id).await
}

/// The log files saved for a host's or cluster's containers.
#[tauri::command]
pub async fn devops_log_rules(state: State<'_, AppState>, owner: String) -> Result<Vec<LogRule>, String> {
    let mut vault = state.vault_manager.lock().await;
    let mut store = state.devops_store.lock().await;
    store.ensure_loaded(&mut vault).await?;
    Ok(store.log_rules(&owner))
}

/// Save them, in the vault with the host, carried to every device.
#[tauri::command]
pub async fn devops_log_rules_save(state: State<'_, AppState>, owner: String, rules: Vec<LogRule>) -> Result<(), String> {
    if rules.len() > 200 {
        return Err("Too many saved log sources".into());
    }
    for r in &rules {
        logfiles::check_paths(&r.paths)?;
    }
    let mut vault = state.vault_manager.lock().await;
    let mut store = state.devops_store.lock().await;
    store.ensure_loaded(&mut vault).await?;
    store.save_log_rules(&owner, rules, &mut vault).await
}

/// Follow files inside a container, as `tail -F`, into `on_data` until
/// [`ctr_logs_stop`]. Reading only: a read-only host allows it.
#[tauri::command]
pub async fn ctr_logs_files(
    state: State<'_, AppState>,
    key: String,
    id: String,
    stream_id: String,
    paths: Vec<String>,
    lines: Option<u32>,
    on_data: Channel<String>,
) -> Result<(), String> {
    let paths = logfiles::check_paths(&paths)?;
    let h = host(&state, &key).await?;
    state.containers.lock().await.follow_files(stream_id, h, id, paths, lines.unwrap_or(500).min(100_000), on_data);
    Ok(())
}

#[tauri::command]
pub async fn ctr_logs_stop(state: State<'_, AppState>, stream_id: String) -> Result<(), String> {
    state.containers.lock().await.stop_stream(&stream_id);
    Ok(())
}

/// The shell command that opens a terminal inside a container: bash if the
/// image has it, sh otherwise. The terminal tab runs it on the same host.
#[tauri::command]
pub fn ctr_shell_command(engine: Engine, id: String) -> Result<String, String> {
    devops::require(Tool::Containers)?;
    Ok(format!(
        "{} exec -it {} sh -c 'if command -v bash >/dev/null 2>&1; then exec bash; else exec sh; fi'",
        engine.cli(),
        container::sh_quote(&id)
    ))
}
