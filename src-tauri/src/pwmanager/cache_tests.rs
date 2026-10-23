use std::sync::atomic::{AtomicUsize, Ordering};

use zeroize::Zeroizing;

use super::*;
use crate::pwmanager::config::{CredentialsMode, PassboltConfig, PassboltSecrets, PasswordManagersConfig};

fn entry(i: usize) -> EntrySummary {
    EntrySummary {
        id: format!("00000000-0000-4000-8000-{i:012}"),
        name: format!("entry {i}"),
        username: None,
        uri: None,
        folder_id: None,
    }
}

fn list(n: usize) -> Arc<Vec<EntrySummary>> {
    Arc::new((0..n).map(entry).collect())
}

fn key(account: u64) -> CacheKey {
    CacheKey::for_test(ProviderId::Passbolt, account)
}

#[test]
fn a_listing_is_served_for_its_own_account_only() {
    let cache = ListCache::new();
    let g = cache.generation();
    assert!(cache.store(key(1), g, list(3)));
    assert_eq!(cache.get(key(1), None).unwrap().len(), 3);
    assert!(cache.get(key(2), None).is_none(), "another account's settings");
}

#[test]
fn the_cache_expires_and_clears() {
    let cache = ListCache::new();
    let g = cache.generation();
    if let Some(old) = Instant::now().checked_sub(LIST_TTL + Duration::from_secs(1)) {
        assert!(cache.store_at(key(1), g, list(1), old));
        assert!(cache.get(key(1), None).is_none(), "expired");
    }
    assert!(cache.store(key(1), g, list(1)));
    assert!(cache.get(key(1), None).is_some());
    cache.clear();
    assert!(cache.get(key(1), None).is_none());
}

#[test]
fn a_listing_started_before_a_clear_is_not_stored() {
    let cache = ListCache::new();
    let before = cache.generation();
    cache.clear(); // Settings saved while the listing ran.
    assert!(!cache.store(key(1), before, list(5)));
    assert!(cache.get(key(1), None).is_none());
    assert!(cache.store(key(1), cache.generation(), list(5)));
}

#[test]
fn only_a_listing_fetched_after_a_refresh_began_answers_it() {
    let cache = ListCache::new();
    let at = Instant::now();
    assert!(cache.store_at(key(1), cache.generation(), list(1), at));
    assert!(cache.get(key(1), Some(at + Duration::from_millis(1))).is_none(), "fetched before the refresh");
    assert!(cache.get(key(1), Some(at)).is_some(), "fetched after it");
}

#[tokio::test]
async fn concurrent_misses_fetch_once() {
    let cache = Arc::new(ListCache::new());
    let fetches = Arc::new(AtomicUsize::new(0));
    let mut tasks = Vec::new();
    for _ in 0..8 {
        let cache = cache.clone();
        let fetches = fetches.clone();
        tasks.push(tokio::spawn(async move {
            let g = cache.generation();
            cache
                .get_or_fetch(key(7), g, false, || async {
                    fetches.fetch_add(1, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    Ok((0..4).map(entry).collect())
                })
                .await
                .map(|l| l.len())
        }));
    }
    for t in tasks {
        assert_eq!(t.await.unwrap().unwrap(), 4);
    }
    assert_eq!(fetches.load(Ordering::SeqCst), 1, "one login for every waiting caller");

    // A refresh fetches again; a cached listing is served without a fetch.
    let g = cache.generation();
    let n = cache
        .get_or_fetch(key(7), g, true, || async {
            fetches.fetch_add(1, Ordering::SeqCst);
            Ok(vec![entry(1)])
        })
        .await
        .unwrap();
    assert_eq!((n.len(), fetches.load(Ordering::SeqCst)), (1, 2));
    let n = cache
        .get_or_fetch(key(7), g, false, || async {
            fetches.fetch_add(1, Ordering::SeqCst);
            Ok(vec![])
        })
        .await
        .unwrap();
    assert_eq!((n.len(), fetches.load(Ordering::SeqCst)), (1, 2));
}

#[tokio::test]
async fn a_failed_fetch_is_not_cached_and_the_next_caller_tries_again() {
    let cache = ListCache::new();
    let g = cache.generation();
    let e = cache.get_or_fetch(key(3), g, false, || async { Err("Passbolt: down".to_string()) }).await;
    assert_eq!(e.unwrap_err(), "Passbolt: down");
    let ok = cache.get_or_fetch(key(3), g, false, || async { Ok(vec![entry(1)]) }).await.unwrap();
    assert_eq!(ok.len(), 1);
}

#[tokio::test]
async fn a_fetch_that_outlives_a_clear_is_returned_but_not_cached() {
    let cache = ListCache::new();
    let g = cache.generation();
    let got = cache
        .get_or_fetch(key(4), g, false, || async {
            cache.clear();
            Ok(vec![entry(1)])
        })
        .await
        .unwrap();
    assert_eq!(got.len(), 1);
    assert!(cache.get(key(4), None).is_none());
}

#[test]
fn the_key_follows_the_account_settings() {
    let loaded = |server: &str, key: Option<&str>, cli: Option<&str>| Loaded {
        config: PasswordManagersConfig {
            enabled: true,
            passbolt: PassboltConfig {
                cli_path: cli.map(Into::into),
                server_address: server.into(),
                ..Default::default()
            },
        },
        passbolt_secrets: PassboltSecrets {
            private_key: key.map(|k| Zeroizing::new(k.into())),
            ..Default::default()
        },
    };
    let k = |l: &Loaded| CacheKey::new(ProviderId::Passbolt, l);
    let base = loaded("https://a.example.com", Some("KEY A"), None);
    assert_eq!(k(&base), k(&base.clone()), "stable within a run");
    assert_ne!(k(&base), k(&loaded("https://b.example.com", Some("KEY A"), None)), "server");
    assert_ne!(k(&base), k(&loaded("https://a.example.com", Some("KEY B"), None)), "private key");
    assert_ne!(k(&base), k(&loaded("https://a.example.com", Some("KEY A"), Some("/usr/bin/passbolt"))), "CLI");
    let mut cli_config = base.clone();
    cli_config.config.passbolt.credentials = CredentialsMode::CliConfig;
    assert_ne!(k(&base), k(&cli_config), "credentials mode");
    // Whether it is switched on is not part of whose entries they are.
    let mut off = base.clone();
    off.config.enabled = false;
    assert_eq!(k(&base), k(&off));
}
