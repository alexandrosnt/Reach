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

/// How many files an operation is about to touch.
///
/// Emitted before the work starts so the progress bar has a denominator. Zero
/// means the count failed or is not knowable, and the bar runs indeterminate
/// rather than the operation being held up for it.
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveTotal {
    pub operation_id: String,
    pub total: u64,
    pub tool: String,
    /// False when the tool says nothing, or when its output cannot be
    /// unbuffered — in which case a count would be stuck at zero and then
    /// jump, which is worse than showing no number at all.
    pub measurable: bool,
    /// The name actually used, which may carry a suffix if the one asked for
    /// was taken. Cancelling removes this, so it has to be the real one.
    pub produced: String,
}

/// Pick an output name that does not already exist.
///
/// Compressing over an existing archive, or extracting into an existing
/// directory, would destroy or merge with something the user already had —
/// and would make cancelling unsafe, since the output might not be ours to
/// delete. Falls back to the plain name if the machine cannot answer, which
/// is the old behaviour rather than a failure.
async fn resolve_free_name(
    handle: &crate::ssh::client::SharedHandle,
    directory: &str,
    base: &str,
) -> String {
    let cmd = archive::resolve_free_name_command(directory, base);
    match crate::ssh::client::exec_on_connection_with_exit_code(handle, &cmd).await {
        Ok((out, _, 0)) if !out.trim().is_empty() => out.trim().to_string(),
        _ => base.to_string(),
    }
}

/// Ask the machine how many files are involved. Never fatal.
async fn count_files(handle: &crate::ssh::client::SharedHandle, command: Option<String>) -> u64 {
    let Some(cmd) = command else { return 0 };
    match crate::ssh::client::exec_on_connection_with_exit_code(handle, &cmd).await {
        Ok((out, _, 0)) => out.trim().parse().unwrap_or(0),
        _ => 0,
    }
}

/// Compress `entries` inside `directory` into a new archive.
///
/// Streams the tool's output as it goes, under `archive-output-<operationId>`,
/// so the caller can count files as they are written. That is why the commands
/// are built verbose: a quiet tool cannot be measured.
#[tauri::command]
pub async fn sftp_archive_create(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    connection_id: String,
    directory: String,
    entries: Vec<String>,
    archive_name: String,
    format: ArchiveFormat,
    operation_id: String,
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

    let archive_name = resolve_free_name(&handle, &directory, &archive_name).await;
    let plan = archive::create_command(&tools, format, &directory, &entries, &archive_name)
        .ok_or_else(|| {
            format!(
                "This machine has no tool that can create a {} archive.",
                format.extension()
            )
        })?;

    let total = count_files(&handle, archive::count_to_create(&directory, &entries)).await;
    let _ = app.emit(
        &format!("archive-total-{}", operation_id),
        ArchiveTotal {
            operation_id: operation_id.clone(),
            total,
            tool: plan.tool.clone(),
            measurable: archive::is_measurable(&tools, &plan.tool),
            produced: archive_name.clone(),
        },
    );

    let code = crate::ssh::client::exec_on_connection_streaming(
        &handle,
        &archive::make_cancellable(&plan.command, &operation_id),
        &operation_id,
        "archive-output",
        &app,
    )
    .await
    .map_err(|e| e.to_string())?;
    if code != 0 {
        return Err(format!("{} failed (exit {})", plan.tool, code));
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
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    connection_id: String,
    directory: String,
    archive_name: String,
    operation_id: String,
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

    let dest = resolve_free_name(&handle, &directory, &archive::extract_dir_name(&archive_name)).await;
    let plan = archive::extract_command(&tools, &directory, &archive_name, &dest)
        .ok_or_else(|| "This machine has no tool that can open that archive.".to_string())?;

    let total = count_files(
        &handle,
        archive::count_to_extract(&tools, &directory, &archive_name),
    )
    .await;
    let _ = app.emit(
        &format!("archive-total-{}", operation_id),
        ArchiveTotal {
            operation_id: operation_id.clone(),
            total,
            tool: plan.tool.clone(),
            measurable: archive::is_measurable(&tools, &plan.tool),
            produced: dest.clone(),
        },
    );

    let code = crate::ssh::client::exec_on_connection_streaming(
        &handle,
        &archive::make_cancellable(&plan.command, &operation_id),
        &operation_id,
        "archive-output",
        &app,
    )
    .await
    .map_err(|e| e.to_string())?;
    if code != 0 {
        return Err(format!("{} failed (exit {})", plan.tool, code));
    }
    Ok(ArchiveOutcome {
        tool: plan.tool,
        produced: dest,
    })
}

/// Stop a running archive operation and remove what it had produced.
///
/// Safe to delete the output because the name was resolved to one that did not
/// exist when the operation started, so whatever is there now was written by
/// this run. Runs on its own channel, since the one doing the work is busy.
#[tauri::command]
pub async fn sftp_archive_cancel(
    state: tauri::State<'_, AppState>,
    connection_id: String,
    operation_id: String,
    directory: String,
    produced: String,
    is_directory: bool,
) -> Result<(), String> {
    let handle = {
        let manager = state.ssh_manager.lock().await;
        manager.get_handle(&connection_id).map_err(|e| e.to_string())?
    };
    let cmd = archive::cancel_command(&directory, &produced, is_directory, &operation_id);
    info!("sftp_archive_cancel: conn={} op={}", connection_id, operation_id);
    // The kill may race the process ending on its own, which is not a failure.
    let _ = crate::ssh::client::exec_on_connection_with_exit_code(&handle, &cmd).await;
    Ok(())
}
