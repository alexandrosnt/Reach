use tracing::info;
use tauri::Emitter;
use crate::state::AppState;
use crate::sftp::browser::{self, RemoteEntry};
use crate::plugin::hooks;

/// List the contents of a remote directory.
#[tauri::command]
pub async fn sftp_list_dir(
    state: tauri::State<'_, AppState>,
    connection_id: String,
    path: String,
) -> Result<Vec<RemoteEntry>, String> {
    info!("sftp_list_dir called: conn={}, path={}", connection_id, path);
    let handle = {
        let manager = state.ssh_manager.lock().await;
        manager.get_handle(&connection_id).map_err(|e| {
            info!("sftp_list_dir handle error: {}", e);
            e.to_string()
        })?
    };
    let result = browser::list_directory(&handle, &path)
        .await
        .map_err(|e| {
            info!("sftp_list_dir browse error: {}", e);
            e.to_string()
        })?;
    info!("sftp_list_dir returning {} entries for {}", result.len(), path);
    Ok(result)
}

/// Upload a local file to the remote host. Returns the transfer_id immediately
/// and runs the upload in a background task, emitting progress events.
#[tauri::command]
pub async fn sftp_upload(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    connection_id: String,
    local_path: String,
    remote_path: String,
) -> Result<String, String> {
    let handle = {
        let manager = state.ssh_manager.lock().await;
        manager.get_handle(&connection_id).map_err(|e| e.to_string())?
    };
    let transfer_id = uuid::Uuid::new_v4().to_string();
    let tid = transfer_id.clone();
    let plugin_mgr = state.plugin_manager.clone();
    let conn_id = connection_id.clone();
    let rpath = remote_path.clone();

    tokio::spawn(async move {
        if let Err(e) = crate::sftp::transfer::upload_file(
            &handle, &local_path, &remote_path, &tid, &app,
        ).await {
            tracing::error!("Upload failed for {}: {}", tid, e);
            let _ = app.emit(&format!("transfer-error-{}", tid), e.to_string());
        } else {
            let hook = hooks::sftp_upload_complete(&conn_id, &rpath);
            let mut mgr = plugin_mgr.lock().await;
            mgr.dispatch_hook(&hook, Some(&app)).await;
        }
    });

    Ok(transfer_id)
}

/// Download a file from the remote host. Returns the transfer_id immediately
/// and runs the download in a background task, emitting progress events.
#[tauri::command]
pub async fn sftp_download(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    connection_id: String,
    remote_path: String,
    local_path: String,
) -> Result<String, String> {
    let handle = {
        let manager = state.ssh_manager.lock().await;
        manager.get_handle(&connection_id).map_err(|e| e.to_string())?
    };
    let transfer_id = uuid::Uuid::new_v4().to_string();
    let tid = transfer_id.clone();
    let plugin_mgr = state.plugin_manager.clone();
    let conn_id = connection_id.clone();
    let rpath = remote_path.clone();
    let lpath = local_path.clone();

    tokio::spawn(async move {
        if let Err(e) = crate::sftp::transfer::download_file(
            &handle, &remote_path, &local_path, &tid, &app,
        ).await {
            tracing::error!("Download failed for {}: {}", tid, e);
            let _ = app.emit(&format!("transfer-error-{}", tid), e.to_string());
        } else {
            let hook = hooks::sftp_download_complete(&conn_id, &rpath, &lpath);
            let mut mgr = plugin_mgr.lock().await;
            mgr.dispatch_hook(&hook, Some(&app)).await;
        }
    });

    Ok(transfer_id)
}

/// Delete a file or directory on the remote host.
#[tauri::command]
pub async fn sftp_delete(
    state: tauri::State<'_, AppState>,
    connection_id: String,
    path: String,
) -> Result<(), String> {
    let handle = {
        let manager = state.ssh_manager.lock().await;
        manager.get_handle(&connection_id).map_err(|e| e.to_string())?
    };
    browser::delete_entry(&handle, &path)
        .await
        .map_err(|e| e.to_string())
}

/// Rename or move a file on the remote host.
#[tauri::command]
pub async fn sftp_rename(
    state: tauri::State<'_, AppState>,
    connection_id: String,
    old_path: String,
    new_path: String,
) -> Result<(), String> {
    let handle = {
        let manager = state.ssh_manager.lock().await;
        manager.get_handle(&connection_id).map_err(|e| e.to_string())?
    };
    browser::rename_entry(&handle, &old_path, &new_path)
        .await
        .map_err(|e| e.to_string())
}

/// Create an empty file on the remote host.
#[tauri::command]
pub async fn sftp_touch(
    state: tauri::State<'_, AppState>,
    connection_id: String,
    path: String,
) -> Result<(), String> {
    let handle = {
        let manager = state.ssh_manager.lock().await;
        manager.get_handle(&connection_id).map_err(|e| e.to_string())?
    };
    browser::touch_file(&handle, &path)
        .await
        .map_err(|e| e.to_string())
}

/// Read a text file's content from the remote host.
#[tauri::command]
pub async fn sftp_read_file(
    state: tauri::State<'_, AppState>,
    connection_id: String,
    path: String,
) -> Result<String, String> {
    info!("sftp_read_file called: conn={}, path={}", connection_id, path);
    let handle = {
        let manager = state.ssh_manager.lock().await;
        manager.get_handle(&connection_id).map_err(|e| e.to_string())?
    };
    browser::read_text_file(&handle, &path)
        .await
        .map_err(|e| e.to_string())
}

/// Write text content to a remote file.
#[tauri::command]
pub async fn sftp_write_file(
    state: tauri::State<'_, AppState>,
    connection_id: String,
    path: String,
    content: String,
) -> Result<(), String> {
    info!("sftp_write_file called: conn={}, path={}", connection_id, path);
    let handle = {
        let manager = state.ssh_manager.lock().await;
        manager.get_handle(&connection_id).map_err(|e| e.to_string())?
    };
    browser::write_text_file(&handle, &path, &content)
        .await
        .map_err(|e| e.to_string())
}

/// Create a directory on the remote host.
#[tauri::command]
pub async fn sftp_mkdir(
    state: tauri::State<'_, AppState>,
    connection_id: String,
    path: String,
) -> Result<(), String> {
    let handle = {
        let manager = state.ssh_manager.lock().await;
        manager.get_handle(&connection_id).map_err(|e| e.to_string())?
    };
    browser::make_directory(&handle, &path)
        .await
        .map_err(|e| e.to_string())
}

// --- Archives -------------------------------------------------------------
//
// The commands themselves live in `sftp::archive`, which decides what to run
// and is tested there. These three do the talking: probe once, then run what
// the probe says is possible. Nothing here builds a command from a filename.

use crate::sftp::archive::{self, ArchiveFormat, ArchiveTools};

/// Which archive tools the remote machine has.
///
/// Worth calling once when the explorer opens and remembering, so the menu can
/// offer only what will actually work. Cheap: one command, one round trip.
#[tauri::command]
pub async fn sftp_archive_tools(
    state: tauri::State<'_, AppState>,
    connection_id: String,
) -> Result<ArchiveTools, String> {
    let handle = {
        let manager = state.ssh_manager.lock().await;
        manager.get_handle(&connection_id).map_err(|e| e.to_string())?
    };
    let (stdout, _stderr, _code) =
        crate::ssh::client::exec_on_connection_with_exit_code(&handle, &archive::probe_command())
            .await
            .map_err(|e| e.to_string())?;
    let tools = ArchiveTools::parse(&stdout);
    info!(
        "sftp_archive_tools: conn={} found {:?}",
        connection_id, tools.present
    );
    Ok(tools)
}

/// What a finished archive operation has to say for itself.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveOutcome {
    /// The tool that ended up doing the work, so the user can be told.
    pub tool: String,
    /// What was produced: the archive, or the directory extracted into.
    pub produced: String,
}

/// Turn a failed command into something worth reading.
///
/// A bare exit code tells the user nothing. The last line of stderr is almost
/// always the real reason — "No space left on device" and the like.
fn failure(tool: &str, stderr: &str, code: i32) -> String {
    let detail = stderr
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .next_back()
        .unwrap_or("no output");
    format!("{tool} failed (exit {code}): {detail}")
}

/// Compress `entries` inside `directory` into a new archive.
#[tauri::command]
pub async fn sftp_archive_create(
    state: tauri::State<'_, AppState>,
    connection_id: String,
    directory: String,
    entries: Vec<String>,
    archive_name: String,
    format: ArchiveFormat,
) -> Result<ArchiveOutcome, String> {
    let handle = {
        let manager = state.ssh_manager.lock().await;
        manager.get_handle(&connection_id).map_err(|e| e.to_string())?
    };
    let tools = {
        let (stdout, _, _) = crate::ssh::client::exec_on_connection_with_exit_code(
            &handle,
            &archive::probe_command(),
        )
        .await
        .map_err(|e| e.to_string())?;
        ArchiveTools::parse(&stdout)
    };

    let plan = archive::create_command(&tools, format, &directory, &entries, &archive_name)
        .ok_or_else(|| {
            format!(
                "This machine has no tool that can create a {} archive.",
                format.extension()
            )
        })?;

    let (_out, stderr, code) =
        crate::ssh::client::exec_on_connection_with_exit_code(&handle, &plan.command)
            .await
            .map_err(|e| e.to_string())?;
    if code != 0 {
        return Err(failure(&plan.tool, &stderr, code));
    }
    Ok(ArchiveOutcome {
        tool: plan.tool,
        produced: archive_name,
    })
}

/// Extract `archive_name` inside `directory`, into a directory of its own.
///
/// The destination is always a fresh subdirectory rather than the current one:
/// an archive full of loose files should not scatter them over whatever the
/// user happens to be looking at, and a hostile archive should not be able to
/// quietly replace a neighbour.
#[tauri::command]
pub async fn sftp_archive_extract(
    state: tauri::State<'_, AppState>,
    connection_id: String,
    directory: String,
    archive_name: String,
) -> Result<ArchiveOutcome, String> {
    let handle = {
        let manager = state.ssh_manager.lock().await;
        manager.get_handle(&connection_id).map_err(|e| e.to_string())?
    };
    let tools = {
        let (stdout, _, _) = crate::ssh::client::exec_on_connection_with_exit_code(
            &handle,
            &archive::probe_command(),
        )
        .await
        .map_err(|e| e.to_string())?;
        ArchiveTools::parse(&stdout)
    };

    let dest = archive::extract_dir_name(&archive_name);
    let plan = archive::extract_command(&tools, &directory, &archive_name, &dest)
        .ok_or_else(|| "This machine has no tool that can open that archive.".to_string())?;

    let (_out, stderr, code) =
        crate::ssh::client::exec_on_connection_with_exit_code(&handle, &plan.command)
            .await
            .map_err(|e| e.to_string())?;
    if code != 0 {
        return Err(failure(&plan.tool, &stderr, code));
    }
    Ok(ArchiveOutcome {
        tool: plan.tool,
        produced: dest,
    })
}
