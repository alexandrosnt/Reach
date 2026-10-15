//! The passphrase of a password manager login, when it is asked for instead
//! of stored (Settings → Password managers → Passphrase: Ask when needed).
//!
//! Three pieces, all in memory, nothing ever written:
//!
//! - **[`Passphrase`]**: what the user typed, held encrypted with the
//!   vault's key shield ([`Shielded`]: sealed under a per-process prekey in
//!   locked memory) and opened only for the moment it is handed to the CLI.
//! - **[`PassphraseCache`]**: the passphrase kept for the user's chosen time,
//!   filed under the account it unlocks (the list cache's [`CacheKey`]:
//!   server, private key, credentials mode, CLI), so a different account
//!   never gets it. The time is a fixed deadline from when it was typed, not
//!   extended by use: "remember for 15 minutes" means fifteen minutes after
//!   the user typed it, however often it is used meanwhile, so a passphrase
//!   in steady use still goes away on schedule. Checked on every read, and a
//!   timer drops it at the deadline even if nothing reads it. Cleared too on
//!   a settings save, a vault lock, a new vault identity, "Forget now",
//!   quitting, and when the manager says it is wrong.
//! - **[`PromptHub`]**: the question to the user. One open question per
//!   account however many callers need the answer (a jump chain resolving
//!   three entries, a picker and a connect at once): the first asks, the
//!   rest wait for the same answer.
//!
//! An **operation** ([`operation`]) is one thing the user asked for: a
//! connect, a listing, a test. Whatever it needs, it asks for at most once:
//! the passphrase it got is kept with the operation until it ends, even when
//! the setting says not to keep it at all, so connecting through two jump
//! hosts with an entry each asks once, not three times. An operation also
//! carries what to ask with (the app's window) and why, for the question's
//! text.
//!
//! When the manager rejects the passphrase, it is forgotten (from the
//! operation and the cache, if it is still the one there) and the user is
//! asked again, saying so, up to [`MAX_ATTEMPTS`] times per operation.

use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use tokio::sync::watch;
use zeroize::Zeroizing;

use super::cache::CacheKey;
use super::config::{clamp_remember, MAX_PASSPHRASE_BYTES};
use super::{Failure, ProviderId};
use crate::vault::shield::Shielded;

/// How long the question waits for an answer before the operation gives up.
pub const PROMPT_TIMEOUT: Duration = Duration::from_secs(180);
/// How many passphrases the manager may reject in one operation before it
/// fails.
pub const MAX_ATTEMPTS: u32 = 3;

/// The length prefix in front of the passphrase in its shielded buffer.
const LEN_PREFIX: usize = 2;
const BUF_LEN: usize = LEN_PREFIX + MAX_PASSPHRASE_BYTES;
const _: () = assert!(MAX_PASSPHRASE_BYTES <= u16::MAX as usize, "the length prefix is a u16");

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

// ---------------------------------------------------------------------------
// The passphrase

/// A passphrase the user typed. Held encrypted in a fixed-size buffer (the
/// length in front, the rest zeros), so its length is all an observer of the
/// sealed bytes could tell, and not even that. No `Clone`; shared as an
/// `Arc`. Its `Debug` prints nothing of it.
pub struct Passphrase {
    /// Which typing this is: a wrong-passphrase answer forgets this one
    /// only, not one typed since.
    serial: u64,
    shielded: Shielded<BUF_LEN>,
}

impl Passphrase {
    /// Take what the window sent, shield it and wipe the plain copy.
    pub fn new(typed: String) -> Result<Self, String> {
        static SERIALS: AtomicU64 = AtomicU64::new(1);
        let typed = Zeroizing::new(typed);
        if typed.is_empty() {
            return Err("Enter the passphrase.".into());
        }
        if typed.len() > MAX_PASSPHRASE_BYTES {
            return Err(format!("The passphrase is too long (at most {MAX_PASSPHRASE_BYTES} bytes)."));
        }
        if typed.contains('\0') {
            return Err("The passphrase contains a NUL character.".into());
        }
        let bytes = typed.as_bytes();
        let len = u16::try_from(bytes.len()).map_err(|_| "The passphrase is too long.".to_string())?;
        let shielded = Shielded::from_fn(|buf| {
            buf[..LEN_PREFIX].copy_from_slice(&len.to_le_bytes());
            buf[LEN_PREFIX..LEN_PREFIX + bytes.len()].copy_from_slice(bytes);
        });
        Ok(Self { serial: SERIALS.fetch_add(1, Ordering::Relaxed), shielded })
    }

    pub fn serial(&self) -> u64 {
        self.serial
    }

    /// Open it for the length of `use_it`.
    pub fn with_str<T>(&self, use_it: impl FnOnce(&str) -> T) -> T {
        self.shielded.with(|buf| {
            let len = usize::from(u16::from_le_bytes([buf[0], buf[1]])).min(MAX_PASSPHRASE_BYTES);
            // Written from a `&str` in `new`, so it is UTF-8.
            use_it(std::str::from_utf8(&buf[LEN_PREFIX..LEN_PREFIX + len]).unwrap_or_default())
        })
    }

    /// A plain copy, wiped when dropped: what is handed to the CLI.
    pub fn reveal(&self) -> Zeroizing<String> {
        self.with_str(|s| Zeroizing::new(s.to_owned()))
    }
}

impl fmt::Debug for Passphrase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Passphrase(#{}, <redacted>)", self.serial)
    }
}

// ---------------------------------------------------------------------------
// Why it is asked, and who asks

/// What the user is doing that needs the passphrase, for the question's text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reason {
    /// Connecting to the session or connection with this name.
    Connect(String),
    /// Listing entries (the picker).
    List,
    /// Settings → Test connection.
    Test,
    Other,
}

impl Reason {
    pub fn code(&self) -> &'static str {
        match self {
            Reason::Connect(_) => "connect",
            Reason::List => "list",
            Reason::Test => "test",
            Reason::Other => "other",
        }
    }

    pub fn target(&self) -> Option<&str> {
        match self {
            Reason::Connect(name) => Some(name),
            _ => None,
        }
    }
}

/// One question to the user.
#[derive(Debug, Clone)]
pub struct PromptRequest {
    pub provider: ProviderId,
    pub reason: Reason,
    /// The previous passphrase was rejected; `error` says how.
    pub retry: bool,
    pub error: Option<String>,
    /// How long the answer will be kept, for the question to say.
    pub remember_secs: u32,
}

/// What the user did with the question.
pub enum Answer {
    Entered(Passphrase),
    Cancelled,
    /// There was nobody to ask.
    NoWindow,
}

/// Something that can ask the user for a passphrase. The app's window is one
/// (see `super::prompt`); tests bring their own.
pub trait PassphraseAsker: Send + Sync {
    fn ask(&self, request: PromptRequest) -> Pin<Box<dyn Future<Output = Answer> + Send + '_>>;
}

/// Why no passphrase came.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptError {
    Cancelled,
    TimedOut,
    NoWindow,
}

impl PromptError {
    /// The user-facing error, named after the manager.
    pub fn message(self, provider: ProviderId) -> String {
        let name = provider.display_name();
        match self {
            PromptError::Cancelled => format!("{name}: cancelled"),
            PromptError::TimedOut => format!("{name}: passphrase not entered"),
            PromptError::NoWindow => format!("{name}: the passphrase is needed, but there is no window to ask in"),
        }
    }
}

type Outcome = Result<Arc<Passphrase>, PromptError>;

// ---------------------------------------------------------------------------
// One question at a time

/// The open questions, one per account. Whoever needs an answer while one
/// is open waits for that one.
pub struct PromptHub {
    timeout: Duration,
    flights: Mutex<HashMap<CacheKey, watch::Receiver<Option<Outcome>>>>,
}

/// Takes a finished (or abandoned) question off the list.
struct Landing<'a> {
    hub: &'a PromptHub,
    key: CacheKey,
}

impl Drop for Landing<'_> {
    fn drop(&mut self) {
        lock(&self.hub.flights).remove(&self.key);
    }
}

impl PromptHub {
    /// `timeout`: how long a question waits for its answer.
    pub fn new(timeout: Duration) -> Self {
        Self { timeout, flights: Mutex::new(HashMap::new()) }
    }

    /// Whether a question for `key` is open now.
    pub fn is_open(&self, key: CacheKey) -> bool {
        lock(&self.flights).contains_key(&key)
    }

    /// Ask for the passphrase of `key`'s account, or wait for the question
    /// already open for it. `asker` is used only if this call is the one
    /// that asks.
    pub async fn ask(&self, key: CacheKey, asker: Option<&Arc<dyn PassphraseAsker>>, request: PromptRequest) -> Outcome {
        loop {
            let joined = {
                let mut flights = lock(&self.flights);
                match flights.get(&key) {
                    Some(rx) => Err(rx.clone()),
                    None => {
                        let (tx, rx) = watch::channel(None);
                        flights.insert(key, rx);
                        Ok(tx)
                    }
                }
            };
            match joined {
                Ok(tx) => {
                    // Off the list however this ends, a dropped connect too.
                    let _landing = Landing { hub: self, key };
                    let outcome = self.put(asker, request).await;
                    let _ = tx.send(Some(outcome.clone()));
                    return outcome;
                }
                Err(mut rx) => {
                    if let Ok(answer) = rx.wait_for(Option::is_some).await {
                        if let Some(outcome) = answer.clone() {
                            return outcome;
                        }
                    }
                    // Whoever asked went away without an answer (its connect
                    // was given up): ask anew.
                }
            }
        }
    }

    async fn put(&self, asker: Option<&Arc<dyn PassphraseAsker>>, request: PromptRequest) -> Outcome {
        let Some(asker) = asker else {
            return Err(PromptError::NoWindow);
        };
        match tokio::time::timeout(self.timeout, asker.ask(request)).await {
            Ok(Answer::Entered(p)) => Ok(Arc::new(p)),
            Ok(Answer::Cancelled) => Err(PromptError::Cancelled),
            Ok(Answer::NoWindow) => Err(PromptError::NoWindow),
            Err(_) => Err(PromptError::TimedOut),
        }
    }
}

// ---------------------------------------------------------------------------
// Kept for a while

struct Remembered {
    account: CacheKey,
    pass: Arc<Passphrase>,
    until: Instant,
}

#[derive(Default)]
struct CacheState {
    /// Bumped by every [`PassphraseCache::forget`], so an answer to a
    /// question asked before is not kept.
    generation: u64,
    entries: HashMap<ProviderId, Remembered>,
}

/// The passphrases kept between operations: at most one per manager, for
/// the account it was typed for, until a fixed deadline.
#[derive(Default)]
pub struct PassphraseCache {
    state: Mutex<CacheState>,
}

impl PassphraseCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Take it before asking; hand it to [`store`](Self::store).
    pub fn generation(&self) -> u64 {
        lock(&self.state).generation
    }

    /// The passphrase kept for `account`, if its time is not up.
    pub fn get(&self, account: CacheKey) -> Option<Arc<Passphrase>> {
        self.get_at(account, Instant::now())
    }

    pub(crate) fn get_at(&self, account: CacheKey, now: Instant) -> Option<Arc<Passphrase>> {
        let mut state = lock(&self.state);
        let provider = account.provider();
        let entry = state.entries.get(&provider)?;
        if entry.until <= now || entry.account != account {
            // Expired, or typed for another account (the settings changed):
            // of no use to anyone now.
            state.entries.remove(&provider);
            return None;
        }
        Some(entry.pass.clone())
    }

    /// Whether a passphrase is kept for `account` now.
    pub fn holds(&self, account: CacheKey) -> bool {
        self.get(account).is_some()
    }

    /// Keep `pass` for `remember`, typed for `account` after `generation`.
    /// Nothing is kept for a zero time, or if the cache was cleared since
    /// the question was asked. Returns the deadline.
    pub fn store(&self, account: CacheKey, pass: Arc<Passphrase>, generation: u64, remember: Duration) -> Option<Instant> {
        self.store_at(account, pass, generation, remember, Instant::now())
    }

    pub(crate) fn store_at(
        &self,
        account: CacheKey,
        pass: Arc<Passphrase>,
        generation: u64,
        remember: Duration,
        now: Instant,
    ) -> Option<Instant> {
        let mut state = lock(&self.state);
        if remember.is_zero() || state.generation != generation {
            return None;
        }
        // The deadline of the first store of this typing holds: another
        // caller that got the same answer does not push it out.
        if let Some(e) = state.entries.get(&account.provider()) {
            if e.pass.serial() == pass.serial() && e.account == account {
                return Some(e.until);
            }
        }
        let until = now + remember;
        state.entries.insert(account.provider(), Remembered { account, pass, until });
        Some(until)
    }

    /// Forget the passphrase with this serial, if it is still the one kept.
    pub fn forget_serial(&self, serial: u64) -> bool {
        let mut state = lock(&self.state);
        let before = state.entries.len();
        state.entries.retain(|_, e| e.pass.serial() != serial);
        state.entries.len() != before
    }

    /// The deadline timer: forget the passphrase with this serial if its
    /// time is up.
    pub(crate) fn expire_at(&self, serial: u64, now: Instant) {
        lock(&self.state).entries.retain(|_, e| e.pass.serial() != serial || e.until > now);
    }

    /// Forget everything, and keep any question still open from being kept.
    pub fn forget(&self) {
        let mut state = lock(&self.state);
        state.generation = state.generation.wrapping_add(1);
        state.entries.clear();
    }
}

// ---------------------------------------------------------------------------
// Operations

/// One thing the user asked for. See the module comment.
pub struct Operation {
    asker: Option<Arc<dyn PassphraseAsker>>,
    reason: Mutex<Reason>,
    held: Mutex<Option<(CacheKey, Arc<Passphrase>)>>,
    rejected: AtomicU32,
}

impl Operation {
    pub fn new(asker: Option<Arc<dyn PassphraseAsker>>, reason: Reason) -> Self {
        Self { asker, reason: Mutex::new(reason), held: Mutex::new(None), rejected: AtomicU32::new(0) }
    }

    fn held(&self, account: CacheKey) -> Option<Arc<Passphrase>> {
        lock(&self.held).as_ref().filter(|(k, _)| *k == account).map(|(_, p)| p.clone())
    }

    fn hold(&self, account: CacheKey, pass: Arc<Passphrase>) {
        *lock(&self.held) = Some((account, pass));
    }

    fn release(&self, serial: u64) {
        let mut held = lock(&self.held);
        if held.as_ref().is_some_and(|(_, p)| p.serial() == serial) {
            *held = None;
        }
    }
}

tokio::task_local! {
    static OPERATION: Arc<Operation>;
}

/// Run `fut` as one operation: the passphrase it needs is asked for at most
/// once (unless rejected), with `asker`, giving `reason`. Inside another
/// operation this is part of that one and changes nothing.
pub async fn operation<F: Future>(asker: Option<Arc<dyn PassphraseAsker>>, reason: Reason, fut: F) -> F::Output {
    if OPERATION.try_with(|_| ()).is_ok() {
        return fut.await;
    }
    OPERATION.scope(Arc::new(Operation::new(asker, reason)), fut).await
}

/// Say more precisely what the current operation connects to, once it is
/// known (a desktop's saved session is read after the operation began).
pub fn name_operation(name: &str) {
    let _ = OPERATION.try_with(|op| {
        let mut reason = lock(&op.reason);
        if matches!(*reason, Reason::Connect(_)) {
            *reason = Reason::Connect(name.to_string());
        }
    });
}

/// What a call needs to unlock: the account, and how long a typed
/// passphrase may be kept (seconds; 0 = only for the operation).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Policy {
    pub account: CacheKey,
    pub remember_secs: u32,
}

// ---------------------------------------------------------------------------
// Putting it together

/// The cache and the question, as one. The app has one ([`global`]); tests
/// make their own (leaked, for the deadline timer's `'static`).
pub struct Unlocker {
    pub cache: PassphraseCache,
    pub hub: PromptHub,
}

impl Unlocker {
    pub fn new(prompt_timeout: Duration) -> Self {
        Self { cache: PassphraseCache::new(), hub: PromptHub::new(prompt_timeout) }
    }

    /// Run `call`, handing it the passphrase when `policy` says one must be
    /// asked for (`None`: the call needs none, or brings its own). A
    /// rejected passphrase is forgotten and asked for again, saying why, up
    /// to [`MAX_ATTEMPTS`] per operation; then the manager's error is
    /// returned.
    pub async fn with_passphrase<T, F, Fut>(&'static self, policy: Option<Policy>, mut call: F) -> Result<T, String>
    where
        F: FnMut(Option<Arc<Passphrase>>) -> Fut,
        Fut: Future<Output = Result<T, Failure>>,
    {
        let Some(policy) = policy else {
            return call(None).await.map_err(Failure::into_message);
        };
        // Outside any operation (nothing should be), this call is its own,
        // with nobody to ask.
        let op = OPERATION.try_with(Arc::clone).unwrap_or_else(|_| Arc::new(Operation::new(None, Reason::Other)));
        let mut rejected: Option<String> = None;
        loop {
            let pass = self.obtain(&op, &policy, rejected.take()).await?;
            match call(Some(pass.clone())).await {
                Ok(value) => return Ok(value),
                Err(Failure::WrongPassphrase(message)) => {
                    op.release(pass.serial());
                    self.cache.forget_serial(pass.serial());
                    let attempts = op.rejected.fetch_add(1, Ordering::SeqCst) + 1;
                    tracing::info!(
                        "pwmanager: {} rejected the passphrase ({attempts} of {MAX_ATTEMPTS})",
                        policy.account.provider().as_str()
                    );
                    if attempts >= MAX_ATTEMPTS {
                        return Err(message);
                    }
                    rejected = Some(message);
                }
                Err(Failure::Other(message)) => return Err(message),
            }
        }
    }

    /// The passphrase for `policy`'s account: the operation's own, the
    /// remembered one, or asked for now.
    async fn obtain(&'static self, op: &Operation, policy: &Policy, rejected: Option<String>) -> Result<Arc<Passphrase>, String> {
        if let Some(p) = op.held(policy.account) {
            return Ok(p);
        }
        if let Some(p) = self.cache.get(policy.account) {
            op.hold(policy.account, p.clone());
            return Ok(p);
        }
        let generation = self.cache.generation();
        let provider = policy.account.provider();
        let remember_secs = clamp_remember(policy.remember_secs);
        let request = PromptRequest {
            provider,
            reason: lock(&op.reason).clone(),
            retry: rejected.is_some(),
            error: rejected,
            remember_secs,
        };
        let pass = self.hub.ask(policy.account, op.asker.as_ref(), request).await.map_err(|e| e.message(provider))?;
        if let Some(until) = self.cache.store(policy.account, pass.clone(), generation, Duration::from_secs(remember_secs.into())) {
            self.expire_later(pass.serial(), until);
        }
        op.hold(policy.account, pass.clone());
        Ok(pass)
    }

    /// Drop the passphrase at its deadline even if nothing asks for it (a
    /// read would drop it then anyway).
    fn expire_later(&'static self, serial: u64, until: Instant) {
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                tokio::time::sleep_until(until.into()).await;
                self.cache.expire_at(serial, Instant::now());
            });
        }
    }
}

/// The app's.
pub fn global() -> &'static Unlocker {
    static UNLOCKER: OnceLock<Unlocker> = OnceLock::new();
    UNLOCKER.get_or_init(|| Unlocker::new(PROMPT_TIMEOUT))
}

#[cfg(test)]
#[path = "unlock_tests.rs"]
mod tests;
