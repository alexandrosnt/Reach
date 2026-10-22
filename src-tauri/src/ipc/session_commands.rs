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
// Only what names the session: the login, jump hosts and proxy carry
// passwords, passphrases and embedded keys, and every argument left in an
// instrument span is written to reach.log.
#[tracing::instrument(skip_all, fields(host = %host, port = port, kind = ?kind))]
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
// The session carries its secrets; only its id is logged.
#[tracing::instrument(skip_all, fields(id = %session.id))]
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

    move_folders_to_their_vaults(&manager).await;
    read_folders(&manager).await
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

    create_folder(&mut manager, name, parent_id, vault_id).await
}

async fn create_folder(
    manager: &mut crate::vault::VaultManager,
    name: String,
    parent_id: Option<String>,
    vault_id: Option<String>,
) -> Result<Folder, String> {
    // A folder lives in the vault its sessions are in, so it syncs with
    // them; one for this device's own sessions stays in __folders__.
    let vault_id = match vault_id {
        Some(vid) if is_user_vault(manager, &vid).await? => Some(vid),
        _ => None,
    };
    let storage_vault_id = match vault_id {
        Some(ref vid) => vid.clone(),
        None => ensure_folders_vault(manager).await?,
    };
    if let Some(ref parent_id) = parent_id {
        let parent = read_folders(manager)
            .await?
            .into_iter()
            .find(|folder| folder.id == *parent_id)
            .ok_or_else(|| format!("Parent folder not found: {parent_id}"))?;
        if parent.vault_id != vault_id {
            return Err("Folders in different vaults cannot be nested".to_string());
        }
    }

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

/// Move a folder under another folder, or to the root.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn session_update_folder_parent(
    state: State<'_, AppState>,
    folder_id: String,
    parent_id: Option<String>,
) -> Result<Folder, String> {
    let manager = state.vault_manager.lock().await;
    if manager.is_locked() {
        return Err("Vault is locked".to_string());
    }

    set_folder_parent(&manager, folder_id, parent_id).await
}

async fn set_folder_parent(
    manager: &crate::vault::VaultManager,
    folder_id: String,
    parent_id: Option<String>,
) -> Result<Folder, String> {
    let folders = read_folders(manager).await?;
    let by_id = folders
        .iter()
        .map(|folder| (folder.id.as_str(), folder))
        .collect::<std::collections::HashMap<_, _>>();
    let mut folder = by_id
        .get(folder_id.as_str())
        .cloned()
        .cloned()
        .ok_or_else(|| format!("Folder not found: {folder_id}"))?;

    if let Some(ref target_id) = parent_id {
        let target = by_id
            .get(target_id.as_str())
            .ok_or_else(|| format!("Parent folder not found: {target_id}"))?;
        if target.vault_id != folder.vault_id {
            return Err("Folders in different vaults cannot be nested".to_string());
        }

        let mut current = Some(target_id.as_str());
        let mut seen = std::collections::HashSet::new();
        while let Some(id) = current {
            if id == folder_id {
                return Err("A folder cannot be moved into itself or its descendant".to_string());
            }
            if !seen.insert(id) {
                return Err("Folder hierarchy contains a cycle".to_string());
            }
            current = by_id.get(id).and_then(|item| item.parent_id.as_deref());
        }
    }

    folder.parent_id = parent_id;
    let json = serde_json::to_string(&folder).map_err(|e| e.to_string())?;
    let plaintext = SecretBox::new(Box::new(json.into_bytes()));
    let vault_id = find_folder_vault(manager, &folder.id)
        .await
        .ok_or_else(|| format!("Folder not found: {}", folder.id))?;
    manager
        .update_secret(&vault_id, &folder.id, plaintext)
        .await
        .map_err(|e| e.to_string())?;

    tracing::info!("Moved folder: {} under {:?}", folder.id, folder.parent_id);
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

    delete_folder(&manager, &folder_id).await
}

/// Removes the folder from its vault and any copy left in __folders__ by a
/// move cut short, which would otherwise be moved back on the next listing.
async fn delete_folder(manager: &crate::vault::VaultManager, folder_id: &str) -> Result<(), String> {
    let mut holders: Vec<String> = find_in_user_vaults(manager, folder_id).await.into_iter().collect();
    if let Some(local) = get_folders_vault_id_if_exists(manager) {
        if manager.secret_exists(&local, folder_id).await {
            holders.push(local);
        }
    }
    for vault_id in holders {
        manager
            .delete_secret(&vault_id, folder_id)
            .await
            .map_err(|e| e.to_string())?;
        tracing::info!("Deleted folder: {} from vault: {}", folder_id, vault_id);
    }
    Ok(())
}

/// Every folder: this device's own from __folders__, and each user vault's
/// from that vault. A folder found in both (a move cut short) is the user
/// vault's.
async fn read_folders(manager: &crate::vault::VaultManager) -> Result<Vec<Folder>, String> {
    let mut folders: Vec<Folder> = Vec::new();
    if let Some(vault_id) = get_folders_vault_id_if_exists(manager) {
        folders.extend(folders_in(manager, &vault_id).await?);
    }
    for vault in manager.list_vaults().await.unwrap_or_default() {
        if vault.name.starts_with("__") {
            continue;
        }
        // A vault that is not open here (unreachable, not yet unlocked)
        // shows its folders once it is.
        let Ok(found) = folders_in(manager, &vault.id).await else { continue };
        for mut folder in found {
            // A vault has its own id on each device; the stored folder
            // names the one it was made on.
            folder.vault_id = Some(vault.id.clone());
            folders.retain(|f| f.id != folder.id);
            folders.push(folder);
        }
    }
    Ok(folders)
}

async fn folders_in(manager: &crate::vault::VaultManager, vault_id: &str) -> Result<Vec<Folder>, String> {
    let secrets = manager
        .read_secrets_in(vault_id, &["folder"])
        .await
        .map_err(|e| e.to_string())?;
    let mut folders = Vec::new();
    for (_, plaintext) in secrets {
        if let Ok(plaintext) = plaintext {
            use secrecy::ExposeSecret;
            if let Ok(folder) = serde_json::from_slice::<Folder>(plaintext.expose_secret()) {
                folders.push(folder);
            }
        }
    }
    Ok(folders)
}

/// Before 0.8.2 every folder was kept in __folders__, which never leaves
/// the device, so a shared vault's folders reached no other device or
/// member. Each one is copied into its vault and only then removed here;
/// one whose vault is not open now is moved on a later listing.
async fn move_folders_to_their_vaults(manager: &crate::vault::VaultManager) {
    let Some(local) = get_folders_vault_id_if_exists(manager) else { return };
    let Ok(secrets) = manager.read_secrets_in(&local, &["folder"]).await else { return };
    let user_vaults: Vec<String> = manager
        .list_vaults()
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|v| !v.name.starts_with("__"))
        .map(|v| v.id)
        .collect();
    for (meta, plaintext) in secrets {
        let Ok(plaintext) = plaintext else { continue };
        use secrecy::ExposeSecret;
        let Ok(folder) = serde_json::from_slice::<Folder>(plaintext.expose_secret()) else { continue };
        let Some(target) = folder.vault_id.as_ref().filter(|id| user_vaults.contains(id)) else { continue };
        let copy = SecretBox::new(Box::new(plaintext.expose_secret().clone()));
        if let Err(e) = manager
            .create_secret_with_id(target, &folder.id, &meta.name, SecretCategory::Folder, copy)
            .await
        {
            tracing::warn!("Folder {} stays on this device for now: {}", folder.id, e);
            continue;
        }
        match manager.delete_secret(&local, &folder.id).await {
            Ok(()) => tracing::info!("Moved folder {} into vault {}", folder.id, target),
            Err(e) => tracing::warn!("Folder {} copied to vault {}, local copy left: {}", folder.id, target, e),
        }
    }
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
    if let Some(vault_id) = get_sessions_vault_id_if_exists(manager) {
        if manager.secret_exists(&vault_id, session_id).await {
            return Some(vault_id);
        }
    }
    find_in_user_vaults(manager, session_id).await
}

/// Find which vault contains a folder by ID: the user vaults first, as
/// `read_folders` prefers them, then __folders__.
async fn find_folder_vault(manager: &crate::vault::VaultManager, folder_id: &str) -> Option<String> {
    if let Some(vault_id) = find_in_user_vaults(manager, folder_id).await {
        return Some(vault_id);
    }
    let local = get_folders_vault_id_if_exists(manager)?;
    manager.secret_exists(&local, folder_id).await.then_some(local)
}

/// The user vault holding a secret, if any.
async fn find_in_user_vaults(manager: &crate::vault::VaultManager, secret_id: &str) -> Option<String> {
    if let Ok(vaults) = manager.list_vaults().await {
        for vault in vaults {
            if vault.name.starts_with("__") {
                continue;
            }
            if manager.secret_exists(&vault.id, secret_id).await {
                return Some(vault.id);
            }
        }
    }

    None
}

/// Whether an id names one of the user's own or joined vaults, not an internal one.
async fn is_user_vault(manager: &crate::vault::VaultManager, vault_id: &str) -> Result<bool, String> {
    let vaults = manager.list_vaults().await.map_err(|e| e.to_string())?;
    Ok(vaults.iter().any(|v| v.id == vault_id && !v.name.starts_with("__")))
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

/// The WSL distributions a person can open, on Windows; elsewhere none.
#[tauri::command]
pub async fn wsl_list_distros() -> Result<Vec<String>, String> {
    if !cfg!(windows) {
        return Ok(Vec::new());
    }
    // No WSL installed, or not answering: nothing to offer, not an error.
    Ok(crate::wsl::run(&["-l", "-q"], crate::wsl::LIST_WITHIN)
        .await
        .map(|listing| crate::wsl::distros(&listing))
        .unwrap_or_default())
}

/// Folders of a shared vault, end to end between two devices over a real
/// libsql server: an old folder kept on the device moves into its vault,
/// new ones are made there, and another member sees, nests and deletes
/// them with no copy coming back.
///
/// Needs a libsql server, so it is skipped unless one is named, e.g.
/// `docker run -d -p 58080:8080 ghcr.io/tursodatabase/libsql-server` and
/// `REACH_TEST_SQLD=http://127.0.0.1:58080`.
#[cfg(test)]
mod folder_sync_tests {
    use super::*;
    use crate::vault::types::{MemberRole, VaultType};
    use crate::vault::VaultManager;

    fn device(name: &str) -> VaultManager {
        let dir = std::env::temp_dir().join(format!("reach-folder-sync-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        VaultManager::new(dir)
    }

    fn put(folder: &Folder) -> SecretBox<Vec<u8>> {
        SecretBox::new(Box::new(serde_json::to_vec(folder).unwrap()))
    }

    fn names(folders: &[Folder]) -> Vec<String> {
        let mut names: Vec<String> = folders.iter().map(|f| f.name.clone()).collect();
        names.sort();
        names
    }

    #[tokio::test]
    #[ignore]
    async fn a_shared_vaults_folders_reach_the_other_member() {
        let Ok(url) = std::env::var("REACH_TEST_SQLD") else { return };
        let server = libsql::Builder::new_remote(url.clone(), String::new())
            .connector(crate::vault::turso_tls::TursoConnector::new().unwrap())
            .build()
            .await
            .unwrap();
        server
            .connect()
            .unwrap()
            .execute_batch("DROP TABLE IF EXISTS secrets; DROP TABLE IF EXISTS vault_members; DROP TABLE IF EXISTS vault_header;")
            .await
            .unwrap();

        let mut a = device("a");
        a.init_identity("folder-sync-a").await.unwrap();
        let team = a.create_vault("Team", VaultType::Private, Some(&url), Some("")).await.unwrap().id;

        // As Reach before 0.8.2 left them: both folders on the device, one
        // naming the shared vault.
        let local = ensure_folders_vault(&mut a).await.unwrap();
        let old = Folder { id: "old".into(), name: "Servers".into(), parent_id: None, vault_id: Some(team.clone()) };
        let mine = Folder { id: "mine".into(), name: "Home".into(), parent_id: None, vault_id: None };
        for f in [&old, &mine] {
            a.create_secret_with_id(&local, &f.id, &f.name, SecretCategory::Folder, put(f)).await.unwrap();
        }

        move_folders_to_their_vaults(&a).await;
        assert!(a.secret_exists(&team, "old").await, "the shared folder is in its vault");
        assert!(!a.secret_exists(&local, "old").await, "and no longer on the device");
        assert!(a.secret_exists(&local, "mine").await, "a folder of the device's own sessions stays");
        move_folders_to_their_vaults(&a).await;
        assert_eq!(names(&read_folders(&a).await.unwrap()), ["Home", "Servers"], "a second run changes nothing");

        // A new folder in the shared vault, nested under the moved one.
        let web = create_folder(&mut a, "Web".into(), Some("old".into()), Some(team.clone())).await.unwrap();
        assert!(a.secret_exists(&team, &web.id).await);
        assert!(create_folder(&mut a, "Mixed".into(), Some("mine".into()), Some(team.clone())).await.is_err());

        // Device B joins; the vault has its own id there.
        let mut b = device("b");
        b.init_identity("folder-sync-b").await.unwrap();
        let b_key: [u8; 32] = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b.get_public_key().unwrap())
            .unwrap()
            .try_into()
            .unwrap();
        let invite = a.invite_member(&team, &b_key, &b.get_user_uuid().unwrap(), MemberRole::Member).await.unwrap();
        let b_team = b.accept_invite(&invite.sync_url, &invite.token).await.unwrap().id;
        assert_ne!(b_team, team);

        let seen = read_folders(&b).await.unwrap();
        assert_eq!(names(&seen), ["Servers", "Web"], "B sees the shared folders, not A's own");
        assert!(seen.iter().all(|f| f.vault_id.as_deref() == Some(b_team.as_str())), "under B's id for the vault");
        assert_eq!(seen.iter().find(|f| f.name == "Web").unwrap().parent_id.as_deref(), Some("old"));

        // B moves Web to the top and deletes Servers; A sees both.
        set_folder_parent(&b, web.id.clone(), None).await.unwrap();
        delete_folder(&b, "old").await.unwrap();
        a.sync_vault(&team).await.unwrap();
        let after = read_folders(&a).await.unwrap();
        assert_eq!(names(&after), ["Home", "Web"]);
        assert_eq!(after.iter().find(|f| f.name == "Web").unwrap().parent_id, None);

        // A copy left on the device by a move cut short goes with the folder.
        a.create_secret_with_id(&local, &web.id, "Web", SecretCategory::Folder, put(&web)).await.unwrap();
        delete_folder(&a, &web.id).await.unwrap();
        move_folders_to_their_vaults(&a).await;
        assert_eq!(names(&read_folders(&a).await.unwrap()), ["Home"]);
    }
}
