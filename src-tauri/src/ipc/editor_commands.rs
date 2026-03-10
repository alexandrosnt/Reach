use tauri::{AppHandle, State, Manager, Emitter, WebviewUrl, WebviewWindowBuilder};
use crate::state::AppState;

const EDITOR_LABEL: &str = "editor-window";

#[tauri::command]
pub async fn editor_open_file(
    app: AppHandle,
    state: State<'_, AppState>,
    file: serde_json::Value,
) -> Result<(), String> {
    // Check if editor window exists (might be hidden)
    if let Some(win) = app.get_webview_window(EDITOR_LABEL) {
        // Show it if hidden, send file, focus
        let _ = win.show();
        win.emit("editor-open-file", &file).map_err(|e| e.to_string())?;
        let _ = win.set_focus();
        return Ok(());
    }

    // No editor window at all — store pending file and create new one
    {
        let mut pending = state.pending_editor_file.lock().await;
        *pending = Some(file);
    }

    let win = WebviewWindowBuilder::new(&app, EDITOR_LABEL, WebviewUrl::App("/?editor=true".into()))
        .title("Editor")
        .inner_size(900.0, 700.0)
        .decorations(false)
        .center()
        .build()
        .map_err(|e| e.to_string())?;

    let _ = win.set_focus();
    Ok(())
}

/// Called by the editor window to hide itself instead of closing
#[tauri::command]
pub async fn editor_hide_window(app: AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window(EDITOR_LABEL) {
        win.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn editor_get_pending_file(
    state: State<'_, AppState>,
) -> Result<Option<serde_json::Value>, String> {
    let mut pending = state.pending_editor_file.lock().await;
    Ok(pending.take())
}
