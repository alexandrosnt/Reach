//! Asking for a password manager's passphrase in the app's window. Built like
//! the SSH login questions (`ssh::prompt`): an event to the window, the
//! caller parked on a oneshot until `pwmanager_passphrase_response` answers,
//! and the question withdrawn (an event again) however it ends.
//!
//! What comes back from the window is moved straight into a [`Passphrase`]
//! (shielded, the plain `String` wiped) and never logged or sent back.
//!
//! Every open question is cancelled at once ([`cancel_all`]) whenever the
//! kept passphrases are forgotten: a vault lock, a settings save, "Forget
//! now", the computer locking or going to sleep. A question left open
//! across a lock would otherwise take a passphrase for a vault that is no
//! longer open.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Mutex, MutexGuard, OnceLock};

use serde::Serialize;
use tauri::Emitter;

use super::unlock::{Answer, Passphrase, PassphraseAsker, PromptRequest};

/// The question, to the window.
pub const PROMPT_EVENT: &str = "pwmanager-passphrase-prompt";
/// The question is withdrawn (answered, timed out, or its caller gone); the
/// payload is its id.
pub const CLOSED_EVENT: &str = "pwmanager-passphrase-prompt-closed";

type Reply = tokio::sync::oneshot::Sender<Option<Passphrase>>;

/// One open question: where its answer goes, and the window to tell when it
/// is withdrawn (`None` in tests).
struct Pending {
    reply: Reply,
    app: Option<tauri::AppHandle>,
}

fn pending() -> MutexGuard<'static, HashMap<String, Pending>> {
    static P: OnceLock<Mutex<HashMap<String, Pending>>> = OnceLock::new();
    P.get_or_init(|| Mutex::new(HashMap::new())).lock().unwrap_or_else(|e| e.into_inner())
}

/// Cancel every open question, as if the user had pressed Cancel on each,
/// and tell the window they are withdrawn. Returns how many there were.
pub(crate) fn cancel_all() -> usize {
    let drained: Vec<(String, Pending)> = pending().drain().collect();
    let count = drained.len();
    for (id, p) in drained {
        let _ = p.reply.send(None);
        if let Some(app) = p.app {
            let _ = app.emit(CLOSED_EVENT, &id);
        }
    }
    if count > 0 {
        tracing::info!("pwmanager: {count} open passphrase question(s) cancelled");
    }
    count
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
    /// The server's host name, so questions for different accounts can be
    /// told apart. Not secret.
    server: Option<&'a str>,
}

/// Withdrawn however the question ends.
struct Open {
    id: String,
    app: tauri::AppHandle,
}

impl Drop for Open {
    fn drop(&mut self) {
        pending().remove(&self.id);
        let _ = self.app.emit(CLOSED_EVENT, &self.id);
    }
}

impl PassphraseAsker for tauri::AppHandle {
    fn ask(&self, request: PromptRequest) -> Pin<Box<dyn Future<Output = Answer> + Send + '_>> {
        Box::pin(async move {
            let id = uuid::Uuid::new_v4().to_string();
            let (tx, rx) = tokio::sync::oneshot::channel();
            pending().insert(id.clone(), Pending { reply: tx, app: Some(self.clone()) });
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
                server: request.server.as_deref(),
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
    let reply = pending().remove(prompt_id);
    match reply {
        Some(p) => {
            let _ = p.reply.send(answer);
            Ok(())
        }
        None => Err("This passphrase question is no longer open.".into()),
    }
}

#[cfg(test)]
#[path = "prompt_tests.rs"]
mod tests;
