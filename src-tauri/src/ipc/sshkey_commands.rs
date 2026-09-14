//! Import, list and remove private keys held in the vault.
//!
//! Asked for in issue #46: someone with several machines should not have to
//! copy `~/.ssh` onto each of them. A key imported here is a secret in the
//! `__ssh_keys__` vault — encrypted at rest, and carried to every machine by
//! the same personal sync that already carries sessions.
//!
//! `ssh_key_list` deliberately returns no key material. The only path that
//! hands out a private key is [`resolve`], which is called by `ssh_connect`
//! while building a handshake and never reaches the frontend.

use crate::ssh::keystore::{self, StoredKeyInfo, StoredKeyMaterial};
use crate::state::AppState;
use crate::vault::manager::SSH_KEYS_VAULT;
use crate::vault::types::SecretCategory;
use secrecy::{ExposeSecret, SecretBox};
use tauri::State;
use uuid::Uuid;

/// Seconds since the epoch, the same clock the vault stamps rows with.
fn now_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Make sure the keys vault exists and is open. Mirrors the credentials vault:
/// internal vaults are created on first use, not at startup.
async fn ensure_vault(manager: &mut crate::vault::VaultManager) -> Result<String, String> {
    if let Some(vault_id) = manager.get_vault_id_by_name(SSH_KEYS_VAULT) {
        let _ = manager.open_vault(&vault_id, None, None).await;
        manager
            .unlock_vault(&vault_id)
            .await
            .map_err(|e| e.to_string())?;
        return Ok(vault_id);
    }
    let vault = manager
        .create_vault(SSH_KEYS_VAULT, crate::vault::types::VaultType::Private, None, None)
        .await
        .map_err(|e| e.to_string())?;
    Ok(vault.id)
}

async fn read_material(
    manager: &crate::vault::VaultManager,
    vault_id: &str,
    id: &str,
) -> Result<StoredKeyMaterial, String> {
    let plaintext = manager
        .read_secret(vault_id, id)
        .await
        .map_err(|_| "That key is not in the vault.".to_string())?;
    serde_json::from_slice(plaintext.expose_secret())
        .map_err(|e| format!("The stored key could not be read: {}", e))
}

/// Import a private key, from pasted text or from a file on this machine.
///
/// A file is read here rather than in the frontend, so key material never
/// travels through the UI layer at all — the only direction it moves is disk
/// to vault. The material is checked before it is saved, so a paste that is
/// actually a public key, or a passphrase that does not open the key, fails
/// here rather than at connect time on a server three hops away.
#[tauri::command]
#[tracing::instrument(skip(private_key, passphrase, state))]
pub async fn ssh_key_import(
    state: State<'_, AppState>,
    name: String,
    private_key: Option<String>,
    path: Option<String>,
    passphrase: Option<String>,
    public_key: Option<String>,
) -> Result<StoredKeyInfo, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("Give the key a name so you can recognise it later.".into());
    }

    // A file wins when both are given: the user picked it most recently.
    let (private_key, public_key) = match path
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
    {
        Some(path) => {
            let expanded = crate::ssh::client::expand_tilde(&path);
            let material = std::fs::read_to_string(&expanded)
                .map_err(|e| format!("Could not read {}: {}", expanded.display(), e))?;
            // The matching .pub sitting beside it, if the user kept one.
            let sibling = std::fs::read_to_string(expanded.with_extension("pub"))
                .ok()
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty());
            (material, public_key.or(sibling))
        }
        None => (
            private_key.ok_or("Paste a private key, or choose a key file.")?,
            public_key,
        ),
    };

    let private_key = private_key.trim().to_string();
    let facts = keystore::inspect(&private_key)?;
    let passphrase = passphrase.filter(|p| !p.is_empty());

    // An encrypted key with a saved passphrase must actually open with it.
    // An unencrypted key is fine either way — see keystore::verify_passphrase.
    if facts.encrypted {
        if let Some(pass) = passphrase.as_deref() {
            keystore::verify_passphrase(&private_key, Some(pass))?;
        }
    } else {
        keystore::verify_passphrase(&private_key, None)?;
    }

    let mut manager = state.vault_manager.lock().await;
    if manager.is_locked() {
        return Err("Vault is locked. Set a master password first.".into());
    }
    let vault_id = ensure_vault(&mut manager).await?;

    let id = Uuid::new_v4().to_string();
    let material = StoredKeyMaterial {
        private_key,
        passphrase,
        public_key: public_key
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty())
            .or(facts.public_key),
    };
    let bytes = serde_json::to_vec(&material).map_err(|e| e.to_string())?;
    manager
        .create_secret_with_id(
            &vault_id,
            &id,
            &name,
            SecretCategory::SshKey,
            SecretBox::new(Box::new(bytes)),
        )
        .await
        .map_err(|e| e.to_string())?;

    tracing::info!("Imported SSH key '{}' ({})", name, id);
    let created_at = now_timestamp();
    Ok(keystore::describe(&id, &name, created_at, &material))
}

/// Every imported key, with no key material in the answer.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn ssh_key_list(state: State<'_, AppState>) -> Result<Vec<StoredKeyInfo>, String> {
    let manager = state.vault_manager.lock().await;
    if manager.is_locked() {
        return Ok(Vec::new());
    }
    let Some(vault_id) = manager.get_vault_id_by_name(SSH_KEYS_VAULT) else {
        return Ok(Vec::new());
    };

    let secrets = manager
        .list_secrets(&vault_id)
        .await
        .map_err(|e| e.to_string())?;

    let mut keys = Vec::new();
    for meta in secrets {
        // The keys vault shares storage with the other internal vaults when
        // personal sync is on, so filter by category rather than assuming
        // everything here is a key.
        if meta.category != SecretCategory::SshKey.to_string() {
            continue;
        }
        match read_material(&manager, &vault_id, &meta.id).await {
            Ok(material) => {
                keys.push(keystore::describe(&meta.id, &meta.name, meta.created_at, &material))
            }
            Err(e) => tracing::warn!("Skipping unreadable key {}: {}", meta.id, e),
        }
    }
    keys.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(keys)
}

/// Rename a key, or save/replace/clear the passphrase stored with it.
#[tauri::command]
#[tracing::instrument(skip(passphrase, state))]
pub async fn ssh_key_update(
    state: State<'_, AppState>,
    id: String,
    name: Option<String>,
    passphrase: Option<String>,
    clear_passphrase: Option<bool>,
) -> Result<StoredKeyInfo, String> {
    let mut manager = state.vault_manager.lock().await;
    if manager.is_locked() {
        return Err("Vault is locked.".into());
    }
    let vault_id = ensure_vault(&mut manager).await?;
    let mut material = read_material(&manager, &vault_id, &id).await?;

    if clear_passphrase.unwrap_or(false) {
        material.passphrase = None;
    } else if let Some(pass) = passphrase.filter(|p| !p.is_empty()) {
        keystore::verify_passphrase(&material.private_key, Some(&pass))?;
        material.passphrase = Some(pass);
    }

    let bytes = serde_json::to_vec(&material).map_err(|e| e.to_string())?;
    manager
        .update_secret(&vault_id, &id, SecretBox::new(Box::new(bytes)))
        .await
        .map_err(|e| e.to_string())?;

    // The name lives on the secret row, not in the payload.
    let name = match name.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()) {
        Some(n) => {
            manager
                .rename_secret(&vault_id, &id, &n)
                .await
                .map_err(|e| e.to_string())?;
            n
        }
        None => manager
            .list_secrets(&vault_id)
            .await
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|m| m.id == id)
            .map(|m| m.name)
            .unwrap_or_default(),
    };

    Ok(keystore::describe(&id, &name, now_timestamp(), &material))
}

/// Forget a key. Sessions that referenced it fall back to asking, rather than
/// silently connecting as someone else.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn ssh_key_delete(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let manager = state.vault_manager.lock().await;
    if manager.is_locked() {
        return Err("Vault is locked.".into());
    }
    let Some(vault_id) = manager.get_vault_id_by_name(SSH_KEYS_VAULT) else {
        return Ok(());
    };
    let _ = manager.delete_secret(&vault_id, &id).await;
    tracing::info!("Deleted imported SSH key {}", id);
    Ok(())
}

/// The public half of an imported key, for pasting into `authorized_keys`.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn ssh_key_public(
    state: State<'_, AppState>,
    id: String,
) -> Result<Option<String>, String> {
    let manager = state.vault_manager.lock().await;
    if manager.is_locked() {
        return Err("Vault is locked.".into());
    }
    let Some(vault_id) = manager.get_vault_id_by_name(SSH_KEYS_VAULT) else {
        return Ok(None);
    };
    let material = read_material(&manager, &vault_id, &id).await?;
    Ok(material
        .public_key
        .or_else(|| keystore::inspect(&material.private_key).ok().and_then(|f| f.public_key)))
}

/// Key material for a handshake. Called by `ssh_connect`, never exposed as a
/// command — the frontend has no way to ask for a private key back.
pub(crate) async fn resolve(
    state: &AppState,
    id: &str,
) -> Result<keystore::ResolvedKey, String> {
    let manager = state.vault_manager.lock().await;
    if manager.is_locked() {
        return Err("Vault is locked — unlock it to use an imported key.".into());
    }
    let vault_id = manager
        .get_vault_id_by_name(SSH_KEYS_VAULT)
        .ok_or_else(|| "No keys have been imported on this machine yet.".to_string())?;
    let material = read_material(&manager, &vault_id, id).await?;
    Ok((&material).into())
}
