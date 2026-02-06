//! Session commands using encrypted vault storage.
//!
//! All sessions are stored encrypted in SQLite using XChaCha20-Poly1305.
//! Lookups are O(1) using session_id as the primary key.

use crate::state::{AppState, AuthMethod, Folder, SessionConfig};
use crate::vault::types::SecretCategory;
use secrecy::SecretBox;
use tauri::State;

const SESSIONS_VAULT_NAME: &str = "__sessions__";
const FOLDERS_VAULT_NAME: &str = "__folders__";

/// List all saved sessions from all accessible vaults. O(n) where n = total sessions.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn session_list(state: State<'_, AppState>) -> Result<Vec<SessionConfig>, String> {
    let manager = state.vault_manager.lock().await;

    if manager.is_locked() {
        return Ok(Vec::new());
    }

    let mut sessions = Vec::new();

    // 1. Get sessions from __sessions__ vault (private sessions)
    if let Some(vault_id) = get_sessions_vault_id_if_exists(&manager) {
        if let Ok(secrets) = manager.list_secrets(&vault_id).await {
            for secret in secrets {
                if let Ok(plaintext) = manager.read_secret(&vault_id, &secret.id).await {
                    use secrecy::ExposeSecret;
                    if let Ok(json) = String::from_utf8(plaintext.expose_secret().clone()) {
                        if let Ok(session) = serde_json::from_str::<SessionConfig>(&json) {
                            sessions.push(session);
                        }
                    }
                }
            }
        }
    }

    // 2. Get sessions from all user vaults (shared vaults)
    let user_vaults = manager.list_vaults().await.unwrap_or_default();
    tracing::info!("session_list: checking {} user vaults", user_vaults.len());
    for vault_info in user_vaults {
        // Skip internal vaults
        if vault_info.name.starts_with("__") {
            continue;
        }

        tracing::info!("session_list: checking vault {} ({})", vault_info.name, vault_info.id);
        match manager.list_secrets(&vault_info.id).await {
            Ok(secrets) => {
                tracing::info!("session_list: vault {} has {} secrets", vault_info.name, secrets.len());
                for secret in secrets {
                    // Only process session-type secrets (handle legacy "custom:session" format)
                    if secret.category != "session" && secret.category != "custom:session" {
                        tracing::debug!("session_list: skipping non-session secret: {} ({})", secret.name, secret.category);
                        continue;
                    }
                    tracing::info!("session_list: reading session secret: {}", secret.name);
                    match manager.read_secret(&vault_info.id, &secret.id).await {
                        Ok(plaintext) => {
                            use secrecy::ExposeSecret;
                            if let Ok(json) = String::from_utf8(plaintext.expose_secret().clone()) {
                                if let Ok(mut session) = serde_json::from_str::<SessionConfig>(&json) {
                                    // Ensure vault_id is set correctly
                                    session.vault_id = Some(vault_info.id.clone());
                                    tracing::info!("session_list: loaded session: {}", session.name);
                                    sessions.push(session);
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!("session_list: failed to read secret {}: {}", secret.id, e);
                        }
                    }
                }
            }
            Err(e) => {
                tracing::error!("session_list: failed to list secrets for vault {}: {}", vault_info.name, e);
            }
        }
    }

    Ok(sessions)
}

/// Get a specific session by ID. O(1) lookup.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn session_get(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<SessionConfig, String> {
    let manager = state.vault_manager.lock().await;

    if manager.is_locked() {
        return Err("Vault is locked".to_string());
    }

    // Find which vault contains this session
    let vault_id = find_session_vault(&manager, &session_id).await
        .ok_or_else(|| format!("Session not found: {}", session_id))?;

    // O(1) lookup by primary key
    let plaintext = manager
        .read_secret(&vault_id, &session_id)
        .await
        .map_err(|_| format!("Session not found: {}", session_id))?;

    use secrecy::ExposeSecret;
    let json = String::from_utf8(plaintext.expose_secret().clone())
        .map_err(|e| format!("Invalid UTF-8: {}", e))?;

    serde_json::from_str(&json).map_err(|e| format!("Invalid session data: {}", e))
}

/// Create a new session configuration. O(1) insert.
/// If vault_id is provided and is a user vault, stores in that vault (for sharing).
/// Otherwise stores in __sessions__ (private).
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn session_create(
    state: State<'_, AppState>,
    name: String,
    host: String,
    port: u16,
    username: String,
    auth_method: AuthMethod,
    folder_id: Option<String>,
    tags: Vec<String>,
    vault_id: Option<String>,
) -> Result<SessionConfig, String> {
    let mut manager = state.vault_manager.lock().await;

    if manager.is_locked() {
        return Err("Vault is locked. Set a master password first.".to_string());
    }

    // Determine storage vault: user vault (shared) or __sessions__ (private)
    let storage_vault_id = if let Some(ref vid) = vault_id {
        // Check if this is a user vault (not internal)
        let vaults = manager.list_vaults().await.map_err(|e| e.to_string())?;
        let is_user_vault = vaults.iter().any(|v| v.id == *vid && !v.name.starts_with("__"));
        if is_user_vault {
            tracing::info!("Storing session in user vault: {}", vid);
            vid.clone()
        } else {
            ensure_sessions_vault(&mut manager).await?
        }
    } else {
        ensure_sessions_vault(&mut manager).await?
    };

    let session = SessionConfig {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        host,
        port,
        username,
        auth_method,
        folder_id,
        tags,
        detected_os: None,
        vault_id: if storage_vault_id != ensure_sessions_vault(&mut manager).await.unwrap_or_default() {
            Some(storage_vault_id.clone())
        } else {
            None
        },
    };

    let json = serde_json::to_string(&session).map_err(|e| e.to_string())?;
    let plaintext = SecretBox::new(Box::new(json.into_bytes()));

    // O(1) insert with session.id as the secret_id
    manager
        .create_secret_with_id(
            &storage_vault_id,
            &session.id,
            &session.name,
            SecretCategory::Session,
            plaintext,
        )
        .await
        .map_err(|e| e.to_string())?;

    tracing::info!("Created session: {} in storage vault: {}", session.id, storage_vault_id);
    Ok(session)
}

/// Update an existing session configuration. O(1) update.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn session_update(
    state: State<'_, AppState>,
    session: SessionConfig,
) -> Result<SessionConfig, String> {
    let manager = state.vault_manager.lock().await;

    if manager.is_locked() {
        return Err("Vault is locked".to_string());
    }

    // Find which vault contains this session
    let storage_vault_id = find_session_vault(&manager, &session.id).await
        .ok_or_else(|| format!("Session not found: {}", session.id))?;

    let json = serde_json::to_string(&session).map_err(|e| e.to_string())?;
    let plaintext = SecretBox::new(Box::new(json.into_bytes()));

    // O(1) update by primary key
    manager
        .update_secret(&storage_vault_id, &session.id, plaintext)
        .await
        .map_err(|e| e.to_string())?;

    tracing::info!("Updated session: {} in vault: {}", session.id, storage_vault_id);
    Ok(session)
}

/// Delete a session by ID. O(1) delete.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn session_delete(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<(), String> {
    let manager = state.vault_manager.lock().await;

    if manager.is_locked() {
        return Err("Vault is locked".to_string());
    }

    // Find which vault contains this session
    let vault_id = match find_session_vault(&manager, &session_id).await {
        Some(id) => id,
        None => return Ok(()), // Already deleted or doesn't exist
    };

    // O(1) delete by primary key
    manager
        .delete_secret(&vault_id, &session_id)
        .await
        .map_err(|e| e.to_string())?;

    tracing::info!("Deleted session: {} from vault: {}", session_id, vault_id);
    Ok(())
}

/// List all session folders. O(n) where n = number of folders.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn session_list_folders(state: State<'_, AppState>) -> Result<Vec<Folder>, String> {
    let manager = state.vault_manager.lock().await;

    if manager.is_locked() {
        return Ok(Vec::new());
    }

    let vault_id = match get_folders_vault_id_if_exists(&manager) {
        Some(id) => id,
        None => return Ok(Vec::new()),
    };

    let secrets = manager
        .list_secrets(&vault_id)
        .await
        .map_err(|e| e.to_string())?;

    let mut folders = Vec::with_capacity(secrets.len());
    for secret in secrets {
        if let Ok(plaintext) = manager.read_secret(&vault_id, &secret.id).await {
            use secrecy::ExposeSecret;
            if let Ok(json) = String::from_utf8(plaintext.expose_secret().clone()) {
                if let Ok(folder) = serde_json::from_str::<Folder>(&json) {
                    folders.push(folder);
                }
            }
        }
    }

    Ok(folders)
}

/// Create a new session folder. O(1) insert.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn session_create_folder(
    state: State<'_, AppState>,
    name: String,
    parent_id: Option<String>,
) -> Result<Folder, String> {
    let mut manager = state.vault_manager.lock().await;

    if manager.is_locked() {
        return Err("Vault is locked. Set a master password first.".to_string());
    }

    let vault_id = ensure_folders_vault(&mut manager).await?;

    let folder = Folder {
        id: uuid::Uuid::new_v4().to_string(),
        name: name.clone(),
        parent_id,
    };

    let json = serde_json::to_string(&folder).map_err(|e| e.to_string())?;
    let plaintext = SecretBox::new(Box::new(json.into_bytes()));

    // O(1) insert
    manager
        .create_secret_with_id(
            &vault_id,
            &folder.id,
            &name,
            SecretCategory::Folder,
            plaintext,
        )
        .await
        .map_err(|e| e.to_string())?;

    tracing::info!("Created folder: {}", folder.id);
    Ok(folder)
}

/// Delete a session folder by ID. O(1) delete.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn session_delete_folder(
    state: State<'_, AppState>,
    folder_id: String,
) -> Result<(), String> {
    let manager = state.vault_manager.lock().await;

    if manager.is_locked() {
        return Err("Vault is locked".to_string());
    }

    let vault_id = match get_folders_vault_id_if_exists(&manager) {
        Some(id) => id,
        None => return Ok(()),
    };

    // O(1) delete by primary key
    manager
        .delete_secret(&vault_id, &folder_id)
        .await
        .map_err(|e| e.to_string())?;

    tracing::info!("Deleted folder: {}", folder_id);
    Ok(())
}

// --- Helper functions (all O(1)) ---

/// Ensure the sessions vault exists. O(1).
async fn ensure_sessions_vault(
    manager: &mut crate::vault::VaultManager,
) -> Result<String, String> {
    if let Some(vault_id) = manager.get_vault_id_by_name(SESSIONS_VAULT_NAME) {
        let _ = manager.open_vault(&vault_id, None, None).await;
        manager
            .unlock_vault(&vault_id)
            .await
            .map_err(|e| e.to_string())?;
        Ok(vault_id)
    } else {
        let vault = manager
            .create_vault(SESSIONS_VAULT_NAME, crate::vault::types::VaultType::Private, None, None)
            .await
            .map_err(|e| e.to_string())?;
        Ok(vault.id)
    }
}

/// Ensure the folders vault exists. O(1).
async fn ensure_folders_vault(
    manager: &mut crate::vault::VaultManager,
) -> Result<String, String> {
    if let Some(vault_id) = manager.get_vault_id_by_name(FOLDERS_VAULT_NAME) {
        let _ = manager.open_vault(&vault_id, None, None).await;
        manager
            .unlock_vault(&vault_id)
            .await
            .map_err(|e| e.to_string())?;
        Ok(vault_id)
    } else {
        let vault = manager
            .create_vault(FOLDERS_VAULT_NAME, crate::vault::types::VaultType::Private, None, None)
            .await
            .map_err(|e| e.to_string())?;
        Ok(vault.id)
    }
}

/// Get sessions vault ID. O(1).
fn get_sessions_vault_id(manager: &crate::vault::VaultManager) -> Result<String, String> {
    manager
        .get_vault_id_by_name(SESSIONS_VAULT_NAME)
        .ok_or_else(|| "Sessions vault not found".to_string())
}

/// Get sessions vault ID if exists. O(1).
fn get_sessions_vault_id_if_exists(manager: &crate::vault::VaultManager) -> Option<String> {
    manager.get_vault_id_by_name(SESSIONS_VAULT_NAME)
}

/// Get folders vault ID if exists. O(1).
fn get_folders_vault_id_if_exists(manager: &crate::vault::VaultManager) -> Option<String> {
    manager.get_vault_id_by_name(FOLDERS_VAULT_NAME)
}

/// Find which vault contains a session by ID.
/// Checks __sessions__ first, then all user vaults.
async fn find_session_vault(manager: &crate::vault::VaultManager, session_id: &str) -> Option<String> {
    // Check __sessions__ vault first
    if let Some(vault_id) = get_sessions_vault_id_if_exists(manager) {
        if manager.secret_exists(&vault_id, session_id).await {
            return Some(vault_id);
        }
    }

    // Check all user vaults
    if let Ok(vaults) = manager.list_vaults().await {
        for vault in vaults {
            if vault.name.starts_with("__") {
                continue;
            }
            if manager.secret_exists(&vault.id, session_id).await {
                return Some(vault.id);
            }
        }
    }

    None
}

/// Share a session with another user via X25519 key re-wrap.
#[tauri::command]
#[tracing::instrument(skip(state, recipient_public_key))]
pub async fn session_share(
    state: State<'_, AppState>,
    session_id: String,
    recipient_uuid: String,
    recipient_public_key: String,
    expires_in_hours: Option<u64>,
) -> Result<crate::vault::types::ShareItemResult, String> {
    let manager = state.vault_manager.lock().await;

    if manager.is_locked() {
        return Err("Vault is locked".to_string());
    }

    let vault_id = get_sessions_vault_id(&manager)?;

    // Decode recipient public key from base64
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
    let pk_bytes = BASE64
        .decode(&recipient_public_key)
        .map_err(|e| format!("Invalid public key base64: {}", e))?;

    if pk_bytes.len() != 32 {
        return Err(format!("Invalid public key length: expected 32, got {}", pk_bytes.len()));
    }

    let mut pk_array = [0u8; 32];
    pk_array.copy_from_slice(&pk_bytes);

    manager
        .share_item(&vault_id, &session_id, &recipient_uuid, &pk_array, expires_in_hours)
        .await
        .map_err(|e| e.to_string())
}
