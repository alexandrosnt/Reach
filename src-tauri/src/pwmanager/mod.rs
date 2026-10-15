//! External password managers.
//!
//! A saved session, or a database connection, can point a secret field at an
//! entry in a password manager instead of keeping the secret in Reach's own
//! vault. What is stored is a [`SecretRef`]: which manager, which entry, and a
//! label for the editor to show. The secret itself is fetched here, in the
//! backend, at the moment a connection needs it, handed to the code that logs
//! in, and never sent to the webview. Nothing fetched is cached: the next
//! connect asks the manager again.
//!
//! Off by default (Settings → Password managers). Every command that talks to
//! a manager checks [`require`] first, so "off" means nothing runs, not just
//! that the settings page is hidden.
//!
//! Each manager is one [`PasswordManager`] implementation and one
//! [`ProviderId`] arm; dispatch is a `match`, not a trait object. The first is
//! Passbolt, driven through its official command-line client (see
//! [`passbolt`]). Desktop only: on Android and iOS there is no CLI to run, and
//! the provider says so in its status.
//!
//! The manager's own login may need a passphrase the user chose not to
//! store. Then it is asked for in the window when a call needs it, and kept
//! encrypted in memory for as long as the user said, or only for the one
//! operation (a connect, a listing) that asked: see [`unlock`]. Every call
//! that can ask runs inside an operation ([`with_prompt`]), which carries the
//! window to ask in and what to say the passphrase is for.

mod cache;
pub mod config;
pub mod passbolt;
#[cfg(desktop)]
mod process;
pub(crate) mod prompt;
pub mod unlock;

use std::future::Future;
use std::sync::Arc;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::state::AppState;
use config::PasswordManagersConfig;

/// A pointer to a secret held by a password manager. Holds no secret itself,
/// so it may be logged, shown and shared with the session.
///
/// A session can sit in a vault other people write to, and a reference is
/// resolved with the connecting person's own manager login. So a reference
/// only counts once that person has said this session may send this entry
/// to where it connects: each such word is a signature in `approvals`, made
/// the way ssh_config approvals are (see `ssh::sshconf::approvals` and
/// `ipc::session_commands::ref_slots`). `trusted` says whether the person
/// reading the session gave one that still holds; it is worked out on every
/// read and never taken from what is sent in.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecretRef {
    /// Which manager: `"passbolt"`.
    pub provider: String,
    /// The manager's own id for the entry (Passbolt: the resource UUID).
    pub entry_id: String,
    /// The entry's name when it was picked. For display only: never used to
    /// find the entry, and never trusted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Signed approvals, one per person who allowed this entry for this
    /// session as it is. Stored with the session.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub approvals: Vec<String>,
    /// The reading person has approved this reference for the session as it
    /// now is. Sent to the editor and the session list; never read from
    /// input, never stored.
    #[serde(default, skip_deserializing, skip_serializing_if = "is_false")]
    pub trusted: bool,
    /// The editor's word that the person picked (or confirmed) this reference
    /// in this edit, so saving signs it. Read from input only, never sent
    /// back and never stored.
    #[serde(default, skip_serializing)]
    pub confirm: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

impl SecretRef {
    /// The same entry of the same manager; the label and the approvals aside.
    pub fn same_entry(&self, other: &SecretRef) -> bool {
        self.provider == other.provider && self.entry_id == other.entry_id
    }
}

/// One entry as the picker lists it: metadata only, never a secret.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct EntrySummary {
    pub id: String,
    pub name: String,
    pub username: Option<String>,
    pub uri: Option<String>,
    pub folder_id: Option<String>,
}

/// What Settings shows for one manager. Finding the manager's tool is all
/// this does; it never logs in.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ProviderStatus {
    pub id: String,
    pub name: String,
    /// The master switch (Settings → Password managers).
    pub enabled: bool,
    /// The tool this manager needs is there (and this platform can run it).
    pub available: bool,
    /// Enough is set up to try a login.
    pub configured: bool,
    pub version: Option<String>,
    /// Where the tool was found, or why it was not.
    pub detail: Option<String>,
}

/// The outcome of "Test connection": a real login and a metadata listing.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TestResult {
    pub ok: bool,
    pub entries: u32,
    pub message: String,
}

/// Every manager Reach knows. Adding one is a new file with a
/// [`PasswordManager`] implementation and an arm here and in [`Provider`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProviderId {
    Passbolt,
}

impl ProviderId {
    pub const ALL: &'static [ProviderId] = &[ProviderId::Passbolt];

    pub fn as_str(self) -> &'static str {
        match self {
            ProviderId::Passbolt => "passbolt",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            ProviderId::Passbolt => "Passbolt",
        }
    }

    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "passbolt" => Ok(ProviderId::Passbolt),
            other => Err(format!("Unknown password manager '{}'", sanitize_display(other, 40))),
        }
    }
}

/// Why a call to a manager failed: the passphrase that unlocks the login was
/// wrong (worth asking again), or anything else. Both carry the user-facing
/// message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    WrongPassphrase(String),
    Other(String),
}

impl Failure {
    pub fn into_message(self) -> String {
        match self {
            Failure::WrongPassphrase(m) | Failure::Other(m) => m,
        }
    }
}

impl From<String> for Failure {
    fn from(message: String) -> Self {
        Failure::Other(message)
    }
}

/// One password manager. Each call is independent: nothing here keeps a
/// login open between calls.
pub(crate) trait PasswordManager {
    /// Find the tool and its version. No login.
    async fn status(&self) -> ProviderStatus;
    /// `Some(seconds to keep it)` when a login needs a passphrase the user
    /// types (see [`unlock`]); `None` when the configuration brings its own.
    fn passphrase_prompt(&self) -> Option<u32>;
    /// Every entry the user can see, metadata only. `passphrase` is the
    /// typed one, when [`passphrase_prompt`](Self::passphrase_prompt) asked
    /// for it.
    async fn list(&self, passphrase: Option<&unlock::Passphrase>) -> Result<Vec<EntrySummary>, Failure>;
    /// The password of one entry.
    async fn resolve(&self, entry_id: &str, passphrase: Option<&unlock::Passphrase>) -> Result<Zeroizing<String>, Failure>;
}

/// A manager, configured and ready to call. Built fresh for every call from
/// what the settings vault holds, and dropped (with the secrets it carries)
/// right after.
pub(crate) enum Provider {
    Passbolt(passbolt::Passbolt),
}

impl Provider {
    fn new(id: ProviderId, loaded: config::Loaded) -> Self {
        match id {
            ProviderId::Passbolt => Provider::Passbolt(passbolt::Passbolt::new(
                loaded.config.passbolt,
                loaded.passbolt_secrets,
                loaded.config.enabled,
            )),
        }
    }

    async fn status(&self) -> ProviderStatus {
        match self {
            Provider::Passbolt(p) => p.status().await,
        }
    }

    fn passphrase_prompt(&self) -> Option<u32> {
        match self {
            Provider::Passbolt(p) => p.passphrase_prompt(),
        }
    }

    async fn list(&self, passphrase: Option<&unlock::Passphrase>) -> Result<Vec<EntrySummary>, Failure> {
        match self {
            Provider::Passbolt(p) => p.list(passphrase).await,
        }
    }

    async fn resolve(&self, entry_id: &str, passphrase: Option<&unlock::Passphrase>) -> Result<Zeroizing<String>, Failure> {
        match self {
            Provider::Passbolt(p) => p.resolve(entry_id, passphrase).await,
        }
    }

    /// What a call must unlock, if the user is to be asked.
    fn unlock_policy(&self, account: cache::CacheKey) -> Option<unlock::Policy> {
        self.passphrase_prompt().map(|remember_secs| unlock::Policy { account, remember_secs })
    }

    /// [`list`](Self::list), asking for the passphrase if it must be.
    async fn list_unlocked(&self, account: cache::CacheKey) -> Result<Vec<EntrySummary>, String> {
        unlock::global()
            .with_passphrase(self.unlock_policy(account), |p| async move { self.list(p.as_deref()).await })
            .await
    }

    /// [`resolve`](Self::resolve), asking for the passphrase if it must be.
    async fn resolve_unlocked(&self, account: cache::CacheKey, entry_id: &str) -> Result<Zeroizing<String>, String> {
        unlock::global()
            .with_passphrase(self.unlock_policy(account), |p| async move { self.resolve(entry_id, p.as_deref()).await })
            .await
    }
}

// ---------------------------------------------------------------------------
// Asking for the passphrase.

pub use unlock::Reason;

/// Run `fut` as one operation that may ask the user for a manager's
/// passphrase in `app`'s window, saying it is for `reason`. Whatever `fut`
/// resolves, the passphrase is asked for at most once (see [`unlock`]).
pub async fn with_prompt<F: Future>(app: &tauri::AppHandle, reason: Reason, fut: F) -> F::Output {
    unlock::operation(Some(Arc::new(app.clone())), reason, fut).await
}

/// Name what the current operation connects to, once known.
pub fn name_operation(name: &str) {
    unlock::name_operation(name);
}

/// Forget every passphrase kept in memory: "Forget now", a settings save, a
/// vault lock or a new identity, quitting.
pub fn forget_passphrase() {
    unlock::global().cache.forget();
}

/// Whether a typed passphrase is kept in memory now for the account these
/// settings log in to.
pub fn passphrase_cached(loaded: &config::Loaded) -> bool {
    loaded.config.passbolt.asks_passphrase()
        && unlock::global().cache.holds(cache::CacheKey::new(ProviderId::Passbolt, loaded))
}

/// A passphrase question is open in the window now.
pub fn prompt_open() -> bool {
    prompt::any_open()
}

/// The window's answer to a passphrase question (`None`: cancelled).
pub fn answer_prompt(prompt_id: &str, passphrase: Option<String>) -> Result<(), String> {
    prompt::answer(prompt_id, passphrase)
}

/// The message every gated call returns while the master switch is off.
pub const DISABLED_MESSAGE: &str =
    "Password managers are turned off. Turn them on in Settings → Password managers.";

/// Refuse unless the user turned password managers on.
pub fn require(config: &PasswordManagersConfig) -> Result<(), String> {
    if config.enabled {
        Ok(())
    } else {
        Err(DISABLED_MESSAGE.to_string())
    }
}

// ---------------------------------------------------------------------------
// Entry listings. Metadata only (names, usernames, URLs), cached for a few
// minutes per account, one login at a time per provider: see [`cache`].

/// The most entries the picker is sent at once; the search box narrows it.
pub const MAX_LIST_RESULTS: usize = 500;

/// Forget every cached listing, and keep any listing still running from
/// being cached: the configuration changed, the feature was switched off,
/// or the vault was locked.
pub fn clear_cache() {
    cache::global().clear();
}

/// Case-insensitive substring match on name, username and URL, at most
/// [`MAX_LIST_RESULTS`] entries.
pub fn filter_entries(entries: &[EntrySummary], query: Option<&str>) -> Vec<EntrySummary> {
    let needle = query.map(str::trim).filter(|q| !q.is_empty()).map(str::to_lowercase);
    let matches = |e: &&EntrySummary| match &needle {
        None => true,
        Some(n) => {
            e.name.to_lowercase().contains(n)
                || e.username.as_deref().is_some_and(|u| u.to_lowercase().contains(n))
                || e.uri.as_deref().is_some_and(|u| u.to_lowercase().contains(n))
        }
    };
    entries.iter().filter(matches).take(MAX_LIST_RESULTS).cloned().collect()
}

/// Text from outside (an entry name, an id the frontend sent) made safe to
/// show: control characters dropped, length capped.
pub(crate) fn sanitize_display(s: &str, max_chars: usize) -> String {
    let cleaned: String = s.chars().filter(|c| !c.is_control()).collect();
    if cleaned.chars().count() > max_chars {
        let mut t: String = cleaned.chars().take(max_chars).collect();
        t.push('…');
        t
    } else {
        cleaned
    }
}

// ---------------------------------------------------------------------------
// The calls the rest of the app makes.

async fn provider_for(state: &AppState, id: ProviderId) -> Result<(Provider, cache::CacheKey), String> {
    let loaded = config::load(state).await?;
    require(&loaded.config)?;
    let key = cache::CacheKey::new(id, &loaded);
    Ok((Provider::new(id, loaded), key))
}

/// Every manager's status, for Settings. Allowed while switched off: it only
/// looks for the tool (and asks its version once switched on), it never logs
/// in.
pub async fn status_all(state: &AppState) -> Vec<ProviderStatus> {
    let loaded = config::load(state).await.unwrap_or_default();
    let mut out = Vec::with_capacity(ProviderId::ALL.len());
    for id in ProviderId::ALL {
        out.push(Provider::new(*id, loaded.clone()).status().await);
    }
    out
}

/// The entries of one manager, from the cache unless `refresh`. Callers that
/// miss the cache at the same time share one login.
pub async fn list(
    state: &AppState,
    id: ProviderId,
    query: Option<&str>,
    refresh: bool,
) -> Result<Vec<EntrySummary>, String> {
    let cache = cache::global();
    // Before the settings are read: a save from here on makes this listing
    // one that is not kept.
    let generation = cache.generation();
    let loaded = config::load(state).await?;
    require(&loaded.config)?;
    let key = cache::CacheKey::new(id, &loaded);
    let provider = Provider::new(id, loaded);
    let all = cache.get_or_fetch(key, generation, refresh, || provider.list_unlocked(key)).await?;
    Ok(filter_entries(&all, query))
}

/// Log in and list, as "Test connection" does. Failures are a result, not
/// an error, so Settings can show them in place.
pub async fn test(state: &AppState, id: ProviderId) -> Result<TestResult, String> {
    let cache = cache::global();
    let generation = cache.generation();
    let loaded = config::load(state).await?;
    require(&loaded.config)?;
    let key = cache::CacheKey::new(id, &loaded);
    let provider = Provider::new(id, loaded);
    let started = Instant::now();
    match provider.list_unlocked(key).await {
        Ok(entries) => {
            let count = u32::try_from(entries.len()).unwrap_or(u32::MAX);
            cache.store(key, generation, Arc::new(entries));
            tracing::info!(
                "pwmanager: test provider={} ok entries={count} in {}ms",
                id.as_str(),
                started.elapsed().as_millis()
            );
            Ok(TestResult {
                ok: true,
                entries: count,
                message: format!("Connected to {}: {count} entries visible", id.display_name()),
            })
        }
        Err(message) => {
            // The message is the user-facing one: the CLI's last error line,
            // cleaned and cut short, never its output or environment.
            tracing::info!(
                "pwmanager: test provider={} failed in {}ms: {message}",
                id.as_str(),
                started.elapsed().as_millis()
            );
            Ok(TestResult { ok: false, entries: 0, message })
        }
    }
}

/// Refuse a reference the connecting person has not approved. `trusted` is
/// never read from input: it is set only when a stored session is read and
/// the person's own signature for it holds (see [`SecretRef`]), so a
/// reference that came any other way is refused here too.
pub fn check_trusted(r: &SecretRef) -> Result<(), String> {
    if r.trusted {
        return Ok(());
    }
    let name = ProviderId::parse(&r.provider).map(ProviderId::display_name).unwrap_or("Password manager");
    Err(format!(
        "{name}: this session's password manager entry is not confirmed. Connect to the session from the session list to confirm it."
    ))
}

/// The secret a session's reference points at, fetched now, if the
/// connecting person approved it ([`check_trusted`]). Errors name the
/// manager and never contain a secret.
pub async fn resolve(state: &AppState, r: &SecretRef) -> Result<Zeroizing<String>, String> {
    check_trusted(r)?;
    fetch(state, r).await
}

/// [`resolve`] without the approval check, for a reference nobody but the
/// user can have written: a database connection's own password. Those are
/// kept in the `__databases__` vault, which is private (its key is wrapped
/// for this identity alone; with personal sync it goes to this identity's
/// own devices, never to a shared vault, and a full backup restore replaces
/// the identity with the one that made it), or come from the form the user
/// is filling in. The SSH leg of such a connection is a saved session and
/// goes through [`resolve`].
pub async fn resolve_own(state: &AppState, r: &SecretRef) -> Result<Zeroizing<String>, String> {
    fetch(state, r).await
}

/// [`resolve_own`], into a plain `String`.
pub async fn resolve_own_to_string(state: &AppState, r: &SecretRef) -> Result<String, String> {
    resolve_own(state, r).await.map(|s| s.as_str().to_owned())
}

async fn fetch(state: &AppState, r: &SecretRef) -> Result<Zeroizing<String>, String> {
    let id = ProviderId::parse(&r.provider)?;
    let (provider, key) = provider_for(state, id).await.map_err(|e| format!("{}: {e}", id.display_name()))?;
    let started = Instant::now();
    let result = provider.resolve_unlocked(key, &r.entry_id).await;
    tracing::info!(
        "pwmanager: resolve provider={} entry={} ok={} in {}ms",
        id.as_str(),
        sanitize_display(&r.entry_id, 64),
        result.is_ok(),
        started.elapsed().as_millis()
    );
    result
}

/// [`resolve`], into the plain `String` the login code takes. For callers
/// that hand the secret straight on.
pub async fn resolve_to_string(state: &AppState, r: &SecretRef) -> Result<String, String> {
    resolve(state, r).await.map(|s| s.as_str().to_owned())
}

/// A saved session's login with its references filled in: a reference wins
/// over whatever plaintext the session also holds.
pub async fn fill_auth_method(
    state: &AppState,
    method: &crate::state::AuthMethod,
) -> Result<crate::state::AuthMethod, String> {
    use crate::state::AuthMethod;
    let mut method = method.clone();
    match &mut method {
        AuthMethod::Password { password, password_ref: Some(r) } => {
            *password = Some(resolve_to_string(state, r).await?);
        }
        AuthMethod::Key { passphrase, passphrase_ref: Some(r), .. } => {
            *passphrase = Some(resolve_to_string(state, r).await?);
        }
        _ => {}
    }
    Ok(method)
}

/// A proxy's password filled in from its reference, if it has one.
pub async fn fill_proxy(
    state: &AppState,
    proxy: &mut Option<crate::state::ProxyConfig>,
) -> Result<(), String> {
    if let Some(p) = proxy.as_mut() {
        if let Some(r) = p.password_ref.as_ref() {
            p.password = Some(resolve_to_string(state, r).await?);
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
