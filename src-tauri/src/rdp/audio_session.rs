//! The name and icon of Reach's entry in the Windows volume mixer.
//!
//! Windows labels an audio session with its process's executable icon and
//! name unless the application sets its own through the session control.
//! cpal opens the stream and sets nothing, so the entry showed the generic
//! icon. The session exists only once the first audio arrives — some seconds
//! after login — so this looks for it a few times and labels it when found.

use std::time::Duration;

use windows::core::{Interface, HSTRING, PCWSTR};
use windows::Win32::Media::Audio::{
    eConsole, eRender, IAudioSessionControl2, IAudioSessionManager2, IMMDeviceEnumerator, MMDeviceEnumerator,
};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED};

/// Poll for this process's audio session on the default output and label it.
/// Returns at once; the work is a thread of its own.
pub fn label_when_it_appears() {
    let _ = std::thread::Builder::new()
        .name("rdp-audio-label".into())
        .spawn(|| {
            for _ in 0..20 {
                std::thread::sleep(Duration::from_secs(2));
                match label_sessions() {
                    Ok(true) => return,
                    Ok(false) => continue,
                    Err(e) => {
                        tracing::debug!("audio session label: {e}");
                        return;
                    }
                }
            }
        });
}

/// Label every session of this process on the default output device.
/// Returns whether one was found.
fn label_sessions() -> windows::core::Result<bool> {
    // SAFETY: plain COM calls on interfaces obtained from the system, on a
    // thread of our own that initialised COM for itself.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
        let manager: IAudioSessionManager2 = device.Activate(CLSCTX_ALL, None)?;
        let sessions = manager.GetSessionEnumerator()?;

        let name = HSTRING::from("Reach");
        let icon = HSTRING::from(format!(
            "{},0",
            std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default()
        ));

        let mut found = false;
        for i in 0..sessions.GetCount()? {
            let control = sessions.GetSession(i)?;
            let control2: IAudioSessionControl2 = control.cast()?;
            if control2.GetProcessId()? != std::process::id() {
                continue;
            }
            control.SetDisplayName(PCWSTR(name.as_ptr()), std::ptr::null())?;
            control.SetIconPath(PCWSTR(icon.as_ptr()), std::ptr::null())?;
            found = true;
        }
        Ok(found)
    }
}
