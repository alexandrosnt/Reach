//! Telling the webview that focus has moved.
//!
//! Issue #49: after dragging the window by its title bar, an input method's
//! candidate window is drawn at the old caret position, or at the corner of
//! the screen. The reporter found the thing that fixes it: click another
//! window, then click back into Reach.
//!
//! That is the whole diagnosis. Clicking away and back is not a DOM focus
//! change — it is the *native window* losing and regaining focus, and wry
//! handles `WM_SETFOCUS` by calling `MoveFocus` on the WebView2 controller,
//! which is what makes the webview work out where the caret is again. A
//! title-bar drag never produces `WM_SETFOCUS`, so that never happens; wry
//! calls `MoveFocus` when a drag *starts* (`WM_ENTERSIZEMOVE`) and not when
//! it ends.
//!
//! So this does deliberately what clicking away and back does by accident —
//! both halves of it, because it is not certain which half matters: a real
//! focus cycle on the focused window, and then the `MoveFocus` call wry makes
//! in response to one. Doing only the second would be betting on an inference
//! rather than reproducing the gesture that is known to work.
//! Two earlier attempts failed because they worked at the wrong level: the
//! first cycled focus on the hidden textarea, the second nudged the caret's
//! rectangle. Neither touches the controller, and the controller is what
//! holds the stale position.

/// Tell the webview that focus moved to it, programmatically.
///
/// Cheap and idempotent. Does nothing anywhere but Windows, which is the only
/// platform the report concerns and the only one with this controller.
#[tauri::command]
pub async fn webview_refocus(window: tauri::WebviewWindow) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use webview2_com::Microsoft::Web::WebView2::Win32::COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC;
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::Input::KeyboardAndMouse::{GetFocus, SetFocus};

        window
            .with_webview(|webview| unsafe {
                // Two halves, because clicking away and back does two things
                // and it is not certain which one matters.
                //
                // First a real focus cycle on this thread's focused window.
                // That produces WM_KILLFOCUS then WM_SETFOCUS, which is what
                // the working gesture produces, and it makes the text
                // services framework re-associate with the window. Nothing
                // reachable from the page can cause this.
                let focused: HWND = GetFocus();
                if !focused.is_invalid() {
                    let _ = SetFocus(None);
                    let _ = SetFocus(Some(focused));
                }

                // Then the call wry itself makes on WM_SETFOCUS. Harmless if
                // the cycle above already triggered it; the point is not to
                // depend on which of the two does the work.
                let controller = webview.controller();
                let _ = controller.MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC);
            })
            .map_err(|e| e.to_string())?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = window;
    }
    Ok(())
}
