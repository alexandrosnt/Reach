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
    // The via sessions, as stored (approvals do not enter a context), from
    // the same list rather than read again one by one.
    let stored: std::collections::HashMap<String, SessionConfig> =
        sessions.iter().map(|s| (s.id.clone(), s.clone())).collect();
    for s in &mut sessions {
        let via = via_id(s).and_then(|id| stored.get(id));
        open_approvals_with(&manager, s, via);
    }
    Ok(sessions)
}

/// What an approval is given for: the session and everything that decides
/// where and how it connects. Change any of it (a shared vault's other
/// members can) and the approvals given before no longer count. Credentials
/// are left out, so saving a password does not cost the approvals; that
/// includes the proxy's password and its password manager entry, which are
/// left out of the proxy as well (a proxy with neither hashes as it did
/// before they existed).
pub(crate) fn approval_context(s: &SessionConfig) -> String {
    use sha2::Digest;
    let jumps: Option<Vec<(String, u16, String)>> =
        s.jump_chain.as_ref().map(|c| c.iter().map(|j| (j.host.clone(), j.port, j.username.clone())).collect());
    let proxy = s.proxy.as_ref().map(|p| crate::state::ProxyConfig { password: None, password_ref: None, ..p.clone() });
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
        proxy,
        o.map(|o| &o.imported),
        o.map(|o| &o.lines),
    ]);
    sha2::Sha256::digest(v.to_string().as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

/// The SSH session a session reaches its server through: a VNC session's
/// `via_session_id` (the field is VNC only; other kinds ignore it when they
/// connect, so it decides nothing for them).
pub(crate) fn via_id(s: &SessionConfig) -> Option<&str> {
    if s.kind != crate::state::SessionKind::Vnc {
        return None;
    }
    s.via_session_id.as_deref().filter(|v| !v.is_empty())
}

/// What a session's password manager references are approved for: its own
/// [`approval_context`], and for a session that goes through an SSH session
/// ([`via_id`]), that session's as it is stored now. The tunnel is built
/// from the via session when connecting, and another member of its vault
/// can re-point it; a reference approved for one way in must not follow it
/// to another server. `via` is the via session as read now; `None` when the
/// session has one and it is gone (or is not the one named): then nothing
/// can be approved and no reference counts.
///
/// Only the references' context gains the via part; the ssh_config
/// approvals keep [`approval_context`], since they are about the session's
/// own login.
pub(crate) fn ref_context(s: &SessionConfig, via: Option<&SessionConfig>) -> Option<String> {
    let own = approval_context(s);
    match via_id(s) {
        None => Some(own),
        Some(id) => via.filter(|v| v.id == id).map(|v| format!("{own}\tvia\t{}", approval_context(v))),
    }
}

/// The via session of `s` as stored now, if it has one and it still exists.
async fn via_for(manager: &crate::vault::VaultManager, s: &SessionConfig) -> Option<SessionConfig> {
    let id = via_id(s)?;
    let vault_id = find_session_vault(manager, id).await?;
    read_session(manager, &vault_id, id).await.ok()
}

/// After reading a session: the user's approvals are the ones signed with
/// their key for this session as it now is; nothing else in the record
/// counts. Returns the via session the references were checked against
/// (see [`ref_context`]).
async fn open_approvals(manager: &crate::vault::VaultManager, session: &mut SessionConfig) -> Option<SessionConfig> {
    let via = via_for(manager, session).await;
    open_approvals_with(manager, session, via.as_ref());
    via
}

fn open_approvals_with(manager: &crate::vault::VaultManager, session: &mut SessionConfig, via: Option<&SessionConfig>) {
    let key = manager.approval_key();
    let who = manager.get_user_uuid();
    open_for(key.as_deref(), who.as_deref(), session, via);
}

fn open_for(key: Option<&[u8; 32]>, who: Option<&str>, session: &mut SessionConfig, via: Option<&SessionConfig>) {
    use crate::ssh::sshconf::approvals::{verified, Kind};
    let context = approval_context(session);
    let refs_context = ref_context(session, via);
    if let Some(o) = session.ssh_options.as_mut() {
        o.open_for(key, who, &context);
    }
    for (slot, r) in ref_slots_mut(session) {
        let what = ref_what(&slot, r);
        r.trusted = match (key, who, refs_context.as_deref()) {
            (Some(k), Some(w), Some(c)) => verified(&r.approvals, k, w, Kind::SecretRef, c).contains(&what),
            _ => false,
        };
        r.confirm = false;
    }
}

/// Before storing a session: the user's approvals signed into it, for it as
/// it is saved. `stored` is the record being replaced, as read for this
/// user (`None` for a new session).
async fn seal_approvals(manager: &crate::vault::VaultManager, session: &mut SessionConfig, stored: Option<&SessionConfig>) {
    let via = via_for(manager, session).await;
    let key = manager.approval_key();
    let who = manager.get_user_uuid();
    seal_for(key.as_deref(), who.as_deref(), session, stored, via.as_ref());
}

/// Which password manager references the saving person signs.
///
/// A reference is signed only when this person said so in this save: the
/// editor sets `confirm` on a reference they picked from their manager (or
/// confirmed in the connect dialog, via `session_approve_secret_refs`).
/// Besides that, a reference they had already approved in the stored record,
/// in the same place and for the same entry, stays approved: they are the one
/// changing the session, as with the ssh_config approvals, which the editor
/// sends back to be signed for the session as saved.
///
/// Nothing else is signed. In particular a reference that was in the record
/// unapproved (another member of a shared vault may have put it there) does
/// not become approved because the user renamed the session or moved it to a
/// folder, and the `trusted` flag the frontend saw is never read back. What
/// counts as "already approved" is worked out here from the stored record
/// and the user's key, not from anything sent in.
///
/// `via` is the session's via session as stored now (see [`ref_context`]).
/// When it has one and that is gone, nothing is signed and the references'
/// approvals stay as they were: they count for nothing until it is back as
/// it was.
fn seal_for(
    key: Option<&[u8; 32]>,
    who: Option<&str>,
    session: &mut SessionConfig,
    stored: Option<&SessionConfig>,
    via: Option<&SessionConfig>,
) {
    use crate::ssh::sshconf::approvals::{replace_mine, Kind};
    let context = approval_context(session);
    let refs_context = ref_context(session, via);
    if let Some(o) = session.ssh_options.as_mut() {
        o.seal_for(key, who, &context);
    }
    let approved_before: Vec<(String, crate::pwmanager::SecretRef)> = stored
        .map(|s| ref_slots(s).into_iter().filter(|(_, r)| r.trusted).map(|(slot, r)| (slot, r.clone())).collect())
        .unwrap_or_default();
    for (slot, r) in ref_slots_mut(session) {
        let mine = r.confirm || approved_before.iter().any(|(s, before)| *s == slot && before.same_entry(r));
        if let (Some(k), Some(w), Some(c)) = (key, who, refs_context.as_deref()) {
            let what = ref_what(&slot, r);
            let list: &[String] = if mine { std::slice::from_ref(&what) } else { &[] };
            r.approvals = replace_mine(&r.approvals, list, k, w, Kind::SecretRef, c);
        }
        r.confirm = false;
        r.trusted = false;
    }
}

// ---------------------------------------------------------------------------
// Password manager references.
//
// A session can name an entry in the user's password manager for its
// password, its key passphrase, each jump host's and the proxy's. Resolving
// one uses the connecting person's own manager login, so a session in a
// shared vault must not be able to send that person's secrets anywhere they
// did not agree to: another member could point the session at a host they
// own, add a jump host or proxy, or swap the entry for another the victim can
// read. Each reference is therefore approved per person, signed like the
// ssh_config approvals, over its slot, its entry and the session's context
// (`approval_context`); the connect commands use only references the person
// approved, and take them from the stored session, not from the webview.

/// The approved statement for a reference: where in the session it sits and
/// which entry it is.
fn ref_what(slot: &str, r: &crate::pwmanager::SecretRef) -> String {
    format!("{slot}\t{}\t{}", r.provider, r.entry_id)
}

fn auth_ref(m: &AuthMethod) -> Option<(&'static str, &crate::pwmanager::SecretRef)> {
    match m {
        AuthMethod::Password { password_ref: Some(r), .. } => Some(("password", r)),
        AuthMethod::Key { passphrase_ref: Some(r), .. } => Some(("passphrase", r)),
        _ => None,
    }
}

fn auth_ref_mut(m: &mut AuthMethod) -> Option<(&'static str, &mut crate::pwmanager::SecretRef)> {
    match m {
        AuthMethod::Password { password_ref: Some(r), .. } => Some(("password", r)),
        AuthMethod::Key { passphrase_ref: Some(r), .. } => Some(("passphrase", r)),
        _ => None,
    }
}

/// Every reference a session holds, by slot: `password` or `passphrase` for
/// the session's own login (RDP and VNC included), `jump:<n>:password` or
/// `jump:<n>:passphrase` for the n-th jump host (from 0), `proxy` for the
/// proxy's password.
pub(crate) fn ref_slots(s: &SessionConfig) -> Vec<(String, &crate::pwmanager::SecretRef)> {
    let mut out = Vec::new();
    if let Some((field, r)) = auth_ref(&s.auth_method) {
        out.push((field.to_string(), r));
    }
    for (i, j) in s.jump_chain.iter().flatten().enumerate() {
        if let Some((field, r)) = auth_ref(&j.auth_method) {
            out.push((format!("jump:{i}:{field}"), r));
        }
    }
    if let Some(r) = s.proxy.as_ref().and_then(|p| p.password_ref.as_ref()) {
        out.push(("proxy".to_string(), r));
    }
    out
}

fn ref_slots_mut(s: &mut SessionConfig) -> Vec<(String, &mut crate::pwmanager::SecretRef)> {
    let SessionConfig { auth_method, jump_chain, proxy, .. } = s;
    let mut out = Vec::new();
    if let Some((field, r)) = auth_ref_mut(auth_method) {
        out.push((field.to_string(), r));
    }
    for (i, j) in jump_chain.iter_mut().flatten().enumerate() {
        if let Some((field, r)) = auth_ref_mut(&mut j.auth_method) {
            out.push((format!("jump:{i}:{field}"), r));
        }
    }
    if let Some(r) = proxy.as_mut().and_then(|p| p.password_ref.as_mut()) {
        out.push(("proxy".to_string(), r));
    }
    out
}

/// The reference in one slot of a session.
pub(crate) fn ref_at<'a>(s: &'a SessionConfig, slot: &str) -> Option<&'a crate::pwmanager::SecretRef> {
    ref_slots(s).into_iter().find(|(name, _)| name == slot).map(|(_, r)| r)
}

/// The slots an SSH login with this session's own settings resolves: all of
/// them, except the proxy's when there are jump hosts (the proxy is used
/// only without).
pub(crate) fn slots_in_use(s: &SessionConfig) -> Vec<String> {
    let jumps = s.jump_chain.as_ref().is_some_and(|c| !c.is_empty());
    ref_slots(s).into_iter().map(|(slot, _)| slot).filter(|slot| !(jumps && slot == "proxy")).collect()
}

/// What an error from a connect command starts with when the session holds
/// references the connecting person has not approved; JSON
/// ([`UntrustedRefs`]) follows. The session list recognises it and asks.
pub const UNTRUSTED_PREFIX: &str = "PWMANAGER_UNTRUSTED:";

/// One reference waiting for approval.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct RefSlot {
    pub slot: String,
    pub provider: String,
    pub entry_id: String,
}

/// Everything the confirmation dialog shows, read from the stored session
/// in one go, so what the person agrees to is what is signed. `context`
/// goes back with the approval, which is refused if the session changed in
/// between.
#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct UntrustedRefs {
    pub session_id: String,
    pub session_name: String,
    pub context: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    /// `user@host:port` for each jump host, in order.
    pub jumps: Vec<String>,
    /// `type user@host:port` of the proxy, when there is one and no jump host.
    pub proxy: Option<String>,
    /// The session's own ssh_config lines that change where it connects
    /// (`HostName`, `ProxyJump`, `ProxyCommand`), as written: they win over
    /// the host shown above when the login is made.
    pub route_lines: Vec<String>,
    /// The SSH session a VNC session goes through, as stored now: the
    /// secret travels through it.
    pub via: Option<ViaRoute>,
    pub refs: Vec<RefSlot>,
}

/// Where a via session goes, for the confirmation dialog.
#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct ViaRoute {
    pub session_id: String,
    pub session_name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub jumps: Vec<String>,
    pub proxy: Option<String>,
    pub route_lines: Vec<String>,
}

/// The ssh_config keywords that decide where a login goes.
const ROUTE_KEYWORDS: &[&str] = &["hostname", "proxyjump", "proxycommand"];

/// A session's way to its server, for display: `user@host:port` for each
/// jump host, the proxy when it is used (no jump hosts), and its own
/// ssh_config lines that re-route it.
fn route_of(s: &SessionConfig) -> (Vec<String>, Option<String>, Vec<String>) {
    let show = |t: &str| crate::pwmanager::sanitize_display(t, 200);
    let jumps: Vec<String> =
        s.jump_chain.iter().flatten().map(|j| show(&format!("{}@{}:{}", j.username, j.host, j.port))).collect();
    let proxy = s.proxy.as_ref().filter(|_| jumps.is_empty()).map(|p| match p.username.as_deref().filter(|u| !u.is_empty()) {
        Some(u) => show(&format!("{} {u}@{}:{}", p.proxy_type, p.host, p.port)),
        None => show(&format!("{} {}:{}", p.proxy_type, p.host, p.port)),
    });
    let route_lines = s
        .ssh_options
        .iter()
        .flat_map(|o| o.lines.iter())
        .filter_map(|line| crate::ssh::sshconf::lex::split_keyword(line))
        .filter(|l| ROUTE_KEYWORDS.contains(&l.keyword.to_ascii_lowercase().as_str()))
        .map(|l| show(&format!("{} {}", l.keyword, l.rest)))
        .collect();
    (jumps, proxy, route_lines)
}

/// The references among `slots` the reading person has not approved;
/// `Ok(None)` when there are none. `via` is the session's via session as
/// read when the references were checked (see [`ref_context`]); when the
/// session has one and it is gone, there is nothing to ask: the error says
/// so.
pub(crate) fn untrusted_refs(s: &SessionConfig, via: Option<&SessionConfig>, slots: &[String]) -> Result<Option<UntrustedRefs>, String> {
    let refs: Vec<RefSlot> = ref_slots(s)
        .into_iter()
        .filter(|(slot, r)| slots.contains(slot) && !r.trusted)
        .map(|(slot, r)| RefSlot { slot, provider: r.provider.clone(), entry_id: r.entry_id.clone() })
        .collect();
    if refs.is_empty() {
        return Ok(None);
    }
    let Some(context) = ref_context(s, via) else {
        return Err(format!(
            "'{}' goes through an SSH session that no longer exists, so its password manager entry is not used. Choose the SSH session to go through in the session's editor.",
            s.name
        ));
    };
    let (jumps, proxy, route_lines) = route_of(s);
    let via = via_id(s).and(via).map(|v| {
        let (jumps, proxy, route_lines) = route_of(v);
        ViaRoute {
            session_id: v.id.clone(),
            session_name: v.name.clone(),
            host: v.host.clone(),
            port: v.port,
            username: v.username.clone(),
            jumps,
            proxy,
            route_lines,
        }
    });
    Ok(Some(UntrustedRefs {
        session_id: s.id.clone(),
        session_name: s.name.clone(),
        context,
        host: s.host.clone(),
        port: s.port,
        username: s.username.clone(),
        jumps,
        proxy,
        route_lines,
        via,
        refs,
    }))
}

/// Refuse, with an error the session list can act on, unless every
/// reference in `slots` is approved by the connecting person. `via` as for
/// [`untrusted_refs`].
pub(crate) fn require_trusted_refs(s: &SessionConfig, via: Option<&SessionConfig>, slots: &[String]) -> Result<(), String> {
    match untrusted_refs(s, via, slots)? {
        None => Ok(()),
        Some(u) => Err(format!("{UNTRUSTED_PREFIX}{}", serde_json::to_string(&u).map_err(|e| e.to_string())?)),
    }
}

/// A stored session as the connect commands use it: read and opened for the
/// current user, exactly as `session_get` returns it.
pub(crate) async fn stored_session(state: &AppState, session_id: &str) -> Result<SessionConfig, String> {
    stored_session_and_via(state, session_id).await.map(|(s, _)| s)
}

/// [`stored_session`], with the via session its references were checked
/// against (see [`ref_context`]).
pub(crate) async fn stored_session_and_via(
    state: &AppState,
    session_id: &str,
) -> Result<(SessionConfig, Option<SessionConfig>), String> {
    let manager = state.vault_manager.lock().await;
    if manager.is_locked() {
        return Err("Vault is locked".to_string());
    }
    let vault_id = find_session_vault(&manager, session_id)
        .await
        .ok_or_else(|| format!("Session not found: {}", session_id))?;
    let mut session = read_session(&manager, &vault_id, session_id).await?;
    let via = open_approvals(&manager, &mut session).await;
    Ok((session, via))
}

/// A connect that uses password manager entries must say which saved session
/// they are from: there is nothing else to check them against.
pub(crate) fn require_session_id(session_id: Option<&str>) -> Result<&str, String> {
    session_id
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "A password manager entry is used only for a saved session; this connection names none".to_string())
}

/// The stored session an RDP or VNC connect takes its password manager entry
/// from. The connect must go where the session goes (its server, and for VNC
/// its way in), and the person connecting must have approved the entry.
/// Returns the session and, for VNC through an SSH session, that session as
/// the approval was checked against: the tunnel must be built from it as it
/// was then (see `db_commands::forward_route`'s `pin`).
pub(crate) async fn stored_session_for_desktop(
    state: &AppState,
    session_id: Option<&str>,
    kind: crate::state::SessionKind,
    host: &str,
    port: u16,
    username: Option<&str>,
    via_session_id: Option<Option<&str>>,
) -> Result<(SessionConfig, Option<SessionConfig>), String> {
    let (stored, via_session) = stored_session_and_via(state, require_session_id(session_id)?).await?;
    let via = |v: Option<&str>| v.filter(|s| !s.is_empty()).map(str::to_owned);
    let same = stored.kind == kind
        && stored.host == host
        && stored.port == port
        && username.is_none_or(|u| stored.username == u)
        && via_session_id.is_none_or(|v| via(v) == via(stored.via_session_id.as_deref()));
    if !same {
        return Err(format!(
            "This connection does not match the saved session '{}', so its password manager entry is not used. Reload the session list and connect again.",
            stored.name
        ));
    }
    require_trusted_refs(&stored, via_session.as_ref(), &["password".to_string()])?;
    Ok((stored, via_session))
}

/// A desktop session's password manager entry.
pub(crate) fn desktop_ref(s: &SessionConfig) -> Result<&crate::pwmanager::SecretRef, String> {
    ref_at(s, "password").ok_or_else(|| {
        format!("The saved session '{}' has no password manager entry any more. Reload the session list and connect again.", s.name)
    })
}

async fn read_session(manager: &crate::vault::VaultManager, vault_id: &str, session_id: &str) -> Result<SessionConfig, String> {
    let plaintext = manager
        .read_secret(vault_id, session_id)
        .await
        .map_err(|_| format!("Session not found: {}", session_id))?;
    use secrecy::ExposeSecret;
    let json = String::from_utf8(plaintext.expose_secret().clone()).map_err(|e| format!("Invalid UTF-8: {}", e))?;
    serde_json::from_str(&json).map_err(|e| format!("Invalid session data: {}", e))
}

/// The password manager references of a session the current user has not
/// approved yet, among those a connect would use; `None` when there are
/// none. For the session list to ask before it connects.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn session_untrusted_secret_refs(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<Option<UntrustedRefs>, String> {
    let (s, via) = stored_session_and_via(&state, &session_id).await?;
    untrusted_refs(&s, via.as_ref(), &slots_in_use(&s))
}

/// The current user approves these references of a session, after saying so
/// in the confirmation dialog: each is signed with their key for the session
/// as it is. `context` and the entries are what the dialog showed (from
/// [`UntrustedRefs`]); if the session or an entry changed since, nothing is
/// approved.
#[tauri::command]
#[tracing::instrument(skip(state, refs), fields(count = refs.len()))]
pub async fn session_approve_secret_refs(
    state: State<'_, AppState>,
    session_id: String,
    context: String,
    refs: Vec<RefSlot>,
) -> Result<SessionConfig, String> {
    let manager = state.vault_manager.lock().await;
    if manager.is_locked() {
        return Err("Vault is locked".to_string());
    }
    let vault_id = find_session_vault(&manager, &session_id)
        .await
        .ok_or_else(|| format!("Session not found: {}", session_id))?;
    let mut session = read_session(&manager, &vault_id, &session_id).await?;
    // The via session is read once: what is checked against `context` is
    // what is signed.
    let via = via_for(&manager, &session).await;
    open_approvals_with(&manager, &mut session, via.as_ref());
    approve_refs(&mut session, via.as_ref(), &context, &refs)?;
    let stored = session.clone();
    let (key, who) = (manager.approval_key(), manager.get_user_uuid());
    seal_for(key.as_deref(), who.as_deref(), &mut session, Some(&stored), via.as_ref());
    let json = serde_json::to_string(&session).map_err(|e| e.to_string())?;
    manager
        .update_secret(&vault_id, &session.id, SecretBox::new(Box::new(json.into_bytes())))
        .await
        .map_err(|e| e.to_string())?;
    tracing::info!("Approved {} password manager entries for session {}", refs.len(), session.id);
    open_approvals_with(&manager, &mut session, via.as_ref());
    Ok(session)
}

/// Mark `refs` for signing, if the session, and the via session it goes
/// through, still are what the dialog showed.
fn approve_refs(session: &mut SessionConfig, via: Option<&SessionConfig>, context: &str, refs: &[RefSlot]) -> Result<(), String> {
    const CHANGED: &str = "The session changed while you were asked. Connect again to see what it now wants.";
    if refs.is_empty() {
        return Err("Nothing to approve".to_string());
    }
    if ref_context(session, via).as_deref() != Some(context) {
        return Err(CHANGED.to_string());
    }
    for want in refs {
        let found = ref_at(session, &want.slot).is_some_and(|r| r.provider == want.provider && r.entry_id == want.entry_id);
        if !found {
            return Err(CHANGED.to_string());
        }
    }
    for (slot, r) in ref_slots_mut(session) {
        if refs.iter().any(|w| w.slot == slot) {
            r.confirm = true;
        }
    }
    Ok(())
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
    let mut session = read_session(&manager, &vault_id, &session_id).await?;
    open_approvals(&manager, &mut session).await;
    Ok(session)
}

/// Create a new session configuration. O(1) insert.
/// If vault_id is provided and is a user vault, stores in that vault (for sharing).
/// Otherwise stores in __sessions__ (private).
#[tauri::command]
// Only fields that hold no credentials: the login, jump chain and proxy carry
// passwords and would be written to the log by Debug.
#[tracing::instrument(skip_all, fields(name = %name, host = %host, port = port, username = %username, kind = ?kind))]
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

    seal_approvals(&manager, &mut session, None).await;
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
    open_approvals(&manager, &mut session).await;
    Ok(session)
}

/// Update an existing session configuration. O(1) update.
#[tauri::command]
// Not the session itself: its Debug includes passwords.
#[tracing::instrument(skip_all, fields(id = %session.id, name = %session.name, host = %session.host))]
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

    // The record being replaced, as this user reads it: which password
    // manager references they had approved (see `seal_for`).
    let stored = match read_session(&manager, &storage_vault_id, &session.id).await {
        Ok(mut s) => {
            open_approvals(&manager, &mut s).await;
            Some(s)
        }
        Err(_) => None,
    };
    seal_approvals(&manager, &mut session, stored.as_ref()).await;
    let json = serde_json::to_string(&session).map_err(|e| e.to_string())?;
    let plaintext = SecretBox::new(Box::new(json.into_bytes()));

    // O(1) update by primary key
    manager
        .update_secret(&storage_vault_id, &session.id, plaintext)
        .await
        .map_err(|e| e.to_string())?;

    tracing::info!("Updated session: {} in vault: {}", session.id, storage_vault_id);
    open_approvals(&manager, &mut session).await;
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

    let storage_vault_id = ensure_folders_vault(&mut manager).await?;
    if let Some(ref parent_id) = parent_id {
        let parent = read_folders(&manager)
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

    let folders = read_folders(&manager).await?;
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
    let vault_id = get_folders_vault_id_if_exists(&manager)
        .ok_or_else(|| "Folders vault not found".to_string())?;
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

async fn read_folders(manager: &crate::vault::VaultManager) -> Result<Vec<Folder>, String> {
    let vault_id = match get_folders_vault_id_if_exists(manager) {
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
        renamed.auth_method = AuthMethod::Password { password: Some("pw".into()), password_ref: None };
        assert_eq!(mine_after_reading(&renamed, &key), vec!["StrictHostKeyChecking no"]);
    }

    #[test]
    fn a_proxy_password_does_not_cost_the_approvals() {
        let key = [3u8; 32];
        let mut s = session();
        s.proxy = Some(serde_json::from_value(serde_json::json!({ "proxy_type": "socks5", "host": "p.example", "port": 1080 })).unwrap());
        s.ssh_options.as_mut().unwrap().my_accepted_weakenings = Some(vec!["StrictHostKeyChecking no".into()]);
        let context = approval_context(&s);
        s.ssh_options.as_mut().unwrap().seal_for(Some(&key), Some("alice"), &context);

        let mut with_password = s.clone();
        let p = with_password.proxy.as_mut().unwrap();
        p.password = Some("secret".into());
        p.password_ref = Some(entry("e1"));
        assert_eq!(approval_context(&with_password), context);
        assert_eq!(mine_after_reading(&with_password, &key), vec!["StrictHostKeyChecking no"]);

        // Where the proxy is still counts.
        let mut moved = s.clone();
        moved.proxy.as_mut().unwrap().host = "attacker.example".into();
        assert!(mine_after_reading(&moved, &key).is_empty());
    }

    // --- Password manager references -------------------------------------

    const ALICE: [u8; 32] = [7; 32];
    const BOB: [u8; 32] = [8; 32];

    fn entry(id: &str) -> crate::pwmanager::SecretRef {
        crate::pwmanager::SecretRef { provider: "passbolt".into(), entry_id: id.into(), ..Default::default() }
    }

    fn picked(id: &str) -> crate::pwmanager::SecretRef {
        crate::pwmanager::SecretRef { confirm: true, ..entry(id) }
    }

    /// A session with a reference in every slot: its password, a jump host's
    /// password and the proxy's.
    fn with_refs() -> SessionConfig {
        let mut s = session();
        s.auth_method = AuthMethod::Password { password: None, password_ref: Some(entry("e-auth")) };
        s.jump_chain = Some(vec![JumpHostConfig {
            host: "bastion.example".into(),
            port: 22,
            username: "jump".into(),
            auth_method: AuthMethod::Key { path: None, passphrase: None, key_content: None, key_id: Some("k".into()), passphrase_ref: Some(entry("e-jump")) },
        }]);
        s.proxy = Some(serde_json::from_value(serde_json::json!({
            "proxy_type": "http", "host": "proxy.example", "port": 3128, "username": "pu",
        }))
        .unwrap());
        s.proxy.as_mut().unwrap().password_ref = Some(entry("e-proxy"));
        s
    }

    /// Mark every reference as picked in the editor.
    fn pick_all(s: &mut SessionConfig) {
        for (_, r) in ref_slots_mut(s) {
            r.confirm = true;
        }
    }

    /// What `who` sees after the session is stored and read back.
    fn read_as(s: &SessionConfig, key: &[u8; 32], who: &str) -> SessionConfig {
        let stored: SessionConfig = serde_json::from_str(&serde_json::to_string(s).unwrap()).unwrap();
        let mut s = stored;
        open_for(Some(key), Some(who), &mut s, None);
        s
    }

    fn trusted(s: &SessionConfig) -> Vec<(String, bool)> {
        ref_slots(s).into_iter().map(|(slot, r)| (slot, r.trusted)).collect()
    }

    fn only_the_password() -> Vec<(String, bool)> {
        vec![("password".into(), true), ("jump:0:passphrase".into(), false), ("proxy".into(), false)]
    }

    /// Saved by alice with every reference picked.
    fn sealed_by_alice() -> SessionConfig {
        let mut s = with_refs();
        pick_all(&mut s);
        seal_for(Some(&ALICE), Some("alice"), &mut s, None, None);
        s
    }

    #[test]
    fn a_reference_counts_only_for_who_approved_it() {
        let s = sealed_by_alice();
        let slots: Vec<String> = ref_slots(&s).into_iter().map(|(slot, _)| slot).collect();
        assert_eq!(slots, ["password", "jump:0:passphrase", "proxy"]);
        assert!(trusted(&read_as(&s, &ALICE, "alice")).iter().all(|(_, t)| *t));
        // Bob shares the vault: alice's approvals are nothing to him.
        assert!(trusted(&read_as(&s, &BOB, "bob")).iter().all(|(_, t)| !*t));
        // Nor can he write one in her name with his key.
        let mut forged = with_refs();
        pick_all(&mut forged);
        seal_for(Some(&BOB), Some("alice"), &mut forged, None, None);
        assert!(trusted(&read_as(&forged, &ALICE, "alice")).iter().all(|(_, t)| !*t));
        // The flags never go to disk, and confirm never comes back out.
        let json = serde_json::to_string(&s).unwrap();
        assert!(!json.contains("trusted") && !json.contains("confirm"), "{json}");
        let read = serde_json::to_string(&read_as(&s, &ALICE, "alice")).unwrap();
        assert!(read.contains("\"trusted\":true") && !read.contains("confirm"), "{read}");
    }

    #[test]
    fn a_reference_does_not_follow_a_changed_session() {
        let s = sealed_by_alice();
        let all_untrusted = |s: &SessionConfig| trusted(&read_as(s, &ALICE, "alice")).iter().all(|(_, t)| !*t);
        type Change = Box<dyn Fn(&mut SessionConfig)>;
        let changes: Vec<(&str, Change)> = vec![
            ("host", Box::new(|s| s.host = "attacker.example".into())),
            ("port", Box::new(|s| s.port = 2222)),
            ("user", Box::new(|s| s.username = "other".into())),
            ("kind", Box::new(|s| s.kind = crate::state::SessionKind::Rdp)),
            ("via", Box::new(|s| s.via_session_id = Some("evil".into()))),
            ("jump host", Box::new(|s| s.jump_chain.as_mut().unwrap()[0].host = "attacker.example".into())),
            ("jump added", Box::new(|s| {
                let hop = s.jump_chain.as_ref().unwrap()[0].clone();
                s.jump_chain.as_mut().unwrap().push(JumpHostConfig { host: "attacker.example".into(), ..hop });
            })),
            ("proxy host", Box::new(|s| s.proxy.as_mut().unwrap().host = "attacker.example".into())),
            ("proxy port", Box::new(|s| s.proxy.as_mut().unwrap().port = 8080)),
            ("ssh_config", Box::new(|s| s.ssh_options.as_mut().unwrap().lines.push("HostName attacker.example".into()))),
        ];
        for (what, change) in changes {
            let mut changed = s.clone();
            change(&mut changed);
            assert!(all_untrusted(&changed), "{what} kept an approval");
        }

        // Another entry in the same place, or the same entry in another place.
        let mut swapped = s.clone();
        if let AuthMethod::Password { password_ref: Some(r), .. } = &mut swapped.auth_method {
            r.entry_id = "e-victim".into();
        }
        assert_eq!(trusted(&read_as(&swapped, &ALICE, "alice"))[0], ("password".into(), false));
        let mut moved = s.clone();
        let jump_ref = match &moved.jump_chain.as_ref().unwrap()[0].auth_method {
            AuthMethod::Key { passphrase_ref: Some(r), .. } => r.clone(),
            _ => unreachable!(),
        };
        moved.auth_method = AuthMethod::Password { password: None, password_ref: Some(jump_ref) };
        assert_eq!(trusted(&read_as(&moved, &ALICE, "alice"))[0], ("password".into(), false));
        let mut as_passphrase = s.clone();
        let auth_ref = match &as_passphrase.auth_method {
            AuthMethod::Password { password_ref: Some(r), .. } => r.clone(),
            _ => unreachable!(),
        };
        as_passphrase.auth_method = AuthMethod::Key { path: Some("k".into()), passphrase: None, key_content: None, key_id: None, passphrase_ref: Some(auth_ref) };
        assert_eq!(trusted(&read_as(&as_passphrase, &ALICE, "alice"))[0], ("passphrase".into(), false));

        // A name, a folder, tags or the proxy's credentials change none of it.
        let mut renamed = s.clone();
        renamed.name = "web (prod)".into();
        renamed.folder_id = Some("f".into());
        renamed.tags = vec!["prod".into()];
        renamed.proxy.as_mut().unwrap().password = Some("typed".into());
        assert!(trusted(&read_as(&renamed, &ALICE, "alice")).iter().all(|(_, t)| *t));
    }

    #[test]
    fn trusted_from_the_webview_is_ignored() {
        let mut v = serde_json::to_value(with_refs()).unwrap();
        v["auth_method"]["password_ref"]["trusted"] = true.into();
        let s: SessionConfig = serde_json::from_value(v).unwrap();
        let r = ref_at(&s, "password").unwrap();
        assert!(!r.trusted);
        assert!(crate::pwmanager::check_trusted(r).is_err());
        // Nor does it survive a save.
        let mut saved = s.clone();
        seal_for(Some(&ALICE), Some("alice"), &mut saved, None, None);
        assert_eq!(trusted(&read_as(&saved, &ALICE, "alice"))[0], ("password".into(), false));
    }

    #[test]
    fn saving_again_does_not_approve_what_someone_else_put_there() {
        // Mallory, a member of the shared vault, adds her reference.
        let mut s = with_refs();
        pick_all(&mut s);
        seal_for(Some(&BOB), Some("mallory"), &mut s, None, None);

        // Alice opens it in the editor, renames it and saves: the reference
        // goes back as it came, unconfirmed.
        let stored = read_as(&s, &ALICE, "alice");
        let mut edited = stored.clone();
        edited.name = "renamed".into();
        seal_for(Some(&ALICE), Some("alice"), &mut edited, Some(&stored), None);
        assert!(trusted(&read_as(&edited, &ALICE, "alice")).iter().all(|(_, t)| !*t));
        // Mallory's own approvals are left alone.
        assert!(trusted(&read_as(&edited, &BOB, "mallory")).iter().all(|(_, t)| *t));

        // Picking it in the editor is what approves it.
        let mut repicked = read_as(&edited, &ALICE, "alice");
        let before = repicked.clone();
        if let AuthMethod::Password { password_ref: Some(r), .. } = &mut repicked.auth_method {
            r.confirm = true;
        }
        seal_for(Some(&ALICE), Some("alice"), &mut repicked, Some(&before), None);
        assert_eq!(
            trusted(&read_as(&repicked, &ALICE, "alice")),
            only_the_password()
        );
    }

    #[test]
    fn the_owner_editing_keeps_their_approvals() {
        let s = sealed_by_alice();
        // Alice renames it: nothing to confirm again.
        let stored = read_as(&s, &ALICE, "alice");
        let mut renamed = stored.clone();
        renamed.name = "renamed".into();
        seal_for(Some(&ALICE), Some("alice"), &mut renamed, Some(&stored), None);
        assert!(trusted(&read_as(&renamed, &ALICE, "alice")).iter().all(|(_, t)| *t));
        // She moves it to another server herself, in the editor, with the
        // entries in view: they stay hers, for the new server.
        let mut moved = stored.clone();
        moved.host = "new.example".into();
        seal_for(Some(&ALICE), Some("alice"), &mut moved, Some(&stored), None);
        assert!(trusted(&read_as(&moved, &ALICE, "alice")).iter().all(|(_, t)| *t));
        // But a different entry in a slot is not carried over.
        let mut swapped = stored.clone();
        if let AuthMethod::Password { password_ref: Some(r), .. } = &mut swapped.auth_method {
            *r = entry("e-other");
        }
        seal_for(Some(&ALICE), Some("alice"), &mut swapped, Some(&stored), None);
        assert_eq!(trusted(&read_as(&swapped, &ALICE, "alice"))[0], ("password".into(), false));
    }

    #[test]
    fn a_new_session_signs_only_what_was_picked() {
        let mut s = with_refs();
        if let AuthMethod::Password { password_ref: Some(r), .. } = &mut s.auth_method {
            *r = picked("e-auth");
        }
        seal_for(Some(&ALICE), Some("alice"), &mut s, None, None);
        assert_eq!(
            trusted(&read_as(&s, &ALICE, "alice")),
            only_the_password()
        );
    }

    #[test]
    fn untrusted_references_are_refused_with_what_to_ask() {
        let mut s = with_refs();
        s.jump_chain = None;
        open_for(Some(&ALICE), Some("alice"), &mut s, None);
        let err = require_trusted_refs(&s, None, &slots_in_use(&s)).unwrap_err();
        let json = err.strip_prefix(UNTRUSTED_PREFIX).expect(&err);
        let v: serde_json::Value = serde_json::from_str(json).unwrap();
        assert_eq!(v["session_id"], "s1");
        assert_eq!(v["host"], "web.example");
        assert_eq!(v["proxy"], "http pu@proxy.example:3128");
        assert_eq!(v["context"], approval_context(&s));
        assert_eq!(v["refs"][0], serde_json::json!({ "slot": "password", "provider": "passbolt", "entry_id": "e-auth" }));
        assert_eq!(v["refs"][1]["slot"], "proxy");
        // A slot the connect does not use is not asked about.
        assert!(require_trusted_refs(&s, None, &[]).is_ok());
        // The proxy is not used behind jump hosts.
        let with_jump = with_refs();
        assert_eq!(slots_in_use(&with_jump), ["password", "jump:0:passphrase"]);
    }

    #[test]
    fn approving_signs_only_what_the_dialog_showed() {
        let mut s = with_refs();
        open_for(Some(&ALICE), Some("alice"), &mut s, None);
        let asked = untrusted_refs(&s, None, &slots_in_use(&s)).unwrap().unwrap();
        let first = vec![asked.refs[0].clone()];

        // The session changed after the dialog opened: nothing is approved.
        let mut moved = s.clone();
        moved.host = "attacker.example".into();
        assert!(approve_refs(&mut moved, None, &asked.context, &first).is_err());
        let mut swapped = s.clone();
        if let AuthMethod::Password { password_ref: Some(r), .. } = &mut swapped.auth_method {
            r.entry_id = "e-victim".into();
        }
        assert!(approve_refs(&mut swapped, None, &asked.context, &first).is_err());

        // As shown: that one is signed, the others are not.
        let before = s.clone();
        approve_refs(&mut s, None, &asked.context, &first).unwrap();
        seal_for(Some(&ALICE), Some("alice"), &mut s, Some(&before), None);
        assert_eq!(
            trusted(&read_as(&s, &ALICE, "alice")),
            only_the_password()
        );
    }

    #[test]
    fn a_connect_with_references_must_name_its_session() {
        use crate::ipc::ssh_commands::{wanted_slots, JumpHostConnectParams};
        assert!(require_session_id(None).is_err());
        assert!(require_session_id(Some("")).is_err());
        assert_eq!(require_session_id(Some("s1")), Ok("s1"));
        let hop: JumpHostConnectParams = serde_json::from_value(serde_json::json!({
            "host": "b", "port": 22, "username": "j", "authMethod": "key",
            "keyPassphraseRef": { "provider": "passbolt", "entry_id": "e" },
        }))
        .unwrap();
        assert_eq!(wanted_slots("password", true, false, Some(std::slice::from_ref(&hop)), None), ["password", "jump:0:passphrase"]);
        // A reference for a secret the login does not use asks for nothing.
        assert!(wanted_slots("agent", true, true, None, None).is_empty());
    }

    #[test]
    fn a_connect_must_go_where_the_session_goes() {
        use crate::ipc::ssh_commands::check_matches_session;
        let s = with_refs();
        let hops: Vec<crate::ipc::ssh_commands::JumpHostConnectParams> = vec![serde_json::from_value(serde_json::json!({
            "host": "bastion.example", "port": 22, "username": "jump", "authMethod": "key",
        }))
        .unwrap()];
        let proxy = s.proxy.clone();
        assert!(check_matches_session(&s, "web.example", 22, "u", Some(&hops), proxy.as_ref()).is_ok());
        assert!(check_matches_session(&s, "attacker.example", 22, "u", Some(&hops), proxy.as_ref()).is_err());
        assert!(check_matches_session(&s, "web.example", 22, "root", Some(&hops), proxy.as_ref()).is_err());
        assert!(check_matches_session(&s, "web.example", 22, "u", None, proxy.as_ref()).is_err());
        let mut elsewhere = proxy.clone();
        elsewhere.as_mut().unwrap().host = "attacker.example".into();
        assert!(check_matches_session(&s, "web.example", 22, "u", Some(&hops), elsewhere.as_ref()).is_err());
        // The proxy's credentials are not where it goes.
        let mut typed = proxy.clone();
        typed.as_mut().unwrap().password = Some("pw".into());
        typed.as_mut().unwrap().password_ref = None;
        assert!(check_matches_session(&s, "web.example", 22, "u", Some(&hops), typed.as_ref()).is_ok());
    }

    // --- A VNC session through an SSH session ------------------------------

    /// The SSH session a VNC session goes through.
    fn via_ssh() -> SessionConfig {
        let mut s = session();
        s.id = "s-ssh".into();
        s.name = "bastion".into();
        s.host = "bastion.example".into();
        s.username = "jump".into();
        s.ssh_options = None;
        s
    }

    /// A VNC session through [`via_ssh`], with its password from the manager.
    fn vnc() -> SessionConfig {
        serde_json::from_value(serde_json::json!({
            "id": "v1", "name": "desk", "host": "10.0.0.5", "port": 5900, "username": "",
            "auth_method": { "type": "Password", "password_ref": { "provider": "passbolt", "entry_id": "e-vnc" } },
            "folder_id": null, "tags": [], "kind": "vnc", "via_session_id": "s-ssh",
        }))
        .unwrap()
    }

    fn read_via(s: &SessionConfig, via: Option<&SessionConfig>) -> SessionConfig {
        let mut s: SessionConfig = serde_json::from_str(&serde_json::to_string(s).unwrap()).unwrap();
        open_for(Some(&ALICE), Some("alice"), &mut s, via);
        s
    }

    fn vnc_trusted(s: &SessionConfig, via: Option<&SessionConfig>) -> bool {
        ref_at(&read_via(s, via), "password").unwrap().trusted
    }

    /// Picked by alice in the editor, with `via` as it is then.
    fn vnc_sealed(via: &SessionConfig) -> SessionConfig {
        let mut v = vnc();
        pick_all(&mut v);
        seal_for(Some(&ALICE), Some("alice"), &mut v, None, Some(via));
        v
    }

    #[test]
    fn a_vnc_reference_does_not_follow_a_re_pointed_via_session() {
        let s = via_ssh();
        let v = vnc_sealed(&s);
        assert!(vnc_trusted(&v, Some(&s)));
        type Change = Box<dyn Fn(&mut SessionConfig)>;
        let changes: Vec<(&str, Change)> = vec![
            ("host", Box::new(|s| s.host = "attacker.example".into())),
            ("port", Box::new(|s| s.port = 2222)),
            ("user", Box::new(|s| s.username = "other".into())),
            ("jumps", Box::new(|s| {
                s.jump_chain = Some(vec![JumpHostConfig { host: "attacker.example".into(), port: 22, username: "j".into(), auth_method: AuthMethod::Agent }])
            })),
            ("proxy", Box::new(|s| {
                s.proxy = Some(serde_json::from_value(serde_json::json!({ "proxy_type": "socks5", "host": "attacker.example", "port": 1080 })).unwrap())
            })),
            ("ssh_config", Box::new(|s| {
                s.ssh_options = Some(serde_json::from_value(serde_json::json!({ "lines": ["ProxyJump attacker.example"] })).unwrap())
            })),
        ];
        for (what, change) in changes {
            let mut moved = s.clone();
            change(&mut moved);
            assert!(!vnc_trusted(&v, Some(&moved)), "via {what} kept the approval");
        }
        // Renaming the via session, or saving its password, changes nothing.
        let mut renamed = s.clone();
        renamed.name = "bastion (prod)".into();
        renamed.folder_id = Some("f".into());
        renamed.auth_method = AuthMethod::Password { password: Some("pw".into()), password_ref: None };
        assert!(vnc_trusted(&v, Some(&renamed)));
        // Another session than the one named is not it.
        let mut other = s.clone();
        other.id = "s-other".into();
        assert!(!vnc_trusted(&v, Some(&other)));
    }

    #[test]
    fn a_vnc_reference_through_a_deleted_session_counts_for_nothing() {
        let s = via_ssh();
        let v = vnc_sealed(&s);
        assert!(!vnc_trusted(&v, None));
        // Nothing to ask either: there is no way in to show.
        let read = read_via(&v, None);
        assert!(untrusted_refs(&read, None, &["password".into()]).is_err());
        assert!(require_trusted_refs(&read, None, &["password".into()]).is_err());
        // Saving while it is gone signs nothing and keeps what was there: it
        // counts again only once the session is back as it was.
        let mut saved = read.clone();
        pick_all(&mut saved);
        seal_for(Some(&ALICE), Some("alice"), &mut saved, Some(&read), None);
        assert_eq!(ref_at(&saved, "password").unwrap().approvals, ref_at(&v, "password").unwrap().approvals);
        assert!(vnc_trusted(&saved, Some(&s)));
        // A session that is not VNC does not go through another, whatever
        // the field says.
        let mut ssh = v.clone();
        ssh.kind = crate::state::SessionKind::Ssh;
        assert!(ref_context(&ssh, None).is_some());
    }

    #[test]
    fn the_via_session_does_not_change_the_ssh_config_approvals() {
        let s = via_ssh();
        let mut moved = s.clone();
        moved.host = "attacker.example".into();
        let mut v = vnc();
        v.ssh_options = Some(serde_json::from_value(serde_json::json!({ "lines": ["StrictHostKeyChecking no"] })).unwrap());
        v.ssh_options.as_mut().unwrap().my_accepted_weakenings = Some(vec!["StrictHostKeyChecking no".into()]);
        pick_all(&mut v);
        seal_for(Some(&ALICE), Some("alice"), &mut v, None, Some(&s));
        // The ssh_config approvals are signed over the session's own
        // context, whatever its via session is; only the references' gain
        // the via part.
        for via in [Some(&s), Some(&moved), None] {
            let read = read_via(&v, via);
            assert_eq!(read.ssh_options.unwrap().my_accepted_weakenings.unwrap(), ["StrictHostKeyChecking no"]);
        }
        assert!(vnc_trusted(&v, Some(&s)) && !vnc_trusted(&v, Some(&moved)));
        assert_ne!(ref_context(&v, Some(&s)), ref_context(&v, Some(&moved)));
        assert!(ref_context(&v, Some(&s)).unwrap().starts_with(&approval_context(&v)));
    }

    #[test]
    fn approving_a_vnc_reference_checks_the_via_session_too() {
        let s = via_ssh();
        let mut v = read_via(&vnc(), Some(&s));
        let asked = untrusted_refs(&v, Some(&s), &["password".into()]).unwrap().unwrap();
        // The dialog shows the way in.
        let via = asked.via.clone().unwrap();
        assert_eq!((via.session_name.as_str(), via.username.as_str(), via.host.as_str(), via.port), ("bastion", "jump", "bastion.example", 22));
        assert_eq!(asked.context, ref_context(&v, Some(&s)).unwrap());

        // The via session is re-pointed while the dialog is open: refused.
        let mut moved = s.clone();
        moved.host = "attacker.example".into();
        let mut changed = v.clone();
        assert!(approve_refs(&mut changed, Some(&moved), &asked.context, &asked.refs).is_err());
        let mut gone = v.clone();
        assert!(approve_refs(&mut gone, None, &asked.context, &asked.refs).is_err());

        // As shown: approved, for that way in only.
        let before = v.clone();
        approve_refs(&mut v, Some(&s), &asked.context, &asked.refs).unwrap();
        seal_for(Some(&ALICE), Some("alice"), &mut v, Some(&before), Some(&s));
        assert!(vnc_trusted(&v, Some(&s)));
        assert!(!vnc_trusted(&v, Some(&moved)));
    }

    #[test]
    fn the_dialog_shows_where_ssh_config_sends_the_login() {
        let mut s = with_refs();
        s.jump_chain = None;
        s.ssh_options = Some(serde_json::from_value(serde_json::json!({
            "lines": ["HostName real.example", "ProxyJump hop.example", "Compression yes", "proxycommand nc %h %p"],
        }))
        .unwrap());
        open_for(Some(&ALICE), Some("alice"), &mut s, None);
        let asked = untrusted_refs(&s, None, &slots_in_use(&s)).unwrap().unwrap();
        assert_eq!(asked.route_lines, ["HostName real.example", "ProxyJump hop.example", "proxycommand nc %h %p"]);
        assert!(asked.via.is_none());
        // And the via session's, for a VNC session.
        let mut via = via_ssh();
        via.ssh_options = Some(serde_json::from_value(serde_json::json!({ "lines": ["HostName inner.example"] })).unwrap());
        let v = read_via(&vnc(), Some(&via));
        let asked = untrusted_refs(&v, Some(&via), &["password".into()]).unwrap().unwrap();
        assert_eq!(asked.via.unwrap().route_lines, ["HostName inner.example"]);
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
