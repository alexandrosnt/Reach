use super::*;

fn tmp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("reach-copied-ids-test-{}-{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn a_copied_id_is_the_id_after_its_vault() {
    assert_eq!(copied_id("__sessions___0b5e-41"), Some("0b5e-41"));
    assert_eq!(copied_id("__folders___f1"), Some("f1"));
    assert_eq!(copied_id("__settings___app_settings"), Some("app_settings"));
    assert_eq!(copied_id("__ssh_keys___k"), Some("k"));
    // An id of its own, not a copy.
    assert_eq!(copied_id("0b5e-41"), None);
    assert_eq!(copied_id("__sessions__"), None);
    assert_eq!(copied_id("__sessions___"), None);
    assert_eq!(copied_id("__personal___x"), None);
}

async fn put(mgr: &VaultManager, vault: &str, id: &str, category: SecretCategory, bytes: &[u8]) {
    mgr.create_secret_with_id(vault, id, id, category, SecretBox::new(Box::new(bytes.to_vec())))
        .await
        .unwrap();
}

/// A session copied in as `__sessions___<id>` by personal sync is stored
/// under its own id again, unchanged, so it can be read, updated and
/// deleted by that id (#148). An id already in use keeps the secret that
/// uses it, and the copy is left alone.
#[tokio::test]
async fn copied_secrets_get_their_own_ids_back() {
    let dir = tmp_dir("restore");
    let mut mgr = VaultManager::new(dir.clone());
    mgr.init_identity("copied-ids-pass").await.unwrap();
    // The vault the copies went into: with personal sync every internal
    // name maps to it.
    let vault = mgr.get_vault_id_by_name(SETTINGS_VAULT).expect("the identity makes the settings vault");

    put(&mgr, &vault, "__sessions___s1", SecretCategory::Session, br#"{"id":"s1"}"#).await;
    put(&mgr, &vault, "__folders___f1", SecretCategory::Custom("folder".into()), b"folder").await;
    put(&mgr, &vault, "taken", SecretCategory::Custom("settings".into()), b"newer").await;
    put(&mgr, &vault, "__settings___taken", SecretCategory::Custom("settings".into()), b"older").await;

    mgr.restore_copied_ids().await;

    assert_eq!(mgr.read_secret(&vault, "s1").await.unwrap().expose_secret(), br#"{"id":"s1"}"#);
    assert!(!mgr.secret_exists(&vault, "__sessions___s1").await, "the copy is gone");
    let s1 = mgr.list_secrets(&vault).await.unwrap().into_iter().find(|m| m.id == "s1").unwrap();
    assert_eq!(s1.category, "session", "it is still a session");
    assert_eq!(mgr.read_secret(&vault, "f1").await.unwrap().expose_secret(), b"folder");
    assert_eq!(mgr.read_secret(&vault, "taken").await.unwrap().expose_secret(), b"newer");
    assert_eq!(mgr.read_secret(&vault, "__settings___taken").await.unwrap().expose_secret(), b"older");

    // Now found by its id, so delete removes it.
    mgr.delete_secret(&vault, "s1").await.unwrap();
    assert!(!mgr.secret_exists(&vault, "s1").await);

    // Running again changes nothing.
    mgr.restore_copied_ids().await;
    assert_eq!(mgr.read_secret(&vault, "taken").await.unwrap().expose_secret(), b"newer");
    assert!(mgr.secret_exists(&vault, "__settings___taken").await);

    let uuid = mgr.user_uuid.clone().unwrap();
    let _ = delete_key_from_keychain(&uuid);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn find_secret_says_when_a_vault_is_not_open() {
    let mgr = VaultManager::new(tmp_dir("missing"));
    assert!(mgr.find_secret("nope", "x").await.is_err());
    assert!(!mgr.secret_exists("nope", "x").await);
}
