//! IPC for Settings → Password managers and the entry picker.
//!
//! None of these returns a secret. The configuration comes back with
//! `has_private_key` and the like in place of the values; listing returns
//! names, usernames and URLs; the secrets an entry holds are fetched only by
//! the connect commands, in the backend (see [`crate::pwmanager::resolve`]).
//! A passphrase typed into the window's passphrase question goes one way
//! only, in (`pwmanager_passphrase_response`).
//!
//! Reading the configuration and the status works while the feature is off,
//! so Settings can show it; listing and testing check the master switch.
//! Reading the configuration needs the vault open: with it locked there is
//! nothing true to show, and defaults shown in its place could be saved
//! back over the real settings.

use tauri::State;

use crate::pwmanager::config::{self, PasswordManagersConfigInput, PasswordManagersView};
use crate::pwmanager::{self, EntrySummary, ProviderId, ProviderStatus, Reason, TestResult};
use crate::state::AppState;

fn view(loaded: &config::Loaded) -> PasswordManagersView {
    let mut view = PasswordManagersView::from(loaded);
    view.passbolt.passphrase_cached = pwmanager::passphrase_cached(loaded);
    view
}

/// The configuration, without secrets. With the vault locked: an error
/// ("Unlock Reach first"), not defaults that Settings could save back.
#[tauri::command]
pub async fn pwmanager_get_config(state: State<'_, AppState>) -> Result<PasswordManagersView, String> {
    let loaded = config::load(&state).await?;
    Ok(view(&loaded))
}

/// Check and store the configuration. A secret left out keeps the stored one;
/// the CLI path is stored on this device only. The cached entry lists are
/// dropped (and any listing still running will not be cached): they may
/// belong to another account. So is a passphrase kept in memory, and any
/// passphrase question still open is cancelled.
#[tauri::command]
pub async fn pwmanager_save_config(
    state: State<'_, AppState>,
    mut config: PasswordManagersConfigInput,
) -> Result<PasswordManagersView, String> {
    let current_cli_path = config::device_cli_path();
    let save = config::validate_input(&mut config, current_cli_path.as_deref())?;
    drop(config);
    config::save(&state, &save).await?;
    drop(save);
    pwmanager::clear_cache();
    pwmanager::forget_passphrase();
    let loaded = config::load(&state).await?;
    Ok(view(&loaded))
}

/// Each manager's status: whether its tool is installed, and its version.
/// No login.
#[tauri::command]
pub async fn pwmanager_status(state: State<'_, AppState>) -> Result<Vec<ProviderStatus>, String> {
    Ok(pwmanager::status_all(&state).await)
}

/// Log in and list entries, to prove the settings work. May ask for the
/// passphrase.
#[tauri::command]
pub async fn pwmanager_test(app: tauri::AppHandle, state: State<'_, AppState>, provider: String) -> Result<TestResult, String> {
    // Boxed: Tauri builds a command's future on the main thread and passes it
    // by value before spawning it, and the main thread's stack is small
    // (1 MB on Windows). Only a pointer is left there this way.
    Box::pin(async move {
        let id = ProviderId::parse(&provider)?;
        pwmanager::with_prompt(&app, Reason::Test, pwmanager::test(&state, id)).await
    })
    .await
}

/// Entries for the picker: metadata only, filtered by `query`, at most 500.
/// Served from a five-minute cache unless `refresh`. May ask for the
/// passphrase.
#[tauri::command]
pub async fn pwmanager_list(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    provider: String,
    query: Option<String>,
    refresh: Option<bool>,
) -> Result<Vec<EntrySummary>, String> {
    // Boxed: Tauri builds a command's future on the main thread and passes it
    // by value before spawning it, and the main thread's stack is small
    // (1 MB on Windows). Only a pointer is left there this way.
    Box::pin(async move {
        let id = ProviderId::parse(&provider)?;
        pwmanager::with_prompt(&app, Reason::List, pwmanager::list(&state, id, query.as_deref(), refresh.unwrap_or(false)))
            .await
    })
    .await
}

/// The window's answer to a `pwmanager-passphrase-prompt` question: the
/// passphrase, or `None` for Cancel. Refused (and the question left open)
/// if Reach cannot take it, so the window can say why.
#[tauri::command]
pub fn pwmanager_passphrase_response(prompt_id: String, passphrase: Option<String>) -> Result<(), String> {
    pwmanager::answer_prompt(&prompt_id, passphrase)
}

/// Settings → "Forget now": drop any passphrase kept in memory, and cancel
/// any passphrase question still open.
#[tauri::command]
pub fn pwmanager_forget_passphrase() {
    pwmanager::forget_passphrase();
    tracing::info!("pwmanager: kept passphrase forgotten on request");
}
