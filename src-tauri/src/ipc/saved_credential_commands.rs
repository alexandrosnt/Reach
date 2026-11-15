//! Saved credentials: a login kept once and used by many sessions.
//!
//! A credential lives in a vault like the sessions that use it: a shared
//! vault's credentials in that vault, this device's own in `__sessions__`.
//! A session links one by `credential_id`, or takes the one its nearest
//! folder names; only a credential in the session's own vault counts.
//!
//! The link is resolved when a session is read ([`fill`]): the credential's
//! username, domain and login take the place of the session's own, which
//! stay stored as they were. So a session whose credential is gone (deleted,
//! or not synced to this device yet) still has a login, and an older Reach
//! reading the session sees the login it always had. Everything that reads
//! a session (the list, the editor, the connect commands, the approvals)
//! sees it filled in, so the approvals are given for the login really used.
//!
//! Password manager references are not allowed in a credential for now:
//! each reference is approved per session, and a credential would carry
//! one approval to every session that uses it.

use crate::ipc::session_commands as sessions;
use crate::state::{AppState, AuthMethod, CredentialUse, Folder, SavedCredential, SessionConfig};
use crate::vault::types::SecretCategory;
use secrecy::{ExposeSecret, SecretBox};
use tauri::State;

/// A session's `credential_id` that says: its own login, even in a folder
/// that names a credential.
pub(crate) const OWN_LOGIN: &str = "own";

/// A session's own login, as stored, while a credential's is filled in.
#[derive(Debug, Clone)]
pub(crate) struct OwnLogin {
    pub username: String,
    pub domain: Option<String>,
    pub auth_method: AuthMethod,
}

impl OwnLogin {
    pub(crate) fn of(s: &SessionConfig) -> Self {
        OwnLogin { username: s.username.clone(), domain: s.domain.clone(), auth_method: s.auth_method.clone() }
    }

    pub(crate) fn put_into(self, s: &mut SessionConfig) {
        s.username = self.username;
        s.domain = self.domain;
        s.auth_method = self.auth_method;
    }

    fn same(&self, other: &OwnLogin) -> bool {
        self.username == other.username
            && self.domain.as_deref().filter(|d| !d.is_empty()) == other.domain.as_deref().filter(|d| !d.is_empty())
            && serde_json::to_value(&self.auth_method).ok() == serde_json::to_value(&other.auth_method).ok()
    }
}

fn login_of(c: &SavedCredential) -> OwnLogin {
    OwnLogin { username: c.username.clone(), domain: c.domain.clone(), auth_method: c.auth_method.clone() }
}

/// The credential a session in `vault` (None: this device's own) logs in
/// with, and the folder it comes from (None: the session links it).
/// `Some((id, None))` with no folder means the session names it itself.
fn wanted(s: &SessionConfig, vault: Option<&str>, folders: &[Folder]) -> Option<(String, Option<String>)> {
    if let Some(id) = s.credential_id.as_deref().filter(|id| !id.is_empty()) {
        return (id != OWN_LOGIN).then(|| (id.to_string(), None));
    }
    let mut current = s.folder_id.as_deref().filter(|f| !f.is_empty());
    let mut seen = std::collections::HashSet::new();
    while let Some(id) = current {
        if !seen.insert(id) {
            break;
        }
        // A folder in another vault is not this session's (they cannot be
        // nested across vaults); its credential would not be either.
        let folder = folders.iter().find(|f| f.id == id && f.vault_id.as_deref() == vault)?;
        if let Some(cred) = folder.credential_id.as_deref().filter(|c| !c.is_empty()) {
            return Some((cred.to_string(), Some(folder.id.clone())));
        }
        current = folder.parent_id.as_deref();
    }
    None
}

/// Fill in the credential that applies to a session in `vault`, if any.
/// Returns the session's own login it replaced (None when nothing was
/// replaced: no credential applies, or it is missing).
pub(crate) fn fill(
    s: &mut SessionConfig,
    vault: Option<&str>,
    folders: &[Folder],
    creds: &[SavedCredential],
) -> Option<OwnLogin> {
    s.credential = None;
    let (id, folder_id) = wanted(s, vault, folders)?;
    let Some(c) = creds.iter().find(|c| c.id == id && c.vault_id.as_deref() == vault) else {
        s.credential = Some(CredentialUse { id, name: String::new(), folder_id, missing: true });
        return None;
    };
    let own = OwnLogin::of(s);
    login_of(c).put_into(s);
    s.credential = Some(CredentialUse { id, name: c.name.clone(), folder_id, missing: false });
    Some(own)
}

/// Which login to store for a session being saved. `sent` is the login in
/// the saved session as it came in; `filled` the logins a credential filled
/// in, now and when the stored record was read; `stored` the stored
/// record's own login. When what came in is one a credential filled in, it
/// is not the session's own (the editor shows the filled-in login): the
/// stored one is kept, and a credential's password is never copied into the
/// session.
pub(crate) fn login_to_store(sent: OwnLogin, filled: &[&OwnLogin], stored: Option<OwnLogin>) -> OwnLogin {
    match stored {
        Some(own) if filled.iter().any(|f| f.same(&sent)) => own,
        _ => sent,
    }
}

/// Every saved credential: this device's own (vault None) and each user
/// vault's. One a vault does not open here is left out until it does.
pub(crate) async fn read_credentials(manager: &crate::vault::VaultManager) -> Vec<SavedCredential> {
    let mut out = Vec::new();
    if let Some(vault) = sessions::sessions_vault_id(manager) {
        out.extend(credentials_in(manager, &vault, None).await);
    }
    for vault in manager.list_vaults().await.unwrap_or_default() {
        if vault.name.starts_with("__") {
            continue;
        }
        for c in credentials_in(manager, &vault.id, Some(vault.id.clone())).await {
            // The same credential in two vaults (a vault joined twice): once.
            if !out.iter().any(|o: &SavedCredential| o.id == c.id) {
                out.push(c);
            }
        }
    }
    out
}

async fn credentials_in(manager: &crate::vault::VaultManager, vault_id: &str, as_vault: Option<String>) -> Vec<SavedCredential> {
    let Ok(secrets) = manager.read_secrets_in(vault_id, &["credential"]).await else { return Vec::new() };
    secrets
        .into_iter()
        .filter_map(|(_, plaintext)| plaintext.ok())
        .filter_map(|p| serde_json::from_slice::<SavedCredential>(p.expose_secret()).ok())
        .map(|mut c| {
            // A vault has its own id on each device.
            c.vault_id = as_vault.clone();
            c
        })
        .collect()
}

/// What `fill` needs, read once.
pub(crate) struct Logins {
    pub folders: Vec<Folder>,
    pub creds: Vec<SavedCredential>,
}

pub(crate) async fn logins(manager: &crate::vault::VaultManager) -> Logins {
    Logins { folders: sessions::all_folders(manager).await, creds: read_credentials(manager).await }
}

/// The vault id a session's credential must be in, from the vault it is
/// stored in: None for this device's own sessions vault.
pub(crate) fn vault_of(manager: &crate::vault::VaultManager, storage_vault: &str) -> Option<String> {
    (sessions::sessions_vault_id(manager).as_deref() != Some(storage_vault)).then(|| storage_vault.to_string())
}

/// A credential that can be saved: named, with a username, and no password
/// manager references (see the module docs).
fn check(name: &str, username: &str, auth: &AuthMethod) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("A credential needs a name".to_string());
    }
    if username.trim().is_empty() {
        return Err("A credential needs a username".to_string());
    }
    match auth {
        AuthMethod::Password { password_ref: Some(_), .. } | AuthMethod::Key { passphrase_ref: Some(_), .. } => {
            Err("A saved credential cannot use a password manager entry yet".to_string())
        }
        _ => Ok(()),
    }
}

/// Where a credential's row is: its vault (as stored) and row id.
async fn locate(manager: &crate::vault::VaultManager, id: &str) -> Option<(String, String)> {
    if let Some(vault) = sessions::sessions_vault_id(manager) {
        if let Some(row) = sessions::row_id_in(manager, &vault, id).await {
            return Some((vault, row));
        }
    }
    let vault = sessions::user_vault_holding(manager, id).await?;
    Some((vault, id.to_string()))
}

/// All saved credentials the user can use, each with its vault.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn saved_credential_list(state: State<'_, AppState>) -> Result<Vec<SavedCredential>, String> {
    let manager = state.vault_manager.lock().await;
    if manager.is_locked() {
        return Ok(Vec::new());
    }
    Ok(read_credentials(&manager).await)
}

/// Save a new credential in a vault (None, or one not a user vault: this
/// device's own).
#[tauri::command]
// The login carries a password or key; only the vault is logged.
#[tracing::instrument(skip_all, fields(vault = ?vault_id))]
pub async fn saved_credential_create(
    state: State<'_, AppState>,
    name: String,
    username: String,
    domain: Option<String>,
    auth_method: AuthMethod,
    vault_id: Option<String>,
) -> Result<SavedCredential, String> {
    check(&name, &username, &auth_method)?;
    let mut manager = state.vault_manager.lock().await;
    if manager.is_locked() {
        return Err("Vault is locked. Set a master password first.".to_string());
    }
    let vault_id = match vault_id {
        Some(v) if sessions::user_vault(&manager, &v).await? => Some(v),
        _ => None,
    };
    let storage = match vault_id.clone() {
        Some(v) => v,
        None => sessions::sessions_vault(&mut manager).await?,
    };
    let cred = SavedCredential {
        id: uuid::Uuid::new_v4().to_string(),
        name: name.trim().to_string(),
        username: username.trim().to_string(),
        domain: domain.filter(|d| !d.trim().is_empty()),
        auth_method,
        vault_id,
    };
    let json = serde_json::to_vec(&cred).map_err(|e| e.to_string())?;
    manager
        .create_secret_with_id(&storage, &cred.id, &cred.name, SecretCategory::Credential, SecretBox::new(Box::new(json)))
        .await
        .map_err(|e| e.to_string())?;
    tracing::info!("Created saved credential {}", cred.id);
    Ok(cred)
}

/// Change a credential's name or login. It stays in its vault.
#[tauri::command]
#[tracing::instrument(skip_all, fields(id = %credential.id))]
pub async fn saved_credential_update(
    state: State<'_, AppState>,
    credential: SavedCredential,
) -> Result<SavedCredential, String> {
    check(&credential.name, &credential.username, &credential.auth_method)?;
    let manager = state.vault_manager.lock().await;
    if manager.is_locked() {
        return Err("Vault is locked".to_string());
    }
    let (vault, row) = locate(&manager, &credential.id)
        .await
        .ok_or_else(|| format!("Credential not found: {}", credential.id))?;
    let cred = SavedCredential {
        name: credential.name.trim().to_string(),
        username: credential.username.trim().to_string(),
        domain: credential.domain.filter(|d| !d.trim().is_empty()),
        vault_id: vault_of(&manager, &vault),
        ..credential
    };
    let json = serde_json::to_vec(&cred).map_err(|e| e.to_string())?;
    manager.update_secret(&vault, &row, SecretBox::new(Box::new(json))).await.map_err(|e| e.to_string())?;
    tracing::info!("Updated saved credential {}", cred.id);
    Ok(cred)
}

/// Delete a credential. Sessions and folders that name it stop naming it,
/// and go back to their own stored login. Returns how many were changed.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn saved_credential_delete(state: State<'_, AppState>, credential_id: String) -> Result<u32, String> {
    let manager = state.vault_manager.lock().await;
    if manager.is_locked() {
        return Err("Vault is locked".to_string());
    }
    let Some((vault, row)) = locate(&manager, &credential_id).await else {
        tracing::warn!("saved_credential_delete: {credential_id} is not in any open vault");
        return Ok(0);
    };
    let unlinked = sessions::unlink_credential(&manager, &credential_id).await?;
    manager.delete_secret(&vault, &row).await.map_err(|e| e.to_string())?;
    tracing::info!("Deleted saved credential {credential_id}, unlinked from {unlinked} sessions and folders");
    Ok(unlinked)
}

/// Check that a credential a session or folder in `vault` names is one it
/// can use: a credential in that same vault.
pub(crate) fn check_link(creds: &[SavedCredential], credential_id: Option<&str>, vault: Option<&str>) -> Result<(), String> {
    let Some(id) = credential_id.filter(|id| !id.is_empty() && *id != OWN_LOGIN) else { return Ok(()) };
    if creds.iter().any(|c| c.id == id && c.vault_id.as_deref() == vault) {
        Ok(())
    } else {
        Err("That credential is not in the same vault, so it cannot be used here".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(folder: Option<&str>, cred: Option<&str>, vault: Option<&str>) -> SessionConfig {
        serde_json::from_value(serde_json::json!({
            "id": "s1",
            "name": "web",
            "host": "web.example",
            "port": 22,
            "username": "own",
            "auth_method": { "type": "Password", "password": "own-pw" },
            "folder_id": folder,
            "tags": [],
            "vault_id": vault,
            "credential_id": cred,
        }))
        .unwrap()
    }

    fn folder(id: &str, parent: Option<&str>, cred: Option<&str>, vault: Option<&str>) -> Folder {
        Folder {
            id: id.into(),
            name: id.into(),
            parent_id: parent.map(Into::into),
            vault_id: vault.map(Into::into),
            credential_id: cred.map(Into::into),
        }
    }

    fn cred(id: &str, user: &str, vault: Option<&str>) -> SavedCredential {
        SavedCredential {
            id: id.into(),
            name: format!("{id} name"),
            username: user.into(),
            domain: None,
            auth_method: AuthMethod::Password { password: Some(format!("{user}-pw")), password_ref: None },
            vault_id: vault.map(Into::into),
        }
    }

    fn password(s: &SessionConfig) -> Option<&str> {
        match &s.auth_method {
            AuthMethod::Password { password, .. } => password.as_deref(),
            _ => None,
        }
    }

    #[test]
    fn a_linked_credential_fills_in_the_login() {
        let mut s = session(None, Some("c1"), None);
        let own = fill(&mut s, None, &[], &[cred("c1", "admin", None)]).unwrap();
        assert_eq!(s.username, "admin");
        assert_eq!(password(&s), Some("admin-pw"));
        assert_eq!(own.username, "own");
        let used = s.credential.unwrap();
        assert_eq!((used.id.as_str(), used.folder_id, used.missing), ("c1", None, false));
    }

    #[test]
    fn the_nearest_folder_credential_applies() {
        let folders = [folder("top", None, Some("c1"), None), folder("mid", Some("top"), Some("c2"), None), folder("leaf", Some("mid"), None, None)];
        let creds = [cred("c1", "one", None), cred("c2", "two", None)];
        let mut s = session(Some("leaf"), None, None);
        fill(&mut s, None, &folders, &creds);
        assert_eq!(s.username, "two");
        assert_eq!(s.credential.unwrap().folder_id.as_deref(), Some("mid"));
    }

    #[test]
    fn the_session_link_wins_over_the_folder() {
        let folders = [folder("f", None, Some("c1"), None)];
        let creds = [cred("c1", "one", None), cred("c2", "two", None)];
        let mut s = session(Some("f"), Some("c2"), None);
        fill(&mut s, None, &folders, &creds);
        assert_eq!(s.username, "two");
    }

    #[test]
    fn a_credential_in_another_vault_does_not_count() {
        let mut s = session(None, Some("c1"), Some("team"));
        assert!(fill(&mut s, Some("team"), &[], &[cred("c1", "admin", None)]).is_none());
        assert_eq!(s.username, "own");
        assert_eq!(password(&s), Some("own-pw"));
        assert!(s.credential.unwrap().missing);
    }

    #[test]
    fn a_folder_in_another_vault_gives_nothing() {
        let folders = [folder("f", None, Some("c1"), Some("other"))];
        let mut s = session(Some("f"), None, Some("team"));
        assert!(fill(&mut s, Some("team"), &folders, &[cred("c1", "admin", Some("other"))]).is_none());
        assert_eq!(s.username, "own");
        assert!(s.credential.is_none());
    }

    #[test]
    fn a_session_can_keep_its_own_login_in_a_folder_with_one() {
        let folders = [folder("f", None, Some("c1"), None)];
        let mut s = session(Some("f"), Some(OWN_LOGIN), None);
        assert!(fill(&mut s, None, &folders, &[cred("c1", "one", None)]).is_none());
        assert_eq!(s.username, "own");
        assert!(s.credential.is_none());
        assert!(check_link(&[], Some(OWN_LOGIN), None).is_ok());
    }

    #[test]
    fn a_folder_cycle_ends() {
        let folders = [folder("a", Some("b"), None, None), folder("b", Some("a"), None, None)];
        let mut s = session(Some("a"), None, None);
        assert!(fill(&mut s, None, &folders, &[]).is_none());
        assert!(s.credential.is_none());
    }

    #[test]
    fn no_credential_leaves_the_session_alone() {
        let mut s = session(None, None, None);
        s.credential = Some(CredentialUse { id: "sent".into(), name: "x".into(), folder_id: None, missing: false });
        assert!(fill(&mut s, None, &[], &[cred("c1", "admin", None)]).is_none());
        assert_eq!(s.username, "own");
        assert!(s.credential.is_none(), "what the webview sent is not kept");
    }

    #[test]
    fn a_filled_in_login_sent_back_is_not_stored() {
        let c = login_of(&cred("c1", "admin", None));
        let stored = OwnLogin::of(&session(None, None, None));
        let kept = login_to_store(c.clone(), &[&c], Some(stored));
        assert_eq!(kept.username, "own");
    }

    #[test]
    fn a_login_the_user_typed_is_stored() {
        let c = login_of(&cred("c1", "admin", None));
        let typed = OwnLogin { username: "new".into(), domain: None, auth_method: AuthMethod::Agent };
        let stored = OwnLogin::of(&session(None, None, None));
        assert_eq!(login_to_store(typed, &[&c], Some(stored)).username, "new");
    }

    #[test]
    fn a_credential_with_a_password_manager_entry_is_refused() {
        let r: crate::pwmanager::SecretRef =
            serde_json::from_value(serde_json::json!({ "provider": "passbolt", "entry_id": "e" })).unwrap();
        let auth = AuthMethod::Password { password: None, password_ref: Some(r) };
        assert!(check("n", "u", &auth).is_err());
        assert!(check("n", "u", &AuthMethod::Agent).is_ok());
        assert!(check(" ", "u", &AuthMethod::Agent).is_err());
    }

    #[test]
    fn links_stay_in_their_vault() {
        let creds = [cred("c1", "a", Some("team"))];
        assert!(check_link(&creds, Some("c1"), Some("team")).is_ok());
        assert!(check_link(&creds, Some("c1"), None).is_err());
        assert!(check_link(&creds, None, None).is_ok());
    }
}
