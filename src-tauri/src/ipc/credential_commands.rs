//! The master password: set it, verify it, lock, and whether one is set.
//!
//! The per-session password commands that were here went unused and stored a
//! password under its session's id, which with personal sync is the session's
//! own row in the one shared vault: saving would have overwritten the session.

use crate::state::AppState;
use tauri::State;

const CREDENTIALS_VAULT_NAME: &str = "__credentials__";

/// Set the master password for the credential vault.
///
/// Initializes the vault identity if none exists. When an identity already
/// exists this CHANGES its password (requires the vault to be unlocked,
/// e.g. via OS keychain auto-unlock) — previously it attempted an unlock
/// with the new password, which always failed (issue #25).
#[tauri::command]
#[tracing::instrument(skip(password, state))]
pub async fn credential_set_master_password(
    state: State<'_, AppState>,
    password: String,
) -> Result<(), String> {
    let mut manager = state.vault_manager.lock().await;

    if manager.has_identity().await {
        if manager.is_locked() {
            return Err(
                "Vault is locked. Unlock it with your current method first, then set a new password."
                    .into(),
            );
        }
        manager
            .change_password(&password)
            .await
            .map_err(|e| e.to_string())?;
    } else {
        // Create new identity. Entry lists cached for the old one go, and
        // a passphrase typed for it (with any question still open).
        crate::pwmanager::forget_all();
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
    // The user's act: held, so the next routine check does not reopen it.
    manager.hold();
    // As vault_lock: the password-manager listings and any passphrase kept
    // in memory go with it, and any passphrase question still open.
    crate::pwmanager::forget_all();
    tracing::info!("Credential vault locked");
    Ok(())
}

/// Check whether a master password has been configured.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn credential_has_master_password(state: State<'_, AppState>) -> Result<bool, String> {
    let manager = state.vault_manager.lock().await;
    // Deliberately not has_identity(): that only checks the identity file
    // exists. An identity created before the issue-25 fix has no
    // password-encrypted key at all, and reporting "password set" for it told
    // the user they had a recovery path they did not have (issue #30).
    Ok(manager.has_password().await)
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
