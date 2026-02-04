use crate::credentials::master::{derive_key, generate_salt, hash_master_password, verify_master};
use crate::credentials::vault;
use crate::state::AppState;
use tauri::Manager;
use zeroize::Zeroize;

/// Set the master password for the credential vault.
///
/// Generates a salt, derives an encryption key, hashes the password for future
/// verification, and persists the hash and salt to the app data directory.
#[tauri::command]
pub async fn credential_set_master_password(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    password: String,
) -> Result<(), String> {
    let salt = generate_salt();

    let key = derive_key(&password, &salt).map_err(|e| e.to_string())?;

    let hash = hash_master_password(&password).map_err(|e| e.to_string())?;

    // Store the derived key in application state
    {
        let mut master_key = state.master_key.write().await;
        if let Some(ref mut old_key) = *master_key {
            old_key.zeroize();
        }
        *master_key = Some(key);
    }

    // Persist hash and salt to disk
    let app_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;
    std::fs::create_dir_all(&app_dir)
        .map_err(|e| format!("Failed to create app data dir: {}", e))?;

    let hash_path = app_dir.join("master.hash");
    let salt_path = app_dir.join("master.salt");

    std::fs::write(&hash_path, hash.as_bytes())
        .map_err(|e| format!("Failed to write master hash: {}", e))?;
    std::fs::write(&salt_path, &salt)
        .map_err(|e| format!("Failed to write master salt: {}", e))?;

    tracing::info!("Master password set successfully");
    Ok(())
}

/// Verify the master password and unlock the credential vault.
///
/// Loads the stored hash and salt, verifies the password, and on success
/// derives and stores the encryption key in application state.
#[tauri::command]
pub async fn credential_verify_master_password(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    password: String,
) -> Result<bool, String> {
    let app_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;

    let hash_path = app_dir.join("master.hash");
    let salt_path = app_dir.join("master.salt");

    let stored_hash =
        std::fs::read(&hash_path).map_err(|e| format!("Failed to read master hash: {}", e))?;
    let stored_salt =
        std::fs::read(&salt_path).map_err(|e| format!("Failed to read master salt: {}", e))?;

    let valid =
        verify_master(&password, &stored_salt, &stored_hash).map_err(|e| e.to_string())?;

    if valid {
        let key = derive_key(&password, &stored_salt).map_err(|e| e.to_string())?;

        let mut master_key = state.master_key.write().await;
        if let Some(ref mut old_key) = *master_key {
            old_key.zeroize();
        }
        *master_key = Some(key);

        tracing::info!("Master password verified, vault unlocked");
    } else {
        tracing::warn!("Master password verification failed");
    }

    Ok(valid)
}

/// Check whether the credential vault is locked.
///
/// Returns `true` if no master key is currently held in memory.
#[tauri::command]
pub async fn credential_is_locked(
    state: tauri::State<'_, AppState>,
) -> Result<bool, String> {
    let master_key = state.master_key.read().await;
    Ok(master_key.is_none())
}

/// Lock the credential vault by clearing the master key from memory.
///
/// The old key material is zeroized before being dropped.
#[tauri::command]
pub async fn credential_lock(
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut master_key = state.master_key.write().await;
    if let Some(ref mut key) = *master_key {
        key.zeroize();
    }
    *master_key = None;

    tracing::info!("Credential vault locked");
    Ok(())
}

/// Check whether a master password has been configured.
///
/// Returns `true` if the `master.hash` file exists in the app data directory.
#[tauri::command]
pub async fn credential_has_master_password(
    app: tauri::AppHandle,
) -> Result<bool, String> {
    let app_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;

    let hash_path = app_dir.join("master.hash");
    Ok(hash_path.exists())
}

/// Save an encrypted password for a session.
///
/// Requires the vault to be unlocked (master key in memory).
/// The password is encrypted with AES-256-GCM and stored in the credentials map.
#[tauri::command]
pub async fn credential_save_password(
    state: tauri::State<'_, AppState>,
    session_id: String,
    password: String,
) -> Result<(), String> {
    let master_key = state.master_key.read().await;
    let key = master_key
        .as_ref()
        .ok_or_else(|| "Vault is locked. Set a master password first.".to_string())?;

    let encrypted = vault::encrypt(key, password.as_bytes()).map_err(|e| e.to_string())?;

    let mut credentials = state.credentials.write().await;
    credentials.insert(session_id.clone(), encrypted);

    tracing::info!("Saved encrypted password for session {}", session_id);
    Ok(())
}

/// Retrieve a decrypted password for a session.
///
/// Requires the vault to be unlocked. Returns None if no password is saved for this session.
#[tauri::command]
pub async fn credential_get_password(
    state: tauri::State<'_, AppState>,
    session_id: String,
) -> Result<Option<String>, String> {
    let master_key = state.master_key.read().await;
    let key = match master_key.as_ref() {
        Some(k) => k,
        None => return Ok(None), // Vault locked, no password available
    };

    let credentials = state.credentials.read().await;
    let encrypted = match credentials.get(&session_id) {
        Some(c) => c,
        None => return Ok(None), // No saved password for this session
    };

    let plaintext = vault::decrypt(key, encrypted).map_err(|e| e.to_string())?;
    let password = String::from_utf8(plaintext)
        .map_err(|e| format!("Invalid UTF-8 in decrypted password: {}", e))?;

    Ok(Some(password))
}

/// Check if a password is saved for a given session.
#[tauri::command]
pub async fn credential_has_password(
    state: tauri::State<'_, AppState>,
    session_id: String,
) -> Result<bool, String> {
    let credentials = state.credentials.read().await;
    Ok(credentials.contains_key(&session_id))
}

/// Delete a saved password for a session.
#[tauri::command]
pub async fn credential_delete_password(
    state: tauri::State<'_, AppState>,
    session_id: String,
) -> Result<(), String> {
    let mut credentials = state.credentials.write().await;
    credentials.remove(&session_id);
    tracing::info!("Deleted saved password for session {}", session_id);
    Ok(())
}
