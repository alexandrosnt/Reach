use crate::state::AppState;
use crate::ssh::client::{AuthParams, ConnectionInfo, JumpHostParams, KeySource, SshManager, exec_on_connection};
use crate::plugin::hooks;

/// Parameters for a jump host received from the frontend.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JumpHostConnectParams {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_method: String,
    pub password: Option<String>,
    pub key_path: Option<String>,
    pub key_passphrase: Option<String>,
    /// An imported key, instead of a path. See `ssh_key_import`.
    pub key_id: Option<String>,
    /// Use the saved session's password manager entry for this hop's
    /// password; wins over `password`. Only a signal: the entry itself is
    /// read from the stored session (see `ssh_connect`). Sent as
    /// `passwordRef` (`password_ref` is accepted too).
    #[serde(default, alias = "password_ref")]
    pub password_ref: Option<crate::pwmanager::SecretRef>,
    /// The same for the key passphrase; wins over `key_passphrase`. Sent as
    /// `keyPassphraseRef` (or `key_passphrase_ref`).
    #[serde(default, alias = "key_passphrase_ref")]
    pub key_passphrase_ref: Option<crate::pwmanager::SecretRef>,
}

/// Which secret of a login a password manager reference stands in for, if
/// the login method uses one: the session slot's field name (see
/// `session_commands::ref_slots`).
fn ref_field(auth_method: &str, has_password_ref: bool, has_passphrase_ref: bool) -> Option<&'static str> {
    match auth_method {
        "password" if has_password_ref => Some("password"),
        "key" if has_passphrase_ref => Some("passphrase"),
        _ => None,
    }
}

/// The slots of the saved session whose password manager entries this
/// connect asks for. The references the webview sends say only that:
/// which entry is used, and where it goes, is read from the stored session.
pub(crate) fn wanted_slots(
    auth_method: &str,
    has_password_ref: bool,
    has_passphrase_ref: bool,
    jump_chain: Option<&[JumpHostConnectParams]>,
    proxy: Option<&crate::state::ProxyConfig>,
) -> Vec<String> {
    let mut out: Vec<String> = ref_field(auth_method, has_password_ref, has_passphrase_ref).map(String::from).into_iter().collect();
    let jumps = jump_chain.unwrap_or_default();
    for (i, j) in jumps.iter().enumerate() {
        if let Some(field) = ref_field(&j.auth_method, j.password_ref.is_some(), j.key_passphrase_ref.is_some()) {
            out.push(format!("jump:{i}:{field}"));
        }
    }
    // The proxy is used only without jump hosts.
    if jumps.is_empty() && proxy.is_some_and(|p| p.password_ref.is_some()) {
        out.push("proxy".to_string());
    }
    out
}

/// Where a proxy goes, without its credentials.
fn proxy_endpoint(p: &crate::state::ProxyConfig) -> (&str, &str, u16, Option<&str>) {
    (p.proxy_type.as_str(), p.host.as_str(), p.port, p.username.as_deref().filter(|u| !u.is_empty()))
}

/// The connect must go where the saved session goes for its password
/// manager entries to be sent: the same server, user, jump hosts and proxy.
/// Their approvals were given for exactly that (`approval_context`).
pub(crate) fn check_matches_session(
    stored: &crate::state::SessionConfig,
    host: &str,
    port: u16,
    username: &str,
    jump_chain: Option<&[JumpHostConnectParams]>,
    proxy: Option<&crate::state::ProxyConfig>,
) -> Result<(), String> {
    let jumps: Vec<(&str, u16, &str)> =
        jump_chain.unwrap_or_default().iter().map(|j| (j.host.as_str(), j.port, j.username.as_str())).collect();
    let stored_jumps: Vec<(&str, u16, &str)> =
        stored.jump_chain.iter().flatten().map(|j| (j.host.as_str(), j.port, j.username.as_str())).collect();
    let same = stored.kind == crate::state::SessionKind::Ssh
        && stored.host == host
        && stored.port == port
        && stored.username == username
        && jumps == stored_jumps
        && proxy.map(proxy_endpoint) == stored.proxy.as_ref().map(proxy_endpoint);
    if same {
        Ok(())
    } else {
        Err(format!(
            "This connection does not match the saved session '{}', so its password manager entries are not used. Reload the session list and connect again.",
            stored.name
        ))
    }
}

/// The reference in one slot of the stored session.
fn stored_ref<'a>(stored: &'a crate::state::SessionConfig, slot: &str) -> Result<&'a crate::pwmanager::SecretRef, String> {
    crate::ipc::session_commands::ref_at(stored, slot).ok_or_else(|| {
        format!("The saved session '{}' has no password manager entry for this login any more. Reload the session list and connect again.", stored.name)
    })
}

/// What a login may try, and nothing else. A key session offers its key, a
/// password session its password, an agent session the agent's keys. The
/// agent's keys are offered to a key session only when the user turned that
/// on for it (`try_agent_keys`).
///
/// Every refused offer counts against the server: OpenSSH's MaxAuthTries
/// (6) and PerSourcePenalties, fail2ban and firewalls block the address
/// after a few, and every key offered tells the server who you are. Offering
/// all the agent's keys behind a refused session key was guessing, and it
/// also hid a session whose own key no longer worked (it then failed on a
/// phone, which has no agent).
pub(crate) fn build_auth(
    auth_method: &str,
    password: Option<String>,
    key_source: Option<KeySource>,
    key_passphrase: Option<String>,
    try_agent_keys: bool,
) -> Result<AuthParams, String> {
    let mut auth = AuthParams { allow_agent: auth_method == "agent", ..Default::default() };
    match auth_method {
        "password" => {
            auth.password = Some(password.ok_or("Password required for password auth")?);
        }
        "key" => {
            auth.key = Some(crate::ssh::client::KeyAuth {
                source: key_source
                    .ok_or("Key auth needs either a key file or an imported key")?,
                passphrase: key_passphrase,
            });
            auth.allow_agent = try_agent_keys;
        }
        "agent" => {}
        _ => return Err(format!("Unknown auth method: {}", auth_method)),
    }
    Ok(auth)
}

/// Work out where a key comes from. An imported key wins over a path: it is
/// the one the user picked most recently, and it is the one that works on a
/// machine with no key files.
pub(crate) async fn resolve_key_source(
    state: &AppState,
    key_id: Option<String>,
    key_path: Option<String>,
) -> Result<(Option<KeySource>, Option<String>), String> {
    if let Some(id) = key_id.filter(|i| !i.is_empty()) {
        let resolved = crate::ipc::sshkey_commands::resolve(state, &id).await?;
        return Ok((
            Some(KeySource::Material(resolved.private_key)),
            resolved.passphrase,
        ));
    }
    Ok((
        key_path
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty())
            .map(KeySource::Path),
        None,
    ))
}

#[tauri::command]
#[expect(clippy::too_many_arguments, reason = "a Tauri command takes each argument from the frontend's invoke by name")]
pub async fn ssh_connect(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    id: String,
    host: String,
    port: u16,
    username: String,
    auth_method: String,
    password: Option<String>,
    key_path: Option<String>,
    key_passphrase: Option<String>,
    key_id: Option<String>,
    cols: u16,
    rows: u16,
    jump_chain: Option<Vec<JumpHostConnectParams>>,
    proxy: Option<crate::state::ProxyConfig>,
    shell: Option<String>,
    inject_colors: Option<bool>,
    show_login_message: Option<bool>,
    session_log: Option<crate::ssh::session_log::SessionLogConfig>,
    try_agent_keys: Option<bool>,
    ssh_options: Option<crate::ssh::sshconf::session::SshOptions>,
    password_ref: Option<crate::pwmanager::SecretRef>,
    key_passphrase_ref: Option<crate::pwmanager::SecretRef>,
    session_id: Option<String>,
) -> Result<String, String> {
    // Only the approvals of the person connecting count: a session synced
    // from a vault others can write to cannot approve its own commands.
    let mut ssh_options = ssh_options.map(|o| o.effective());
    // Nothing is typed into the user's shell unless the caller asks for it
    // (Settings → Appearance): a caller that does not say gets no setup. The
    // login message is shown unless asked otherwise.
    let login = crate::ssh::client::LoginOptions {
        inject_colors: inject_colors.unwrap_or(false),
        show_login_message: show_login_message.unwrap_or(true),
        log: session_log,
    };
    tracing::info!(
        "ssh_connect IPC: id={}, host={}, port={}, user={}, auth_method='{}', has_key_path={}, has_password={}, has_passphrase={}, has_password_ref={}, has_passphrase_ref={}, has_proxy={}, has_jump={}",
        id, host, port, username, auth_method,
        key_path.as_deref().map(|p| !p.is_empty()).unwrap_or(false),
        password.as_deref().map(|p| !p.is_empty()).unwrap_or(false),
        key_passphrase.as_deref().map(|p| !p.is_empty()).unwrap_or(false),
        password_ref.is_some(),
        key_passphrase_ref.is_some(),
        proxy.is_some(),
        jump_chain.as_ref().map(|c| c.len()).unwrap_or(0),
    );
    // Secrets held by a password manager are fetched now, for the target,
    // every jump host and the proxy, before anything connects. The proxy is
    // used only without a jump chain.
    //
    // Which entries, and where they may go, come from the saved session as
    // stored, not from this call: the references sent here only say "use
    // the session's entry". The connect must match the session, and each
    // entry must be one the connecting person approved for it; otherwise
    // the error says which, for the session list to ask (see
    // `session_commands::require_trusted_refs`).
    let mut password = password;
    let mut key_passphrase = key_passphrase;
    let mut jump_chain = jump_chain;
    let mut proxy = proxy;
    let wanted = wanted_slots(&auth_method, password_ref.is_some(), key_passphrase_ref.is_some(), jump_chain.as_deref(), proxy.as_ref());
    if !wanted.is_empty() {
        let sid = crate::ipc::session_commands::require_session_id(session_id.as_deref())?;
        let stored = crate::ipc::session_commands::stored_session(&state, sid).await?;
        check_matches_session(&stored, &host, port, &username, jump_chain.as_deref(), proxy.as_ref())?;
        crate::ipc::session_commands::require_trusted_refs(&stored, None, &wanted)?;
        // The ssh_config settings decide where the login really goes
        // (HostName, ProxyJump): the stored session's, which the approvals
        // were given for.
        ssh_options = stored.ssh_options.clone().map(|o| o.effective());
        // One operation: if the manager's passphrase must be asked for, it
        // is asked once for all of these, not once per entry.
        let reason = crate::pwmanager::Reason::Connect(stored.name.clone());
        crate::pwmanager::with_prompt(&app, reason, async {
            if let Some(field) = ref_field(&auth_method, password_ref.is_some(), key_passphrase_ref.is_some()) {
                let secret = crate::pwmanager::resolve_to_string(&state, stored_ref(&stored, field)?).await?;
                if field == "password" { password = Some(secret) } else { key_passphrase = Some(secret) }
            }
            for (i, j) in jump_chain.iter_mut().flatten().enumerate() {
                if let Some(field) = ref_field(&j.auth_method, j.password_ref.is_some(), j.key_passphrase_ref.is_some()) {
                    let secret = crate::pwmanager::resolve_to_string(&state, stored_ref(&stored, &format!("jump:{i}:{field}"))?).await?;
                    if field == "password" { j.password = Some(secret) } else { j.key_passphrase = Some(secret) }
                }
            }
            if wanted.iter().any(|s| s == "proxy") {
                if let Some(p) = proxy.as_mut() {
                    p.password = Some(crate::pwmanager::resolve_to_string(&state, stored_ref(&stored, "proxy")?).await?);
                }
            }
            Ok::<(), String>(())
        })
        .await?;
    }
    if let Some(s) = shell.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        tracing::info!("ssh_connect: per-session login shell override = '{}'", s);
    }
    // A key stored in the vault brings its own passphrase; one typed into the
    // connect dialog still wins, so a saved passphrase that went stale can be
    // corrected without editing the key.
    let (key_source, stored_passphrase) =
        resolve_key_source(&state, key_id, key_path).await?;
    let key_passphrase = key_passphrase.filter(|p| !p.is_empty()).or(stored_passphrase);
    let auth = build_auth(&auth_method, password, key_source, key_passphrase, try_agent_keys.unwrap_or(false))?;

    // Establish the connection WITHOUT holding the global ssh_manager lock. The
    // handshake/auth/shell setup can take up to the connect timeout (longer if a
    // host hangs); holding the lock across it would block ssh_send / ssh_resize /
    // ssh_disconnect on every other live connection. We lock only afterwards,
    // briefly, to register the finished connection (a single HashMap insert).
    // ssh_config settings for the target: resolved here, before connecting.
    let mut target_opts: crate::ssh::client::HopOptions =
        crate::ssh::sshconf::session::plan_for(ssh_options.as_ref(), &host, port, &username, false).into();
    // LogLevel/LogVerbose: everything logged while connecting is collected
    // for the terminal, and shown with the error if the connection fails.
    let connlog = target_opts.log.as_ref().map(|(level, verbose)| crate::ssh::connlog::ConnLog::new(level.as_deref(), verbose));
    target_opts.connlog = connlog.clone();
    let span = connlog.as_ref().map_or_else(tracing::Span::none, |l| l.span());
    let connected: Result<_, String> = tracing::Instrument::instrument(async {
    let conn = if let Some(chain) = jump_chain {
        if chain.is_empty() {
            // No jump hosts, connect directly
            SshManager::connect(&id, &host, port, &username, auth, cols, rows, app.clone(), proxy, shell, login, target_opts)
                .await
                .map_err(|e| e.to_string())?
        } else {
            // Resolve every jump host's key before the closure below, which
            // cannot await.
            let mut jump_key_sources: Vec<Option<(Option<KeySource>, Option<String>)>> =
                Vec::with_capacity(chain.len());
            for j in &chain {
                jump_key_sources.push(Some(
                    resolve_key_source(&state, j.key_id.clone(), j.key_path.clone()).await?,
                ));
            }

            // Build jump host params
            let jump_params: Result<Vec<JumpHostParams>, String> = chain
                .into_iter()
                .map(|j| {
                    let (jsource, jstored) = jump_key_sources
                        .remove(0)
                        .ok_or_else(|| "Jump host key could not be resolved".to_string())?;
                    let jauth = build_auth(
                        &j.auth_method,
                        j.password,
                        jsource,
                        j.key_passphrase.filter(|p| !p.is_empty()).or(jstored),
                        false,
                    )?;
                    let opts = crate::ssh::sshconf::session::plan_for(ssh_options.as_ref(), &j.host, j.port, &j.username, true).into();
                    Ok(JumpHostParams {
                        host: j.host,
                        port: j.port,
                        username: j.username,
                        auth: jauth,
                        opts,
                    })
                })
                .collect();

            SshManager::connect_via_jump(
                &id,
                &host,
                port,
                &username,
                auth,
                jump_params?,
                cols,
                rows,
                app.clone(),
                shell,
                login,
                target_opts,
            )
            .await
            .map_err(|e| e.to_string())?
        }
    } else {
        SshManager::connect(&id, &host, port, &username, auth, cols, rows, app.clone(), proxy, shell, login, target_opts)
            .await
            .map_err(|e| e.to_string())?
    };
    Ok(conn)
    }, span).await;
    let conn = match connected {
        Ok(c) => c,
        Err(e) => {
            let Some(cl) = connlog else { return Err(e) };
            crate::ssh::connlog::close(&cl);
            let lines = cl.take();
            return Err(if lines.is_empty() { e } else { format!("{e}\n\n{}", lines.join("\n")) });
        }
    };

    // Register the finished connection under a brief lock, released immediately
    // (before plugin hooks, which may themselves need the lock).
    let info = state.ssh_manager.lock().await.register(conn);

    let connection_id = info.id.clone();

    // Fire-and-forget: a slow or hung plugin hook must not block the IPC
    // return. dispatch_hook applies a per-hook timeout internally.
    let hook = hooks::session_connected(&connection_id, &host, &username);
    let plugin_mgr = state.plugin_manager.clone();
    let app_for_hook = app.clone();
    tokio::spawn(async move {
        let mut mgr = plugin_mgr.lock().await;
        mgr.dispatch_hook(&hook, Some(&app_for_hook)).await;
    });

    Ok(connection_id)
}

#[tauri::command]
pub async fn ssh_send(
    state: tauri::State<'_, AppState>,
    connection_id: String,
    data: Vec<u8>,
) -> Result<(), String> {
    let manager = state.ssh_manager.lock().await;
    manager.send_data(&connection_id, &data).map_err(|e| e.to_string())
}

/// Called by the terminal once its data listener is attached: flushes any
/// buffered remote output (motd/banner) and switches to live streaming.
#[tauri::command]
pub async fn ssh_ready(
    state: tauri::State<'_, AppState>,
    connection_id: String,
) -> Result<(), String> {
    let manager = state.ssh_manager.lock().await;
    manager.mark_ready(&connection_id).map_err(|e| e.to_string())
}

/// Where session logs go when Settings leaves the folder empty, shown there
/// as the placeholder.
#[tauri::command]
pub fn session_log_default_dir() -> String {
    crate::ssh::session_log::default_dir().display().to_string()
}

/// The frontend's host-key verification dialog reports the user's decision,
/// un-parking the SSH handshake that's waiting on `check_server_key`.
#[tauri::command]
pub fn ssh_hostkey_response(prompt_id: String, accept: bool) {
    crate::ssh::client::resolve_hostkey_prompt(&prompt_id, accept);
}

/// The answer to a login question (password, passphrase, keyboard-interactive,
/// yes/no); `None` when the user cancelled.
#[tauri::command]
pub fn ssh_auth_prompt_response(prompt_id: String, answers: Option<Vec<String>>) {
    crate::ssh::prompt::resolve(&prompt_id, answers);
}

#[tauri::command]
pub async fn ssh_resize(
    state: tauri::State<'_, AppState>,
    connection_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    let manager = state.ssh_manager.lock().await;
    manager.resize(&connection_id, cols, rows).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn ssh_disconnect(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    connection_id: String,
) -> Result<(), String> {
    let mut manager = state.ssh_manager.lock().await;
    manager.disconnect(&connection_id).map_err(|e| e.to_string())?;
    drop(manager);

    // Fire-and-forget hook dispatch (see ssh_connect for rationale).
    let hook = hooks::session_disconnected(&connection_id);
    let plugin_mgr = state.plugin_manager.clone();
    let app_for_hook = app.clone();
    tokio::spawn(async move {
        let mut mgr = plugin_mgr.lock().await;
        mgr.dispatch_hook(&hook, Some(&app_for_hook)).await;
    });

    Ok(())
}

#[tauri::command]
pub async fn ssh_list_connections(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ConnectionInfo>, String> {
    let manager = state.ssh_manager.lock().await;
    Ok(manager.list_connections())
}

/// Detect the remote operating system by parsing /etc/os-release.
/// Returns a lowercase distro ID (e.g. "debian", "ubuntu", "alpine")
/// or a fallback from uname -s (e.g. "darwin", "freebsd").
#[tauri::command]
pub async fn ssh_detect_os(
    state: tauri::State<'_, AppState>,
    connection_id: String,
) -> Result<String, String> {
    let handle = {
        let manager = state.ssh_manager.lock().await;
        manager.get_handle(&connection_id).map_err(|e| e.to_string())?
    };

    // Try /etc/os-release first (standard on modern Linux)
    if let Ok(output) = exec_on_connection(&handle, "cat /etc/os-release 2>/dev/null").await {
        for line in output.lines() {
            // Match the ID= line (not ID_LIKE=)
            if let Some(rest) = line.strip_prefix("ID=") {
                let id = rest.trim().trim_matches('"').to_lowercase();
                if !id.is_empty() {
                    return Ok(id);
                }
            }
        }
    }

    // Fallback: uname -s for non-Linux systems
    if let Ok(output) = exec_on_connection(&handle, "uname -s 2>/dev/null").await {
        let os = output.trim().to_lowercase();
        if !os.is_empty() {
            return Ok(os);
        }
    }

    Ok("linux".to_string())
}

/// Inspect a key-file path so the UI can warn when the user picked the wrong
/// file (e.g. an OpenSSH public key) and suggest the right private key from the
/// same folder. Pure filesystem read — no connection or state required.
#[tauri::command]
pub fn inspect_key_file(path: String) -> crate::ssh::keyfile::KeyFileInfo {
    crate::ssh::keyfile::classify_path(&path)
}

#[cfg(test)]
mod auth_choice_tests {
    use super::*;

    fn key() -> Option<KeySource> {
        Some(KeySource::Material("k".into()))
    }

    /// Each kind of session offers what it names and nothing else, as PuTTY
    /// (userauth2-client.c filters Pageant down to the configured key) and
    /// OpenSSH with IdentitiesOnly do. No other key, no password on the side.
    #[test]
    fn a_session_offers_only_what_it_names() {
        let k = build_auth("key", Some("pw".into()), key(), None, false).unwrap();
        assert!(k.key.is_some() && !k.allow_agent && k.password.is_none());

        let p = build_auth("password", Some("pw".into()), None, None, true).unwrap();
        assert!(p.password.is_some() && !p.allow_agent && p.key.is_none());

        let a = build_auth("agent", None, None, None, false).unwrap();
        assert!(a.allow_agent && a.key.is_none() && a.password.is_none());
    }

    #[test]
    fn the_agent_backs_a_key_only_when_the_session_asks() {
        assert!(build_auth("key", None, key(), None, true).unwrap().allow_agent);
    }
}
