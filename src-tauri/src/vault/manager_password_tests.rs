use super::*;

fn tmp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "reach-vault-test-{}-{}",
        name,
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The cache of a synced vault, end to end against a real libsql server:
/// built by the first refresh, enough on its own to open, unlock and list
/// the vault with the server out of reach, updated when someone else
/// changes the vault, and unreadable on disk.
///
/// Needs a libsql server, so it is skipped unless one is named, e.g.
/// `docker run -d -p 58080:8080 ghcr.io/tursodatabase/libsql-server` and
/// `REACH_TEST_SQLD=http://127.0.0.1:58080`.
#[tokio::test]
#[ignore]
async fn a_synced_vault_opens_and_lists_from_its_cache() {
    let Ok(url) = std::env::var("REACH_TEST_SQLD") else { return };
    let server = libsql::Builder::new_remote(url.clone(), String::new())
        .connector(crate::vault::turso_tls::TursoConnector::new().unwrap())
        .build()
        .await
        .unwrap();
    let other_device = server.connect().unwrap();
    other_device
        .execute_batch("DROP TABLE IF EXISTS secrets; DROP TABLE IF EXISTS vault_members; DROP TABLE IF EXISTS vault_header;")
        .await
        .unwrap();

    let dir = tmp_dir("cache-e2e");
    let mut mgr = VaultManager::new(dir.clone());
    mgr.init_identity("cache-e2e-pass").await.unwrap();
    let vault = mgr.create_vault("synced", VaultType::Private, Some(&url), Some("")).await.unwrap();
    let put = |text: &str| SecretBox::new(Box::new(text.as_bytes().to_vec()));
    let keep = mgr.create_secret(&vault.id, "Production Xostme V2", SecretCategory::Session, put("one")).await.unwrap();
    let gone = mgr.create_secret(&vault.id, "FiveM BoX", SecretCategory::Session, put("two")).await.unwrap();
    mgr.create_secret(&vault.id, "Servers", SecretCategory::Folder, put("folder")).await.unwrap();

    // The first refresh makes the cache.
    let mgr = tokio::sync::Mutex::new(mgr);
    assert_eq!(refresh_caches(&mgr).await, vec![vault.id.clone()]);
    let cache_file = cache::cache_path(&dir.join("vaults"), &vault.id);
    let on_disk = std::fs::read(&cache_file).unwrap();
    for name in ["Production Xostme V2", "FiveM BoX", "Servers", "session"] {
        assert!(!on_disk.windows(name.len()).any(|w| w == name.as_bytes()), "{name} is readable on disk");
    }
    // Nothing changed since, so a second refresh reports nothing.
    assert!(refresh_caches(&mgr).await.is_empty());

    // Another run of the app, with the server out of reach: the vault
    // opens, unlocks and lists from the cache alone.
    let mut offline = VaultManager::new(dir.clone());
    offline.unlock("cache-e2e-pass").await.unwrap();
    offline.close_vault(&vault.id).await.unwrap();
    offline.open_vault(&vault.id, Some("http://127.0.0.1:9"), Some("")).await.unwrap();
    offline.unlock_vault(&vault.id).await.unwrap();
    let mut listed: Vec<Vec<u8>> = offline
        .read_secrets_in(&vault.id, &["session"])
        .await
        .unwrap()
        .into_iter()
        .map(|(_, plain)| plain.unwrap().expose_secret().clone())
        .collect();
    listed.sort();
    assert_eq!(listed, vec![b"one".to_vec(), b"two".to_vec()]);
    assert_eq!(offline.read_secret(&vault.id, &keep).await.unwrap().expose_secret(), b"one");

    // Locked, nothing of the cache stays in memory; unlocked again, it is
    // read back from the sealed file, still without the server.
    offline.lock();
    assert!(offline.vaults.values().all(|v| v.cache.is_none()));
    offline.unlock("cache-e2e-pass").await.unwrap();
    offline.unlock_vault(&vault.id).await.unwrap();
    assert_eq!(offline.read_secret(&vault.id, &keep).await.unwrap().expose_secret(), b"one");
    drop(offline);

    // Someone else deletes a session on the server; the next refresh
    // takes it in and says so.
    other_device.execute("DELETE FROM secrets WHERE id = ?", [gone.as_str()]).await.unwrap();
    assert_eq!(refresh_caches(&mgr).await, vec![vault.id.clone()]);
    let names: Vec<String> = mgr
        .lock()
        .await
        .read_secrets_in(&vault.id, &["session"])
        .await
        .unwrap()
        .into_iter()
        .map(|(meta, _)| meta.name)
        .collect();
    assert_eq!(names, ["Production Xostme V2"]);
}

/// With a device unlock method on, the vault opens by that method's key
/// or the password, never silently from the keychain; the last method
/// removed puts the keychain copy back.
#[tokio::test]
async fn a_device_guarded_vault_opens_only_with_its_own_key() {
    let dir = tmp_dir("device-unlock");
    let mut mgr = VaultManager::new(dir.clone());
    mgr.init_identity("device-vault-pass").await.unwrap();
    let (mut unlockers, secret) = mgr.unlock_enrolment().await.unwrap();
    let yubikey = unlockers.seal_for_test(biometric::FIDO2, &[1u8; 32], &*secret);
    let backup = unlockers.seal_for_test(biometric::FIDO2, &[2u8; 32], &*secret);
    let (yubikey_id, backup_id) = (yubikey.id.clone(), backup.id.clone());
    unlockers.add(yubikey);
    unlockers.add(backup);
    mgr.save_unlockers(&unlockers).unwrap();
    mgr.lock();

    // A fresh start with a method on is held, and the keychain alone
    // (auto-unlock, the plain unlock button) does not open it.
    assert!(mgr.is_held());
    assert!(!mgr.auto_unlock().await.unwrap());
    assert!(!mgr.resume().await.unwrap_or(false));
    assert!(mgr.is_locked());

    // Another key never opens it.
    assert!(mgr.unlock_with_device(&[9u8; 32]).await.is_err());
    assert!(mgr.is_locked());

    // What a seal releases does.
    let released = mgr.unlockers().unwrap().open_for_test(&[2u8; 32], &backup_id).unwrap();
    assert!(mgr.unlock_with_device(&released).await.unwrap());
    assert!(!mgr.is_locked() && !mgr.is_held());

    // A password unlock still works, and does not put the key back in the
    // keychain behind the method's back.
    mgr.lock();
    assert!(mgr.unlock("device-vault-pass").await.unwrap());

    // A file left from another identity does not count.
    biometric::save(&dir, &biometric::Unlockers::new("someone-else")).unwrap();
    assert!(mgr.unlockers().is_none());
    mgr.save_unlockers(&unlockers).unwrap();

    // Removing one key keeps the other; the last one needs the keychain.
    assert_eq!(mgr.remove_unlocker(&yubikey_id).unwrap().map(|s| s.kind).as_deref(), Some(biometric::FIDO2));
    assert!(mgr.unlockers().is_some());
    if mgr.remove_unlocker(&backup_id).is_ok() {
        assert!(mgr.unlockers().is_none());
        let mut restarted = VaultManager::new(dir.clone());
        assert!(restarted.auto_unlock().await.unwrap());
    } else {
        eprintln!("skipped the last-method check: this machine's keychain cannot be written");
        assert!(mgr.unlockers().is_some());
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Locking and unlocking again leaves every vault readable, not just open:
/// the internal ones and the user's own (issue: a vault of servers showed up
/// empty after a lock). The lock wipes their keys; the unlock must restore
/// them, by password and by a key a device method releases.
#[tokio::test]
async fn every_vault_can_be_read_after_a_lock_and_unlock() {
    let dir = tmp_dir("relock");
    let mut mgr = VaultManager::new(dir.clone());
    mgr.init_identity("relock-vault-pass").await.unwrap();
    let own = mgr.create_vault("My servers", VaultType::Private, None, None).await.unwrap().id;
    let readable = |mgr: &VaultManager| {
        let internal = INTERNAL_VAULTS.iter().all(|name| {
            let id = mgr.vault_names.get(*name).expect("internal vault mapped");
            mgr.vaults.get(id).is_some_and(|v| v.master_dek.is_some())
        });
        internal && mgr.vaults.get(&own).is_some_and(|v| v.master_dek.is_some())
    };
    assert!(readable(&mgr));

    mgr.hold();
    assert!(!readable(&mgr));
    assert!(mgr.unlock("relock-vault-pass").await.unwrap());
    assert!(readable(&mgr), "a vault stayed unreadable after unlocking");

    // And the key released by a device unlock method does the same.
    let (_, secret) = mgr.unlock_enrolment().await.unwrap();
    mgr.hold();
    assert!(mgr.unlock_with_secret_key(&*secret).await.unwrap());
    assert!(readable(&mgr));
    let _ = std::fs::remove_dir_all(&dir);
}

/// A lock by the user holds: the silent keychain unlock is refused
/// until the user opens it again, by the unlock button or by password.
#[tokio::test]
async fn a_held_vault_does_not_open_by_itself() {
    let dir = tmp_dir("held");
    let mut mgr = VaultManager::new(dir.clone());
    mgr.init_identity("held-vault-pass").await.unwrap();

    mgr.hold();
    assert!(mgr.is_locked() && mgr.is_held());
    // What the interface's routine check does: refused while held.
    assert!(!mgr.auto_unlock().await.unwrap());
    assert!(mgr.is_locked());

    // With a master password set, the one-click unlock button is refused:
    // the lock would otherwise protect nothing.
    assert!(!mgr.resume().await.unwrap());
    assert!(mgr.is_locked() && mgr.is_held());

    // Opened by password.
    assert!(mgr.unlock("held-vault-pass").await.unwrap());
    assert!(!mgr.is_locked() && !mgr.is_held());

    // A plain lock (not the user's) is not held.
    mgr.lock();
    assert!(!mgr.is_held());
}

/// A list is read in one query, and it is the same list, with the same
/// plaintexts, that one `read_secret` per item gives.
#[tokio::test]
async fn a_list_of_one_category_is_read_in_one_go() {
    let dir = tmp_dir("batch-read");
    let mut mgr = VaultManager::new(dir.clone());
    mgr.init_identity("batch-read-pass").await.unwrap();
    let vault = mgr.create_vault("batch", VaultType::Private, None, None).await.unwrap();

    let put = |text: &str| SecretBox::new(Box::new(text.as_bytes().to_vec()));
    let a = mgr.create_secret(&vault.id, "a", SecretCategory::Session, put("session a")).await.unwrap();
    let b = mgr.create_secret(&vault.id, "b", SecretCategory::Session, put("session b")).await.unwrap();
    mgr.create_secret(&vault.id, "f", SecretCategory::Folder, put("a folder")).await.unwrap();
    mgr.create_secret(&vault.id, "s", SecretCategory::Custom("snippet".into()), put("a snippet")).await.unwrap();

    let mut sessions: Vec<(String, Vec<u8>)> = mgr
        .read_secrets_in(&vault.id, &["session"])
        .await
        .unwrap()
        .into_iter()
        .map(|(meta, plain)| (meta.id, plain.unwrap().expose_secret().clone()))
        .collect();
    sessions.sort();
    let mut expected = vec![
        (a.clone(), mgr.read_secret(&vault.id, &a).await.unwrap().expose_secret().clone()),
        (b.clone(), mgr.read_secret(&vault.id, &b).await.unwrap().expose_secret().clone()),
    ];
    expected.sort();
    assert_eq!(sessions, expected);

    let mut names: Vec<String> = mgr
        .read_secrets_in(&vault.id, &["folder", "custom:snippet"])
        .await
        .unwrap()
        .into_iter()
        .map(|(meta, _)| meta.name)
        .collect();
    names.sort();
    assert_eq!(names, ["f", "s"]);

    assert!(mgr.read_secrets_in(&vault.id, &[]).await.unwrap().is_empty());
    assert!(mgr.read_secrets_in(&vault.id, &["no-such-category"]).await.unwrap().is_empty());
}

/// Regression test for issue #25: the password-encrypted secret key was
/// computed at identity init but discarded, so password unlock never
/// worked after a restart — only OS-keychain auto-unlock did.
#[tokio::test]
async fn password_roundtrip_and_change_issue_25() {
    let dir = tmp_dir("roundtrip");

    // Init identity with a password.
    let mut mgr = VaultManager::new(dir.clone());
    mgr.init_identity("hunter2hunter2").await.unwrap();
    mgr.lock();

    // Fresh manager (simulated restart): unlock with the init password.
    let mut mgr2 = VaultManager::new(dir.clone());
    assert!(
        mgr2.unlock("hunter2hunter2").await.unwrap(),
        "unlock with the init password must succeed (issue #25)"
    );

    // Wrong password must not unlock.
    mgr2.lock();
    let mut mgr3 = VaultManager::new(dir.clone());
    let bad = mgr3.unlock("wrong-password").await;
    assert!(bad.is_err() || matches!(bad, Ok(false)));

    // Change password while unlocked, then the new one works.
    let mut mgr4 = VaultManager::new(dir.clone());
    mgr4.unlock("hunter2hunter2").await.unwrap();
    mgr4.change_password("second-password").await.unwrap();
    mgr4.lock();

    let mut mgr5 = VaultManager::new(dir.clone());
    assert!(mgr5.unlock("second-password").await.unwrap());
    mgr5.lock();

    // The old password no longer works.
    let mut mgr6 = VaultManager::new(dir.clone());
    let old = mgr6.unlock("hunter2hunter2").await;
    assert!(old.is_err() || matches!(old, Ok(false)));

    let _ = std::fs::remove_dir_all(&dir);
}

/// Issue #30: an identity created before the issue-25 fix has no
/// password-encrypted key, so no password can open it — but has_identity()
/// is true, and the UI used that to report "password set". The user was
/// told they had a recovery path that did not exist.
#[tokio::test]
async fn has_password_is_false_when_no_password_material_was_persisted() {
    let dir = tmp_dir("legacy-identity");

    let mut mgr = VaultManager::new(dir.clone());
    mgr.init_identity("a-real-password").await.unwrap();
    assert!(mgr.has_identity().await);
    assert!(
        mgr.has_password().await,
        "a freshly created identity does have password material"
    );

    // Reproduce a pre-fix identity: the file exists, but the
    // password-encrypted key was discarded rather than written.
    let path = dir.join("vault_identity.json");
    let data = std::fs::read_to_string(&path).unwrap();
    let mut stored: StoredIdentity = serde_json::from_str(&data).unwrap();
    stored.encrypted_key = String::new();
    stored.nonce = String::new();
    std::fs::write(&path, serde_json::to_string(&stored).unwrap()).unwrap();

    let mgr2 = VaultManager::new(dir.clone());
    assert!(
        mgr2.has_identity().await,
        "the identity file is still there, which is why has_identity() misled the UI"
    );
    assert!(
        !mgr2.has_password().await,
        "no password can open this vault, so has_password() must say so"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// Set up without a password (the welcome screen's default), the key is
/// still sealed, with an empty password. That is no password: has_password()
/// said there was one, so a lock asked for a password nobody had set, its
/// field would not take an empty one, and the one-click unlock was refused.
#[tokio::test]
async fn an_identity_made_without_a_password_has_none() {
    let dir = tmp_dir("no-password");

    let mut mgr = VaultManager::new(dir.clone());
    mgr.init_identity("").await.unwrap();
    assert!(!mgr.has_password().await, "an empty password is not a password");

    // So a lock is undone with one click, as for any vault without a password.
    mgr.hold();
    assert!(mgr.resume().await.unwrap(), "the unlock button opens it again");

    // Once one is set, it counts, and a lock asks for it.
    mgr.change_password("a-real-password").await.unwrap();
    assert!(mgr.has_password().await);

    let _ = std::fs::remove_dir_all(&dir);
}

/// Changing the password requires an unlocked manager.
#[tokio::test]
async fn change_password_requires_unlock() {
    let dir = tmp_dir("locked-change");
    let mut mgr = VaultManager::new(dir.clone());
    mgr.init_identity("initial-pass-123").await.unwrap();
    mgr.lock();

    let err = mgr
        .change_password("whatever-else")
        .await
        .expect_err("change_password while locked must fail");
    assert!(matches!(err, VaultError::Locked));

    let _ = std::fs::remove_dir_all(&dir);
}
