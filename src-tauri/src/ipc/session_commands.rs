//! Session commands using encrypted vault storage.
//!
//! All sessions are stored encrypted in SQLite using XChaCha20-Poly1305.
//! Lookups are O(1) using session_id as the primary key.

use crate::state::{AppState, AuthMethod, Folder, JumpHostConfig, SessionConfig};
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
        if let Ok(secrets) = manager.read_secrets_in(&vault_id, &["session", "custom:session"]).await {
            for (_, plaintext) in secrets {
                if let Ok(plaintext) = plaintext {
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
        // Session secrets only, including the legacy "custom:session" category.
        match manager.read_secrets_in(&vault_info.id, &["session", "custom:session"]).await {
            Ok(secrets) => {
                tracing::info!("session_list: vault {} has {} sessions", vault_info.name, secrets.len());
                for (secret, plaintext) in secrets {
                    match plaintext {
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

    let mut sessions = unique_sessions(sessions);
    for s in &mut sessions {
        open_approvals(&manager, s);
    }
    Ok(sessions)
}

/// What an approval is given for: the session and everything that decides
/// where and how it connects. Change any of it (a shared vault's other
/// members can) and the approvals given before no longer count. Credentials
/// are left out, so saving a password does not cost the approvals.
fn approval_context(s: &SessionConfig) -> String {
    use sha2::Digest;
    let jumps: Option<Vec<(String, u16, String)>> =
        s.jump_chain.as_ref().map(|c| c.iter().map(|j| (j.host.clone(), j.port, j.username.clone())).collect());
    let o = s.ssh_options.as_ref();
    let v = serde_json::json!([
        s.id,
        s.host,
        s.port,
        s.username,
        s.kind,
        s.via_session_id,
        s.shell,
        jumps,
        s.proxy,
        o.map(|o| &o.imported),
        o.map(|o| &o.lines),
    ]);
    sha2::Sha256::digest(v.to_string().as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

/// After reading a session: the user's approvals are the ones signed with
/// their key for this session as it now is; nothing else in the record
/// counts.
fn open_approvals(manager: &crate::vault::VaultManager, session: &mut SessionConfig) {
    let context = approval_context(session);
    if let Some(o) = session.ssh_options.as_mut() {
        let key = manager.approval_key();
        let who = manager.get_user_uuid();
        o.open_for(key.as_deref(), who.as_deref(), &context);
    }
}

/// Before storing a session: the user's approvals signed into it, for it as
/// it is saved.
fn seal_approvals(manager: &crate::vault::VaultManager, session: &mut SessionConfig) {
    let context = approval_context(session);
    if let Some(o) = session.ssh_options.as_mut() {
        let key = manager.approval_key();
        let who = manager.get_user_uuid();
        o.seal_for(key.as_deref(), who.as_deref(), &context);
    }
}

/// One entry per session id, the first one found.
///
/// The same session can be in two vaults: a shared vault joined twice is two
/// vaults over one database. The list keys its rows by id, and a duplicate
/// key is an error that stopped the whole list from rendering (it sat on
/// "loading" for good). Both copies are the same session, so one is shown.
fn unique_sessions(sessions: Vec<SessionConfig>) -> Vec<SessionConfig> {
    let mut seen = std::collections::HashMap::<String, Option<String>>::new();
    let mut unique = Vec::with_capacity(sessions.len());
    for session in sessions {
        if let Some(first) = seen.get(&session.id) {
            tracing::warn!(
                "session_list: session {} is in vault {:?} and again in {:?}; listing it once",
                session.id, first, session.vault_id
            );
            continue;
        }
        seen.insert(session.id.clone(), session.vault_id.clone());
        unique.push(session);
    }
    unique
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

    let mut session: SessionConfig = serde_json::from_str(&json).map_err(|e| format!("Invalid session data: {}", e))?;
    open_approvals(&manager, &mut session);
    Ok(session)
}

/// Create a new session configuration. O(1) insert.
/// If vault_id is provided and is a user vault, stores in that vault (for sharing).
/// Otherwise stores in __sessions__ (private).
#[tauri::command]
#[tracing::instrument(skip(state))]
#[expect(clippy::too_many_arguments, reason = "a Tauri command takes each argument from the frontend's invoke by name")]
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
    jump_chain: Option<Vec<JumpHostConfig>>,
    proxy: Option<crate::state::ProxyConfig>,
    shell: Option<String>,
    kind: Option<crate::state::SessionKind>,
    domain: Option<String>,
    detected_os: Option<String>,
    share_path: Option<String>,
    via_session_id: Option<String>,
    try_agent_keys: Option<bool>,
    ssh_options: Option<crate::ssh::sshconf::session::SshOptions>,
    wsl_distro: Option<String>,
) -> Result<SessionConfig, String> {
    let mut manager = state.vault_manager.lock().await;
    let kind = kind.unwrap_or_default();

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

    let mut session = SessionConfig {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        host,
        port,
        username,
        auth_method,
        kind,
        wsl_distro,
        domain: domain.filter(|d| !d.trim().is_empty()),
        share_path: share_path.filter(|d| !d.trim().is_empty()),
        via_session_id: via_session_id.filter(|s| !s.is_empty()),
        try_agent_keys: try_agent_keys.filter(|t| *t),
        folder_id,
        tags,
        // RDP cannot say what the machine is (xrdp claims to be Windows), so
        // the user says, Windows unless told otherwise; SSH sessions find
        // out on first connect.
        detected_os: detected_os
            .filter(|os| !os.is_empty())
            .or_else(|| (kind == crate::state::SessionKind::Rdp).then(|| "windows".to_string())),
        vault_id: if storage_vault_id != ensure_sessions_vault(&mut manager).await.unwrap_or_default() {
            Some(storage_vault_id.clone())
        } else {
            None
        },
        jump_chain,
        proxy,
        shell,
        ssh_options: ssh_options.filter(|o| !o.is_empty()),
    };

    seal_approvals(&manager, &mut session);
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
    open_approvals(&manager, &mut session);
    Ok(session)
}

/// Update an existing session configuration. O(1) update.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn session_update(
    state: State<'_, AppState>,
    mut session: SessionConfig,
) -> Result<SessionConfig, String> {
    let manager = state.vault_manager.lock().await;

    if manager.is_locked() {
        return Err("Vault is locked".to_string());
    }

    // Find which vault contains this session
    let storage_vault_id = find_session_vault(&manager, &session.id).await
        .ok_or_else(|| format!("Session not found: {}", session.id))?;

    seal_approvals(&manager, &mut session);
    let json = serde_json::to_string(&session).map_err(|e| e.to_string())?;
    let plaintext = SecretBox::new(Box::new(json.into_bytes()));

    // O(1) update by primary key
    manager
        .update_secret(&storage_vault_id, &session.id, plaintext)
        .await
        .map_err(|e| e.to_string())?;

    tracing::info!("Updated session: {} in vault: {}", session.id, storage_vault_id);
    open_approvals(&manager, &mut session);
    Ok(session)
}

/// Move many sessions into a folder (`None`: out of any folder) in one
/// go. Only `folder_id` changes: each stored session is read and written
/// back with every other field as it was, including fields a newer Reach
/// may have added. Returns how many moved; a session that no longer
/// exists is skipped.
#[tauri::command]
#[tracing::instrument(skip(state, session_ids), fields(count = session_ids.len()))]
pub async fn session_move_to_folder(
    state: State<'_, AppState>,
    session_ids: Vec<String>,
    folder_id: Option<String>,
) -> Result<u32, String> {
    let manager = state.vault_manager.lock().await;
    if manager.is_locked() {
        return Err("Vault is locked".to_string());
    }
    let mut moved = 0u32;
    for id in &session_ids {
        let Some(vault_id) = find_session_vault(&manager, id).await else { continue };
        let plaintext = manager.read_secret(&vault_id, id).await.map_err(|e| e.to_string())?;
        use secrecy::ExposeSecret;
        let mut value: serde_json::Value =
            serde_json::from_slice(plaintext.expose_secret()).map_err(|e| format!("Invalid session data: {e}"))?;
        let Some(obj) = value.as_object_mut() else { return Err(format!("Invalid session data: {id}")) };
        let target = folder_id.clone().map_or(serde_json::Value::Null, serde_json::Value::String);
        if obj.get("folder_id") == Some(&target) {
            continue;
        }
        obj.insert("folder_id".into(), target);
        let json = serde_json::to_vec(&value).map_err(|e| e.to_string())?;
        manager
            .update_secret(&vault_id, id, SecretBox::new(Box::new(json)))
            .await
            .map_err(|e| e.to_string())?;
        moved += 1;
    }
    tracing::info!("Moved {moved} sessions to folder {folder_id:?}");
    Ok(moved)
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
        .read_secrets_in(&vault_id, &["folder"])
        .await
        .map_err(|e| e.to_string())?;

    let mut folders = Vec::new();
    for (_, plaintext) in secrets {
        if let Ok(plaintext) = plaintext {
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
    vault_id: Option<String>,
) -> Result<Folder, String> {
    let mut manager = state.vault_manager.lock().await;

    if manager.is_locked() {
        return Err("Vault is locked. Set a master password first.".to_string());
    }

    let storage_vault_id = ensure_folders_vault(&mut manager).await?;

    let folder = Folder {
        id: uuid::Uuid::new_v4().to_string(),
        name: name.clone(),
        parent_id,
        vault_id,
    };

    let json = serde_json::to_string(&folder).map_err(|e| e.to_string())?;
    let plaintext = SecretBox::new(Box::new(json.into_bytes()));

    // O(1) insert
    manager
        .create_secret_with_id(
            &storage_vault_id,
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
#[tauri::command(rename_all = "snake_case")]
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

#[cfg(test)]
mod unique_tests {
    use super::*;

    fn session(id: &str, vault: &str) -> SessionConfig {
        serde_json::from_value(serde_json::json!({
            "id": id, "name": id, "host": "h", "port": 22, "username": "u",
            "auth_method": { "type": "Agent" }, "folder_id": null, "tags": [],
            "vault_id": vault,
        }))
        .unwrap()
    }

    #[test]
    fn a_session_in_two_vaults_is_listed_once() {
        let listed = unique_sessions(vec![session("a", "v1"), session("b", "v1"), session("a", "v2")]);
        let ids: Vec<_> = listed.iter().map(|s| (s.id.as_str(), s.vault_id.as_deref())).collect();
        assert_eq!(ids, [("a", Some("v1")), ("b", Some("v1"))]);
    }
}

#[cfg(test)]
mod approval_tests {
    use super::*;

    fn session() -> SessionConfig {
        serde_json::from_value(serde_json::json!({
            "id": "s1", "name": "web", "host": "web.example", "port": 22, "username": "u",
            "auth_method": { "type": "Agent" }, "folder_id": null, "tags": [],
            "ssh_options": { "lines": ["StrictHostKeyChecking no"] },
        }))
        .unwrap()
    }

    fn mine_after_reading(s: &SessionConfig, key: &[u8; 32]) -> Vec<String> {
        let mut s = s.clone();
        let context = approval_context(&s);
        let o = s.ssh_options.as_mut().unwrap();
        o.open_for(Some(key), Some("alice"), &context);
        o.my_accepted_weakenings.clone().unwrap()
    }

    #[test]
    fn an_approval_does_not_follow_a_changed_server() {
        let key = [3u8; 32];
        let mut s = session();
        s.ssh_options.as_mut().unwrap().my_accepted_weakenings = Some(vec!["StrictHostKeyChecking no".into()]);
        let context = approval_context(&s);
        s.ssh_options.as_mut().unwrap().seal_for(Some(&key), Some("alice"), &context);
        assert_eq!(mine_after_reading(&s, &key), vec!["StrictHostKeyChecking no"]);

        // Someone points the session at their own server: the approval stays
        // in the record but counts for nothing.
        let mut moved = s.clone();
        moved.host = "attacker.example".into();
        assert!(mine_after_reading(&moved, &key).is_empty());
        let mut lines = s.clone();
        lines.ssh_options.as_mut().unwrap().lines.push("ProxyJump evil".into());
        assert!(mine_after_reading(&lines, &key).is_empty());

        // Saving a password, a name or a folder changes none of that.
        let mut renamed = s.clone();
        renamed.name = "web (prod)".into();
        renamed.folder_id = Some("f".into());
        renamed.auth_method = AuthMethod::Password { password: Some("pw".into()) };
        assert_eq!(mine_after_reading(&renamed, &key), vec!["StrictHostKeyChecking no"]);
    }
}

/// List installed WSL distributions on Windows.
/// On non-Windows platforms, returns an empty list.
#[tauri::command]
pub async fn wsl_list_distros() -> Result<Vec<String>, String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW prevents command prompt flashing
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let output = std::process::Command::new("wsl.exe")
            .args(["-l", "-q"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|e| format!("Failed to execute wsl.exe: {}", e))?;
        if !output.status.success() {
            return Ok(Vec::new());
        }
        // wsl.exe output is UTF-16LE with null bytes
        let stdout = output.stdout;
        let text = if stdout.len() >= 2 && (stdout[1] == 0 || stdout[0] == 0) {
            let u16_vec: Vec<u16> = stdout
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            String::from_utf16_lossy(&u16_vec)
        } else {
            String::from_utf8_lossy(&stdout).to_string()
        };
        let distros: Vec<String> = text
            .lines()
            .map(|l| l.trim().trim_matches('\0').to_string())
            .filter(|l| !l.is_empty())
            .collect();
        Ok(distros)
    }
    #[cfg(not(target_os = "windows"))]
    {
        // WSL is only available on Windows
        Ok(Vec::new())
    }
}