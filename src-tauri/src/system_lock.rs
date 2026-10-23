//! Noticing when the computer's own session locks or goes to sleep, so Reach
//! can lock with it (Settings → Security). The callback (in `lib.rs`) also
//! forgets any password manager passphrase kept in memory, every time,
//! whether or not Reach itself locks. Modelled on KeePassXC, which has done
//! the same for years on the same signals:
//!
//! - Windows: `WM_WTSSESSION_CHANGE` / `WTS_SESSION_LOCK` from
//!   `WTSRegisterSessionNotification`, and `PBT_APMSUSPEND` from
//!   `RegisterSuspendResumeNotification`, delivered to a message-only window.
//! - macOS: the `com.apple.screenIsLocked` distributed notification and
//!   `NSWorkspaceWillSleepNotification`.
//! - Linux: logind's `Session.Lock` and `Manager.PrepareForSleep(true)` on the
//!   system bus, and the desktop screensaver's `ActiveChanged(true)` on the
//!   session bus (freedesktop and GNOME).
//!
//! Android has no such signal for an app; the interface locks when Reach has
//! been in the background too long instead. Watching is best-effort: whatever
//! a platform refuses is logged, and Reach runs on without it.

use std::sync::Arc;

pub type OnLock = Arc<dyn Fn() + Send + Sync>;

/// Start watching; `on_lock` runs, on some thread, each time the session
/// locks or the machine is about to sleep.
pub fn watch(on_lock: OnLock) {
    if let Err(e) = platform::watch(on_lock) {
        tracing::warn!("Cannot follow the computer's lock state: {}", e);
    }
}

#[cfg(windows)]
mod platform {
    use super::OnLock;
    use std::sync::OnceLock;
    use windows::core::w;
    use windows::Win32::Foundation::{HANDLE, HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Power::RegisterSuspendResumeNotification;
    use windows::Win32::System::RemoteDesktop::{WTSRegisterSessionNotification, NOTIFY_FOR_THIS_SESSION};
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, RegisterClassW, TranslateMessage,
        DEVICE_NOTIFY_WINDOW_HANDLE, HWND_MESSAGE, MSG, WINDOW_EX_STYLE, WINDOW_STYLE, WNDCLASSW,
    };

    const WM_WTSSESSION_CHANGE: u32 = 0x02B1;
    const WTS_SESSION_LOCK: usize = 0x7;
    const WM_POWERBROADCAST: u32 = 0x0218;
    const PBT_APMSUSPEND: usize = 0x4;

    static ON_LOCK: OnceLock<OnLock> = OnceLock::new();

    unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        let locked = (msg == WM_WTSSESSION_CHANGE && wparam.0 == WTS_SESSION_LOCK)
            || (msg == WM_POWERBROADCAST && wparam.0 == PBT_APMSUSPEND);
        if locked {
            if let Some(on_lock) = ON_LOCK.get() {
                on_lock();
            }
        }
        // SAFETY: passing the message on unchanged, as a window procedure must.
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    }

    pub fn watch(on_lock: OnLock) -> Result<(), String> {
        if ON_LOCK.set(on_lock).is_err() {
            return Ok(());
        }
        std::thread::Builder::new()
            .name("session-lock-watch".into())
            .spawn(|| {
                // SAFETY: a message-only window owned by this thread, with a
                // class whose procedure is the function above; the message
                // loop runs for the life of the process.
                unsafe {
                    let instance = match GetModuleHandleW(None) {
                        Ok(h) => h,
                        Err(e) => return tracing::warn!("GetModuleHandleW: {e}"),
                    };
                    let class = w!("ReachSessionLockWatch");
                    let wc = WNDCLASSW {
                        lpfnWndProc: Some(window_proc),
                        hInstance: instance.into(),
                        lpszClassName: class,
                        ..Default::default()
                    };
                    RegisterClassW(&wc);
                    let hwnd = match CreateWindowExW(
                        WINDOW_EX_STYLE(0),
                        class,
                        w!(""),
                        WINDOW_STYLE(0),
                        0,
                        0,
                        0,
                        0,
                        Some(HWND_MESSAGE),
                        None,
                        Some(instance.into()),
                        None,
                    ) {
                        Ok(h) => h,
                        Err(e) => return tracing::warn!("CreateWindowExW: {e}"),
                    };
                    if let Err(e) = WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION) {
                        tracing::warn!("WTSRegisterSessionNotification: {e}");
                    }
                    if let Err(e) = RegisterSuspendResumeNotification(HANDLE(hwnd.0), DEVICE_NOTIFY_WINDOW_HANDLE) {
                        tracing::warn!("RegisterSuspendResumeNotification: {e}");
                    }
                    let mut msg = MSG::default();
                    while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                        let _ = TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }
            })
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use super::OnLock;
    use block2::RcBlock;
    use objc2_app_kit::{NSWorkspace, NSWorkspaceWillSleepNotification};
    use objc2_foundation::{NSDistributedNotificationCenter, NSNotification, NSString};
    use std::ptr::NonNull;

    pub fn watch(on_lock: OnLock) -> Result<(), String> {
        let screen = on_lock.clone();
        let screen_block = RcBlock::new(move |_: NonNull<NSNotification>| screen());
        let sleep_block = RcBlock::new(move |_: NonNull<NSNotification>| on_lock());
        // SAFETY: registering observers with the system notification centres;
        // the tokens are kept for the life of the process so the observers
        // are never removed while a notification could arrive.
        unsafe {
            let distributed = NSDistributedNotificationCenter::defaultCenter();
            let token = distributed.addObserverForName_object_queue_usingBlock(
                Some(&NSString::from_str("com.apple.screenIsLocked")),
                None,
                None,
                &screen_block,
            );
            std::mem::forget(token);
            let workspace = NSWorkspace::sharedWorkspace().notificationCenter();
            let token = workspace.addObserverForName_object_queue_usingBlock(
                Some(NSWorkspaceWillSleepNotification),
                None,
                None,
                &sleep_block,
            );
            std::mem::forget(token);
        }
        std::mem::forget(screen_block);
        std::mem::forget(sleep_block);
        Ok(())
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use super::OnLock;
    use zbus::blocking::{Connection, MessageIterator};
    use zbus::MatchRule;

    /// One signal to follow: which bus, the match, and whether the signal's
    /// first argument must be `true` (going to sleep, screensaver on).
    struct Watch {
        system: bool,
        rule: &'static str,
        needs_true: bool,
    }

    const WATCHES: &[Watch] = &[
        Watch { system: true, rule: "type='signal',interface='org.freedesktop.login1.Session',member='Lock'", needs_true: false },
        Watch { system: true, rule: "type='signal',interface='org.freedesktop.login1.Manager',member='PrepareForSleep'", needs_true: true },
        Watch { system: false, rule: "type='signal',interface='org.freedesktop.ScreenSaver',member='ActiveChanged'", needs_true: true },
        Watch { system: false, rule: "type='signal',interface='org.gnome.ScreenSaver',member='ActiveChanged'", needs_true: true },
    ];

    pub fn watch(on_lock: OnLock) -> Result<(), String> {
        for w in WATCHES {
            let on_lock = on_lock.clone();
            std::thread::Builder::new()
                .name("session-lock-watch".into())
                .spawn(move || {
                    if let Err(e) = follow(w, &on_lock) {
                        tracing::debug!("Not following {}: {}", w.rule, e);
                    }
                })
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn follow(w: &Watch, on_lock: &OnLock) -> Result<(), zbus::Error> {
        let conn = if w.system { Connection::system()? } else { Connection::session()? };
        let rule = MatchRule::try_from(w.rule)?;
        for msg in MessageIterator::for_match_rule(rule, &conn, None)? {
            let Ok(msg) = msg else { continue };
            let fire = if w.needs_true { msg.body().deserialize::<(bool,)>().map(|(b,)| b).unwrap_or(false) } else { true };
            if fire {
                on_lock();
            }
        }
        Ok(())
    }
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
mod platform {
    pub fn watch(_: super::OnLock) -> Result<(), String> {
        Ok(())
    }
}
