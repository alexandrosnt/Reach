use tracing::info;
use tauri::Emitter;
use crate::state::AppState;
use crate::sftp::browser::{self, RemoteEntry};

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

    tokio::spawn(async move {
        if let Err(e) = crate::sftp::transfer::upload_file(
            &handle, &local_path, &remote_path, &tid, &app,
        ).await {
            tracing::error!("Upload failed for {}: {}", tid, e);
            let _ = app.emit(&format!("transfer-error-{}", tid), e.to_string());
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

    tokio::spawn(async move {
        if let Err(e) = crate::sftp::transfer::download_file(
            &handle, &remote_path, &local_path, &tid, &app,
        ).await {
            tracing::error!("Download failed for {}: {}", tid, e);
            let _ = app.emit(&format!("transfer-error-{}", tid), e.to_string());
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
