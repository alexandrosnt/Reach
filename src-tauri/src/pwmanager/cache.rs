//! The entry-list cache, and one listing at a time.
//!
//! A listing is a full login to the password manager (several seconds with
//! Passbolt), too slow to repeat on every keystroke in the picker. What is
//! cached is metadata only (names, usernames, URLs), never a secret, for
//! [`LIST_TTL`].
//!
//! Three rules keep it from serving the wrong thing:
//!
//! - **One listing at a time per provider.** Callers that miss the cache
//!   queue on that provider's lock and look again once they hold it, so ten
//!   searches typed during the first (slow) load cause one login, not ten.
//! - **A listing started before the cache was cleared is not stored.**
//!   [`ListCache::clear`] bumps a generation number; a fetch remembers the
//!   generation it started under and is dropped if it changed. Otherwise a
//!   listing begun with the previous account's settings, finishing after
//!   Save, would fill the cache with that account's entries.
//! - **Entries are filed under the account they came from**
//!   ([`CacheKey`]): the provider and a keyed hash of what decides whose
//!   entries come back (CLI, credentials mode, server, MFA mode, private
//!   key). Different settings never see each other's listing, even before a
//!   clear.
//!
//! The cache is also cleared when the vault is locked (see
//! `ipc::vault_commands::vault_lock`).

use std::collections::HashMap;
use std::future::Future;
use std::hash::{BuildHasher, Hash, Hasher, RandomState};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use super::config::Loaded;
use super::{EntrySummary, ProviderId};

/// How long a listing is served from the cache.
pub const LIST_TTL: Duration = Duration::from_secs(5 * 60);

/// Which account a listing belongs to. A passphrase that was asked for is
/// filed under the same key (see `super::unlock`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CacheKey {
    provider: ProviderId,
    /// A hash of the settings that decide whose entries are listed. Keyed
    /// with a random key chosen when Reach starts, kept in memory only, and
    /// never logged: it is not a fingerprint of the private key.
    account: u64,
}

impl CacheKey {
    /// The key for `provider` with the settings and secrets in `loaded`.
    pub fn new(provider: ProviderId, loaded: &Loaded) -> Self {
        static KEYS: OnceLock<RandomState> = OnceLock::new();
        let mut h = KEYS.get_or_init(RandomState::new).build_hasher();
        match provider {
            ProviderId::Passbolt => {
                let c = &loaded.config.passbolt;
                c.cli_path.as_deref().map(str::trim).hash(&mut h);
                c.credentials.hash(&mut h);
                c.server_address.trim().hash(&mut h);
                c.mfa_mode.hash(&mut h);
                loaded.passbolt_secrets.private_key.as_ref().map(|k| k.as_bytes()).hash(&mut h);
            }
        }
        Self { provider, account: h.finish() }
    }

    pub fn provider(&self) -> ProviderId {
        self.provider
    }

    #[cfg(test)]
    pub fn for_test(provider: ProviderId, account: u64) -> Self {
        Self { provider, account }
    }
}

struct Entry {
    key: CacheKey,
    at: Instant,
    list: Arc<Vec<EntrySummary>>,
}

#[derive(Default)]
struct State {
    generation: u64,
    entries: HashMap<ProviderId, Entry>,
}

/// The cache. One lives for the whole app ([`global`]); tests make their own.
#[derive(Default)]
pub struct ListCache {
    state: Mutex<State>,
    flights: Mutex<HashMap<ProviderId, Arc<tokio::sync::Mutex<()>>>>,
}

impl ListCache {
    pub fn new() -> Self {
        Self::default()
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The current generation. Take it before reading the settings a fetch
    /// will use, and hand it to [`store`](Self::store).
    pub fn generation(&self) -> u64 {
        self.state().generation
    }

    /// The cached listing for `key`, if fresh, and (with `fetched_after`)
    /// only if it was stored after that moment.
    pub fn get(&self, key: CacheKey, fetched_after: Option<Instant>) -> Option<Arc<Vec<EntrySummary>>> {
        let state = self.state();
        state
            .entries
            .get(&key.provider)
            .filter(|e| e.key == key && e.at.elapsed() < LIST_TTL)
            .filter(|e| fetched_after.is_none_or(|t| e.at >= t))
            .map(|e| e.list.clone())
    }

    /// Store a listing fetched under `generation`. Refused (false) if the
    /// cache was cleared since.
    pub fn store(&self, key: CacheKey, generation: u64, list: Arc<Vec<EntrySummary>>) -> bool {
        self.store_at(key, generation, list, Instant::now())
    }

    fn store_at(&self, key: CacheKey, generation: u64, list: Arc<Vec<EntrySummary>>, at: Instant) -> bool {
        let mut state = self.state();
        if state.generation != generation {
            return false;
        }
        state.entries.insert(key.provider, Entry { key, at, list });
        true
    }

    /// Forget every listing, and make any listing still running unstorable.
    pub fn clear(&self) {
        let mut state = self.state();
        state.generation = state.generation.wrapping_add(1);
        state.entries.clear();
    }

    fn flight(&self, provider: ProviderId) -> Arc<tokio::sync::Mutex<()>> {
        self.flights
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(provider)
            .or_default()
            .clone()
    }

    /// The listing for `key`: from the cache, or fetched with `fetch`, one
    /// fetch per provider at a time. `generation` is what
    /// [`generation`](Self::generation) said before the settings used by
    /// `fetch` were read. `refresh` skips a cached listing unless it was
    /// fetched after this call began (by whoever held the lock meanwhile).
    pub async fn get_or_fetch<F, Fut>(
        &self,
        key: CacheKey,
        generation: u64,
        refresh: bool,
        fetch: F,
    ) -> Result<Arc<Vec<EntrySummary>>, String>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<Vec<EntrySummary>, String>>,
    {
        let asked = Instant::now();
        if !refresh {
            if let Some(list) = self.get(key, None) {
                return Ok(list);
            }
        }
        let flight = self.flight(key.provider);
        let _one_at_a_time = flight.lock().await;
        // Whoever held the lock may just have fetched this.
        if let Some(list) = self.get(key, refresh.then_some(asked)) {
            return Ok(list);
        }
        let list = Arc::new(fetch().await?);
        if !self.store(key, generation, list.clone()) {
            tracing::info!("pwmanager: a listing finished after the settings changed; not cached");
        }
        Ok(list)
    }
}

/// The app's cache.
pub fn global() -> &'static ListCache {
    static CACHE: OnceLock<ListCache> = OnceLock::new();
    CACHE.get_or_init(ListCache::new)
}

#[cfg(test)]
#[path = "cache_tests.rs"]
mod tests;
