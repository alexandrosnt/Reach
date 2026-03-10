use tauri::State;
use crate::state::AppState;

#[cfg(desktop)]
use tauri::{AppHandle, Manager, Emitter, WebviewUrl, WebviewWindowBuilder};

#[cfg(desktop)]
const EDITOR_LABEL: &str = "editor-window";

#[cfg(desktop)]
#[tauri::command]
pub async fn editor_open_file(
    app: AppHandle,
    state: State<'_, AppState>,
    file: serde_json::Value,
) -> Result<(), String> {
    // Check if editor window exists (might be hidden)
    if let Some(win) = app.get_webview_window(EDITOR_LABEL) {
        // Show it if hidden, send file, focus
        let _ = win.as_ref().show();
        win.emit("editor-open-file", &file).map_err(|e| e.to_string())?;
        let _ = win.as_ref().set_focus();
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
        .map_err(|e: tauri::Error| e.to_string())?;

    let _ = win.as_ref().set_focus();
    Ok(())
}

#[cfg(not(desktop))]
#[tauri::command]
pub async fn editor_open_file(
    _state: State<'_, AppState>,
    _file: serde_json::Value,
) -> Result<(), String> {
    Err("Editor is not available on this platform".to_string())
}

#[cfg(desktop)]
#[tauri::command]
pub async fn editor_hide_window(app: AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window(EDITOR_LABEL) {
        win.as_ref().hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(not(desktop))]
#[tauri::command]
pub async fn editor_hide_window() -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub async fn editor_get_pending_file(
    state: State<'_, AppState>,
) -> Result<Option<serde_json::Value>, String> {
    let mut pending = state.pending_editor_file.lock().await;
    Ok(pending.take())
}
