//! The RDP surface the webview calls.
//!
//! Thin on purpose: every command is a lock, a lookup and one call into
//! [`crate::rdp::RdpManager`], which is where the protocol mapping lives.
//! Frames do not pass through here at all — `rdp_connect` hands the manager a
//! channel and the pixels go straight down it for the life of the session.

use tauri::ipc::Channel;

use crate::rdp::{mouse_pdu, RdpConnectParams};
use crate::state::AppState;

/// Open a desktop. Frames arrive on `on_frame` as raw bytes; lifecycle
/// arrives as `rdp-status-{id}` events.
#[tauri::command]
pub async fn rdp_connect(
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
    params: RdpConnectParams,
    on_frame: Channel,
) -> Result<(), String> {
    let id = params.id.clone();
    state
        .rdp_manager
        .lock()
        .await
        .connect(app_handle, params, on_frame)
        .map_err(|e| {
            // The panel shows this; the log keeps it.
            tracing::warn!("RDP {id}: connect rejected: {e}");
            e
        })
}

#[tauri::command]
pub async fn rdp_disconnect(state: tauri::State<'_, AppState>, id: String) -> Result<(), String> {
    state.rdp_manager.lock().await.disconnect(&id)
}

/// Every open desktop, on the way out of the app.
#[tauri::command]
pub async fn rdp_disconnect_all(state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.rdp_manager.lock().await.disconnect_all();
    Ok(())
}

/// `action` is one of `move`, `down`, `up`, `wheel`; `button` is 0 left,
/// 1 middle, 2 right; `delta` is the wheel step, negative for away.
#[tauri::command]
pub async fn rdp_mouse(
    state: tauri::State<'_, AppState>,
    id: String,
    x: u16,
    y: u16,
    action: String,
    button: u8,
    delta: i16,
) -> Result<(), String> {
    let pdu = mouse_pdu(x, y, &action, button, delta)?;
    state.rdp_manager.lock().await.send_mouse(&id, pdu)
}

/// A PC/AT set-1 scancode, with `extended` for the keys that carry the E0
/// prefix (right Ctrl and Alt, the arrow cluster, the navigation block).
#[tauri::command]
pub async fn rdp_key(
    state: tauri::State<'_, AppState>,
    id: String,
    scancode: u8,
    extended: bool,
    release: bool,
) -> Result<(), String> {
    state.rdp_manager.lock().await.send_key(&id, scancode, extended, release)
}

/// A character rather than a key, for text with no scancode on this keyboard.
#[tauri::command]
pub async fn rdp_unicode(
    state: tauri::State<'_, AppState>,
    id: String,
    code: u16,
    release: bool,
) -> Result<(), String> {
    state.rdp_manager.lock().await.send_unicode(&id, code, release)
}

/// The desktop gained focus: the local clipboard may have changed since it
/// was last offered to the remote.
#[tauri::command]
pub async fn rdp_clipboard_sync(state: tauri::State<'_, AppState>, id: String) -> Result<(), String> {
    state.rdp_manager.lock().await.clipboard_sync(&id)
}

/// The window itself goes full screen (or comes back) for a desktop. Done
/// here rather than through the web-side window API so it needs no
/// capability grant and cannot be refused quietly.
///
/// A maximized window told to go full screen keeps the work-area size on
/// Windows — the screen less the taskbar — so it is un-maximized first, and
/// maximized again on the way back if that is how it was found.
#[tauri::command]
pub async fn rdp_window_fullscreen(app_handle: tauri::AppHandle, on: bool) -> Result<(), String> {
    use std::sync::atomic::{AtomicBool, Ordering};
    use tauri::Manager as _;
    static WAS_MAXIMIZED: AtomicBool = AtomicBool::new(false);

    let window = app_handle.get_webview_window("main").ok_or("no main window")?;
    let err = |e: tauri::Error| e.to_string();
    if on {
        let maximized = window.is_maximized().map_err(err)?;
        WAS_MAXIMIZED.store(maximized, Ordering::Relaxed);
        if maximized {
            window.unmaximize().map_err(err)?;
        }
        window.set_fullscreen(true).map_err(err)
    } else {
        window.set_fullscreen(false).map_err(err)?;
        if WAS_MAXIMIZED.swap(false, Ordering::Relaxed) {
            window.maximize().map_err(err)?;
        }
        Ok(())
    }
}

/// The webview painted one frame message. The pump sends the next only once
/// it hears this, so a busy screen never gets ahead of the canvas.
#[tauri::command]
pub async fn rdp_ack(state: tauri::State<'_, AppState>, id: String) -> Result<(), String> {
    state.rdp_manager.lock().await.ack(&id)
}

#[tauri::command]
pub async fn rdp_resize(
    state: tauri::State<'_, AppState>,
    id: String,
    width: u16,
    height: u16,
) -> Result<(), String> {
    state.rdp_manager.lock().await.resize(&id, width, height)
}
