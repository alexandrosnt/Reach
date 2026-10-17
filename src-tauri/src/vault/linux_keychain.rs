//! Linux: the desktop's keychain first, the kernel keyring as the fallback.
//!
//! The kernel keyring (keyutils) is emptied by every reboot and expires after
//! a few days (issue #87), so a key kept only there is lost. The Secret
//! Service (GNOME Keyring, KWallet) keeps it on disk, encrypted with the login
//! password, as every desktop app's saved passwords are. Not every Linux has
//! one (a server, a bare window manager), so the kernel keyring stays as the
//! fallback, and as the copy a key already stored by an earlier Reach is
//! found in: read there, it is copied to the Secret Service.
//!
//! zbus's blocking calls must not run inside an async runtime, which is where
//! Reach calls the keychain from, so each Secret Service call runs on a short
//! thread of its own. That thread is not waited on forever: a locked keychain
//! whose unlock prompt never appears leaves the call unanswered, and Reach
//! hung with it (issue #87, on Arch-based systems). After one unanswered call
//! the rest of the run uses the kernel keyring only.

use std::any::Any;
use std::collections::HashMap;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use keyring_core::api::{CredentialApi, CredentialStoreApi};
use keyring_core::{Credential, CredentialStore, Entry, Error, Result};

/// How long a Secret Service call may take: enough for someone to answer an
/// unlock prompt, not forever for one that never appears.
const ANSWER_WITHIN: Duration = Duration::from_secs(30);

/// Set once a Secret Service call went unanswered. Waiting again on every
/// later call would only hang the app 30 seconds at a time.
static UNANSWERED: AtomicBool = AtomicBool::new(false);

/// Runs `f` on a thread of its own and waits for it, up to [`ANSWER_WITHIN`].
fn off_runtime<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T> {
    if UNANSWERED.load(Ordering::Relaxed) {
        return Err(Error::PlatformFailure("the Secret Service did not answer earlier in this run".into()));
    }
    match std::panic::catch_unwind(AssertUnwindSafe(|| super::deadline::call_with_deadline(ANSWER_WITHIN, f))) {
        Ok(Some(v)) => Ok(v),
        Ok(None) => {
            UNANSWERED.store(true, Ordering::Relaxed);
            tracing::warn!(
                "The Secret Service did not answer within {}s (a locked keychain whose unlock prompt never appeared?); \
                 until Reach restarts the vault key is kept in the kernel keyring only, which a reboot empties",
                ANSWER_WITHIN.as_secs()
            );
            Err(Error::PlatformFailure("the Secret Service did not answer".into()))
        }
        Err(_) => Err(Error::PlatformFailure("the Secret Service call panicked".into())),
    }
}

pub struct Store {
    /// None where the session has no Secret Service.
    secret_service: Option<Arc<CredentialStore>>,
    keyutils: Arc<CredentialStore>,
}

impl Store {
    pub fn new() -> Result<Arc<Self>> {
        let keyutils: Arc<CredentialStore> = linux_keyutils_keyring_store::Store::new_with_configuration(&HashMap::from([(
            "prefix",
            "keyring-rs:",
        )]))?;
        let secret_service = match off_runtime(zbus_secret_service_keyring_store::Store::new).and_then(|r| r) {
            Ok(s) => Some(s as Arc<CredentialStore>),
            Err(e) => {
                tracing::warn!("No Secret Service in this session ({e}); the vault key goes to the kernel keyring only, which a reboot empties");
                None
            }
        };
        Ok(Arc::new(Store { secret_service, keyutils }))
    }
}

impl std::fmt::Debug for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LinuxKeychain").field("secret_service", &self.secret_service.is_some()).finish()
    }
}

impl CredentialStoreApi for Store {
    fn vendor(&self) -> String {
        "Reach: Secret Service, then the kernel keyring".into()
    }

    fn id(&self) -> String {
        "reach-linux-keychain".into()
    }

    fn build(&self, service: &str, user: &str, _modifiers: Option<&HashMap<&str, &str>>) -> Result<Entry> {
        let ss = match &self.secret_service {
            Some(s) => {
                let (s, service, user) = (s.clone(), service.to_string(), user.to_string());
                off_runtime(move || s.build(&service, &user, None)).and_then(|r| r).ok().map(Arc::new)
            }
            None => None,
        };
        let ku = self.keyutils.build(service, user, None)?;
        Ok(Entry::new_with_credential(Arc::new(Cred { ss, ku, service: service.into(), user: user.into() })))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

struct Cred {
    ss: Option<Arc<Entry>>,
    ku: Entry,
    service: String,
    user: String,
}

impl Cred {
    /// The Secret Service entry, unless the Secret Service stopped answering:
    /// then the kernel keyring alone holds the key, as where there is none.
    fn ss(&self) -> Option<Arc<Entry>> {
        self.ss.clone().filter(|_| !UNANSWERED.load(Ordering::Relaxed))
    }

    fn ss_set(&self, secret: &[u8]) -> Option<Result<()>> {
        let e = self.ss()?;
        let secret = secret.to_vec();
        Some(off_runtime(move || e.set_secret(&secret)).and_then(|r| r))
    }
}

impl CredentialApi for Cred {
    fn set_secret(&self, secret: &[u8]) -> Result<()> {
        let ss = self.ss_set(secret);
        let ku = self.ku.set_secret(secret);
        match ss {
            // The keychain that lasts holds it: a kernel keyring failure
            // only costs the fallback copy. An older key left there would be
            // read if the Secret Service is ever unreachable, so it goes.
            Some(Ok(())) => {
                if let Err(e) = ku {
                    tracing::debug!("kernel keyring copy not stored: {e}");
                    let _ = self.ku.delete_credential();
                }
                Ok(())
            }
            // An older key left in the Secret Service would be read first, so
            // it goes; if it cannot be removed, the store failed.
            Some(Err(e)) => {
                tracing::warn!("Secret Service did not store the key ({e}); kept in the kernel keyring, which a reboot empties");
                ku?;
                match self.ss().map(|s| off_runtime(move || s.delete_credential()).and_then(|r| r)) {
                    Some(Ok(())) | Some(Err(Error::NoEntry)) | None => Ok(()),
                    Some(Err(_)) => Err(e),
                }
            }
            None => ku,
        }
    }

    fn get_secret(&self) -> Result<Vec<u8>> {
        if let Some(e) = self.ss() {
            match off_runtime(move || e.get_secret()).and_then(|r| r) {
                Ok(secret) => return Ok(secret),
                Err(Error::NoEntry) => {}
                Err(e) => tracing::warn!("Secret Service read failed ({e}); trying the kernel keyring"),
            }
        }
        let secret = self.ku.get_secret()?;
        // Stored by an earlier Reach, or while there was no Secret Service:
        // keep it where a reboot cannot take it.
        if let Some(Err(e)) = self.ss_set(&secret) {
            tracing::warn!("could not copy the key to the Secret Service: {e}");
        }
        Ok(secret)
    }

    fn delete_credential(&self) -> Result<()> {
        let ss = self.ss().map(|e| off_runtime(move || e.delete_credential()).and_then(|r| r));
        let ku = self.ku.delete_credential();
        match (ss, ku) {
            (Some(Err(Error::NoEntry)) | None, Err(Error::NoEntry)) => Err(Error::NoEntry),
            (Some(Err(e)), _) if !matches!(e, Error::NoEntry) => Err(e),
            (_, Err(e)) if !matches!(e, Error::NoEntry) => Err(e),
            _ => Ok(()),
        }
    }

    fn get_credential(&self) -> Result<Option<Arc<Credential>>> {
        Ok(None)
    }

    fn get_specifiers(&self) -> Option<(String, String)> {
        Some((self.service.clone(), self.user.clone()))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use keyring_core::api::CredentialStoreApi;
    use std::time::{Duration, Instant};

    /// Issue #87: a Secret Service that never answers must not hang Reach.
    /// Needs one: run under `dbus-run-session` with gnome-keyring holding a
    /// locked default collection and no display for its unlock prompt.
    #[test]
    #[ignore]
    fn an_unanswered_secret_service_falls_back_to_the_kernel_keyring() {
        let store = super::Store::new().expect("store");
        let entry = store.build("reach-unanswered-test", "user", None).expect("entry");
        let started = Instant::now();
        entry.set_secret(b"key").expect("kept in the kernel keyring");
        assert_eq!(entry.get_secret().expect("read back"), b"key");
        assert!(started.elapsed() < super::ANSWER_WITHIN + Duration::from_secs(10), "took {:?}", started.elapsed());
        let _ = entry.delete_credential();
    }
}
