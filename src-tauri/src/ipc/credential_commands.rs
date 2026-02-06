//! Credential commands that use the encrypted vault system.
//!
//! All credentials are stored encrypted in SQLite using XChaCha20-Poly1305.
//! The vault is persisted to disk and safe at rest.
//! Lookups are O(1) using session_id as the primary key.

use crate::state::AppState;
use crate::vault::types::SecretCategory;
use secrecy::SecretBox;
use tauri::State;

const CREDENTIALS_VAULT_NAME: &str = "__credentials__";

/// Set the master password for the credential vault.
///
/// Initializes the vault identity if not already done, or unlocks if it exists.
#[tauri::command]
#[tracing::instrument(skip(password, state))]
pub async fn credential_set_master_password(
    state: State<'_, AppState>,
    password: String,
) -> Result<(), String> {
    let mut manager = state.vault_manager.lock().await;

    if manager.has_identity().await {
        // Identity exists, just unlock
        manager
            .unlock(&password)
            .await
            .map_err(|e| e.to_string())?;
    } else {
        // Create new identity
        manager
            .init_identity(&password)
            .await
            .map_err(|e| e.to_string())?;
    }

    // Ensure credentials vault exists
    ensure_credentials_vault(&mut manager).await?;

    tracing::info!("Master password set, vault unlocked");
    Ok(())
}

/// Verify the master password and unlock the credential vault.
#[tauri::command]
#[tracing::instrument(skip(password, state))]
pub async fn credential_verify_master_password(
    state: State<'_, AppState>,
    password: String,
) -> Result<bool, String> {
    let mut manager = state.vault_manager.lock().await;

    let success = manager.unlock(&password).await.map_err(|e| e.to_string())?;

    if success {
        // Ensure credentials vault exists and is unlocked
        ensure_credentials_vault(&mut manager).await?;
        tracing::info!("Master password verified, vault unlocked");
    } else {
        tracing::warn!("Master password verification failed");
    }

    Ok(success)
}

/// Check whether the credential vault is locked.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn credential_is_locked(state: State<'_, AppState>) -> Result<bool, String> {
    let manager = state.vault_manager.lock().await;
    Ok(manager.is_locked())
}

/// Lock the credential vault.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn credential_lock(state: State<'_, AppState>) -> Result<(), String> {
    let mut manager = state.vault_manager.lock().await;
    manager.lock();
    tracing::info!("Credential vault locked");
    Ok(())
}

/// Check whether a master password has been configured.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn credential_has_master_password(state: State<'_, AppState>) -> Result<bool, String> {
    let manager = state.vault_manager.lock().await;
    Ok(manager.has_identity().await)
}

/// Save an encrypted password for a session. O(1) upsert.
///
/// The password is stored encrypted in SQLite, persisted to disk.
#[tauri::command]
#[tracing::instrument(skip(password, state))]
pub async fn credential_save_password(
    state: State<'_, AppState>,
    session_id: String,
    password: String,
) -> Result<(), String> {
    let manager = state.vault_manager.lock().await;

    if manager.is_locked() {
        return Err("Vault is locked. Set a master password first.".to_string());
    }

    let vault_id = get_credentials_vault_id(&manager)?;
    let password_bytes = password.into_bytes();

    // Check if exists first, then either update or create
    if manager.secret_exists(&vault_id, &session_id).await {
        // Update existing (O(1) by primary key)
        let plaintext = SecretBox::new(Box::new(password_bytes));
        manager
            .update_secret(&vault_id, &session_id, plaintext)
            .await
            .map_err(|e| e.to_string())?;
    } else {
        // Create with session_id as the secret ID (O(1) insert)
        let plaintext = SecretBox::new(Box::new(password_bytes));
        manager
            .create_secret_with_id(&vault_id, &session_id, &session_id, SecretCategory::Password, plaintext)
            .await
            .map_err(|e| e.to_string())?;
    }

    tracing::info!("Saved encrypted password for session {}", session_id);
    Ok(())
}

/// Retrieve a decrypted password for a session. O(1) lookup by primary key.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn credential_get_password(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<Option<String>, String> {
    let manager = state.vault_manager.lock().await;

    if manager.is_locked() {
        return Ok(None);
    }

    let vault_id = match get_credentials_vault_id_if_exists(&manager) {
        Some(id) => id,
        None => return Ok(None),
    };

    // O(1) lookup by primary key (session_id = secret_id)
    match manager.read_secret(&vault_id, &session_id).await {
        Ok(plaintext) => {
            use secrecy::ExposeSecret;
            let password = String::from_utf8(plaintext.expose_secret().clone())
                .map_err(|e| format!("Invalid UTF-8 in decrypted password: {}", e))?;
            Ok(Some(password))
        }
        Err(_) => Ok(None), // Not found
    }
}

/// Check if a password is saved for a given session. O(1) lookup.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn credential_has_password(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<bool, String> {
    let manager = state.vault_manager.lock().await;

    if manager.is_locked() {
        return Ok(false);
    }

    let vault_id = match get_credentials_vault_id_if_exists(&manager) {
        Some(id) => id,
        None => return Ok(false),
    };

    // O(1) check by trying to read
    Ok(manager.secret_exists(&vault_id, &session_id).await)
}

/// Delete a saved password for a session. O(1) delete by primary key.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn credential_delete_password(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<(), String> {
    let manager = state.vault_manager.lock().await;

    if manager.is_locked() {
        return Err("Vault is locked".to_string());
    }

    let vault_id = match get_credentials_vault_id_if_exists(&manager) {
        Some(id) => id,
        None => return Ok(()), // No vault, nothing to delete
    };

    // O(1) delete by primary key
    let _ = manager.delete_secret(&vault_id, &session_id).await;
    tracing::info!("Deleted saved password for session {}", session_id);

    Ok(())
}

/// Ensure the credentials vault exists and is open (O(1) lookup).
async fn ensure_credentials_vault(
    manager: &mut crate::vault::VaultManager,
) -> Result<String, String> {
    // O(1) lookup by name
    if let Some(vault_id) = manager.get_vault_id_by_name(CREDENTIALS_VAULT_NAME) {
        // Open and unlock if not already
        let _ = manager.open_vault(&vault_id, None, None).await;
        manager
            .unlock_vault(&vault_id)
            .await
            .map_err(|e| e.to_string())?;
        Ok(vault_id)
    } else {
        // Create new credentials vault (O(1) insert)
        let vault = manager
            .create_vault(CREDENTIALS_VAULT_NAME, crate::vault::types::VaultType::Private, None, None)
            .await
            .map_err(|e| e.to_string())?;
        Ok(vault.id)
    }
}

/// Get the credentials vault ID (O(1) by name).
fn get_credentials_vault_id(
    manager: &crate::vault::VaultManager,
) -> Result<String, String> {
    manager
        .get_vault_id_by_name(CREDENTIALS_VAULT_NAME)
        .ok_or_else(|| "Credentials vault not found".to_string())
}

/// Get the credentials vault ID if it exists (O(1) by name).
fn get_credentials_vault_id_if_exists(
    manager: &crate::vault::VaultManager,
) -> Option<String> {
    manager.get_vault_id_by_name(CREDENTIALS_VAULT_NAME)
}
