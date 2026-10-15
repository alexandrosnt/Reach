//! Asking for a password manager's passphrase in the app's window. Built like
//! the SSH login questions (`ssh::prompt`): an event to the window, the
//! caller parked on a oneshot until `pwmanager_passphrase_response` answers,
//! and the question withdrawn (an event again) however it ends.
//!
//! What comes back from the window is moved straight into a [`Passphrase`]
//! (shielded, the plain `String` wiped) and never logged or sent back.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

use serde::Serialize;
use tauri::Emitter;

use super::unlock::{Answer, Passphrase, PassphraseAsker, PromptRequest};

/// The question, to the window.
pub const PROMPT_EVENT: &str = "pwmanager-passphrase-prompt";
/// The question is withdrawn (answered, timed out, or its caller gone); the
/// payload is its id.
pub const CLOSED_EVENT: &str = "pwmanager-passphrase-prompt-closed";

type Reply = tokio::sync::oneshot::Sender<Option<Passphrase>>;

fn pending() -> &'static Mutex<HashMap<String, Reply>> {
    static P: OnceLock<Mutex<HashMap<String, Reply>>> = OnceLock::new();
    P.get_or_init(|| Mutex::new(HashMap::new()))
}

/// How many questions are open now. A connection logging in meanwhile does
/// not count that time against its connect limit (see
/// `ssh::client::within_connect_limit`): the user is busy answering.
static OPEN: AtomicUsize = AtomicUsize::new(0);

pub(crate) fn any_open() -> bool {
    OPEN.load(Ordering::SeqCst) > 0
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Payload<'a> {
    prompt_id: &'a str,
    provider: &'static str,
    provider_name: &'static str,
    /// `connect`, `list`, `test` or `other`.
    reason: &'static str,
    /// What is being connected to, for `connect`.
    target: Option<&'a str>,
    /// The previous passphrase was rejected.
    retry: bool,
    error: Option<&'a str>,
    /// How long the answer is kept in memory; 0 = not kept.
    remember_secs: u32,
}

/// Withdrawn however the question ends.
struct Open {
    id: String,
    app: tauri::AppHandle,
}

impl Drop for Open {
    fn drop(&mut self) {
        pending().lock().unwrap_or_else(|e| e.into_inner()).remove(&self.id);
        OPEN.fetch_sub(1, Ordering::SeqCst);
        let _ = self.app.emit(CLOSED_EVENT, &self.id);
    }
}

impl PassphraseAsker for tauri::AppHandle {
    fn ask(&self, request: PromptRequest) -> Pin<Box<dyn Future<Output = Answer> + Send + '_>> {
        Box::pin(async move {
            let id = uuid::Uuid::new_v4().to_string();
            let (tx, rx) = tokio::sync::oneshot::channel();
            pending().lock().unwrap_or_else(|e| e.into_inner()).insert(id.clone(), tx);
            OPEN.fetch_add(1, Ordering::SeqCst);
            let _open = Open { id: id.clone(), app: self.clone() };
            let payload = Payload {
                prompt_id: &id,
                provider: request.provider.as_str(),
                provider_name: request.provider.display_name(),
                reason: request.reason.code(),
                target: request.reason.target(),
                retry: request.retry,
                error: request.error.as_deref(),
                remember_secs: request.remember_secs,
            };
            tracing::info!(
                "pwmanager: asking for the {} passphrase ({}, retry={})",
                request.provider.as_str(),
                request.reason.code(),
                request.retry
            );
            if self.emit(PROMPT_EVENT, &payload).is_err() {
                return Answer::NoWindow;
            }
            match rx.await {
                Ok(Some(pass)) => Answer::Entered(pass),
                _ => Answer::Cancelled,
            }
        })
    }
}

/// The window's answer: the passphrase, or `None` for Cancel. A passphrase
/// Reach cannot take (empty, too long) is refused and the question stays
/// open, so the window can say why.
pub(crate) fn answer(prompt_id: &str, passphrase: Option<String>) -> Result<(), String> {
    let answer = match passphrase {
        Some(typed) => Some(Passphrase::new(typed)?),
        None => None,
    };
    let reply = pending().lock().unwrap_or_else(|e| e.into_inner()).remove(prompt_id);
    match reply {
        Some(tx) => {
            let _ = tx.send(answer);
            Ok(())
        }
        None => Err("This passphrase question is no longer open.".into()),
    }
}
