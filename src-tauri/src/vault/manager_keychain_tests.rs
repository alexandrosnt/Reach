use super::*;

/// Tests run against an in-memory keychain that keeps what is written,
/// never the OS one (see keychain_entry_in).
#[test]
fn the_test_keychain_keeps_what_is_written() {
    let entry = keychain_entry_in("reach-keychain-test", "user-a").unwrap();
    entry.set_password("value").unwrap();
    let again = keychain_entry_in("reach-keychain-test", "user-a").unwrap();
    assert_eq!(again.get_password().unwrap(), "value");
    again.delete_credential().unwrap();
    assert!(keychain_entry_in("reach-keychain-test", "user-a").unwrap().get_password().is_err());
}

/// A reset removes the identity's key: it must not stay behind able to
/// open what the identity encrypted.
#[tokio::test]
async fn a_reset_takes_the_identity_key_with_it() {
    let dir = std::env::temp_dir().join(format!("reach-reset-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut mgr = VaultManager::new(dir.clone());
    let test_password = format!(
        "reset-test-pass-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    mgr.init_identity(&test_password).await.unwrap();
    let uuid = mgr.user_uuid.clone().unwrap();
    assert!(get_key_from_keychain(&uuid).is_ok(), "the identity stored its key");
    mgr.reset().await.unwrap();
    assert!(get_key_from_keychain(&uuid).is_err(), "the key outlived the reset");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Issue #87: the keychain loses the key (on Linux every reboot empties the
/// kernel keyring). An identity set up without a password must still open,
/// with every secret it held, and the key must be back in the keychain.
#[tokio::test]
async fn a_lost_keychain_key_does_not_lose_the_vault() {
    use secrecy::{ExposeSecret, SecretBox};
    let dir = std::env::temp_dir().join(format!("reach-lostkey-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut mgr = VaultManager::new(dir.clone());
    mgr.init_identity("").await.unwrap();
    let uuid = mgr.user_uuid.clone().unwrap();
    let vault = mgr.internal_vault_ids.get("__sessions__").cloned().expect("sessions vault");
    let id = mgr
        .create_secret(&vault, "web", SecretCategory::Session, SecretBox::new(Box::new(b"saved session".to_vec())))
        .await
        .unwrap();

    // Reboot: the keychain forgets the key, Reach starts again.
    delete_key_from_keychain(&uuid).unwrap();
    drop(mgr);
    let mut mgr = VaultManager::new(dir.clone());
    assert!(mgr.auto_unlock().await.unwrap(), "the vault opened without its keychain key");
    let back = mgr.read_secret(&vault, &id).await.unwrap();
    assert_eq!(back.expose_secret().as_slice(), b"saved session");
    assert!(get_key_from_keychain(&uuid).is_ok(), "the key is back in the keychain");
    let _ = std::fs::remove_dir_all(&dir);
}

/// With a real password the key stays behind it: no automatic way in.
#[tokio::test]
async fn a_lost_keychain_key_with_a_password_still_asks_for_it() {
    let dir = std::env::temp_dir().join(format!("reach-lostkey-pw-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut mgr = VaultManager::new(dir.clone());
    mgr.init_identity("a real password").await.unwrap();
    let uuid = mgr.user_uuid.clone().unwrap();
    delete_key_from_keychain(&uuid).unwrap();
    drop(mgr);
    let mut mgr = VaultManager::new(dir.clone());
    assert!(matches!(mgr.auto_unlock().await, Err(VaultError::KeychainKeyMissing)));
    assert!(mgr.is_locked());
    assert!(mgr.unlock("a real password").await.unwrap());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Issue #87, second half: a backup restored into the same app folder while
/// the vault there is locked for want of its keychain key must finish, and
/// bring the sessions back.
#[tokio::test]
async fn a_backup_restores_over_a_vault_that_lost_its_key() {
    use secrecy::{ExposeSecret, SecretBox};
    let dir = std::env::temp_dir().join(format!("reach-lostkey-backup-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut mgr = VaultManager::new(dir.clone());
    mgr.init_identity("").await.unwrap();
    let uuid = mgr.user_uuid.clone().unwrap();
    let vault = mgr.internal_vault_ids.get("__sessions__").cloned().unwrap();
    let id = mgr
        .create_secret(&vault, "web", SecretCategory::Session, SecretBox::new(Box::new(b"saved session".to_vec())))
        .await
        .unwrap();
    let backup = mgr.export_full_backup("export-pass").await.unwrap();
    delete_key_from_keychain(&uuid).unwrap();
    drop(mgr);

    let mut mgr = VaultManager::new(dir.clone());
    // Only a hang must fail here. Opening the backup derives its key slowly
    // on purpose, and a busy CI runner has taken over 30 seconds.
    let restored = tokio::time::timeout(std::time::Duration::from_secs(300), mgr.import_full_backup(&backup, "export-pass", ""))
        .await
        .expect("the import hung");
    restored.unwrap();
    let mut mgr = VaultManager::new(dir.clone());
    assert!(mgr.auto_unlock().await.unwrap());
    let back = mgr.read_secret(&vault, &id).await.unwrap();
    assert_eq!(back.expose_secret().as_slice(), b"saved session");
    let _ = std::fs::remove_dir_all(&dir);
}
