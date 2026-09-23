//! Starting a promise drag from the interface.
//!
//! Nothing is transferred when this is called. It hands the drop target a
//! description of the files and a way to ask for their contents; the transfer
//! begins only if the drop actually happens, and the target writes the bytes
//! wherever the user let go. A cancelled drag costs nothing.

use crate::state::AppState;
use serde::Deserialize;

/// One file being offered to the drop target.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DragFile {
    /// The name the copy should be given.
    pub name: String,
    /// Where to read it from, on the remote machine.
    pub path: String,
    /// Known ahead of time, so the drop target can show progress.
    pub size: u64,
}

/// Begin dragging remote files out of the window.
///
/// Blocks until the drop happens or the drag is abandoned, because that is
/// how the platform's drag loop works. Returns whether anything was dropped.
#[tauri::command]
pub async fn dragout_start(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    connection_id: String,
    files: Vec<DragFile>,
) -> Result<bool, String> {
    #[cfg(target_os = "windows")]
    {
        use crate::dragout::windows::{drag_promised_files, PromisedFile};

        let handle = {
            let manager = state.ssh_manager.lock().await;
            manager.get_handle(&connection_id).map_err(|e| e.to_string())?
        };

        let promised: Vec<PromisedFile> = files
            .into_iter()
            .map(|file| {
                let handle = handle.clone();
                let path = file.path.clone();
                PromisedFile {
                    name: file.name,
                    size: file.size,
                    // Called by the drop target after the drop, never before.
                    open: Box::new(move || {
                        // Bounded, so a slow write on the far end stops us
                        // reading rather than filling memory.
                        let (tx, rx) = std::sync::mpsc::sync_channel::<Vec<u8>>(64);
                        let handle = handle.clone();
                        let path = path.clone();
                        tauri::async_runtime::spawn(async move {
                            if let Err(e) =
                                crate::sftp::transfer::stream_file(&handle, &path, tx).await
                            {
                                tracing::warn!("drag transfer of {} ended: {}", path, e);
                            }
                        });
                        rx
                    }),
                }
            })
            .collect();

        let (tx, rx) = std::sync::mpsc::channel();
        app.run_on_main_thread(move || {
            let _ = tx.send(drag_promised_files(promised));
        })
        .map_err(|e| e.to_string())?;
        rx.recv().map_err(|e| e.to_string())?
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (app, state, connection_id, files);
        Err("Promise drag is only implemented on Windows so far".into())
    }
}
