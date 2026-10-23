use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use secrecy::SecretBox;
use tauri::State;

use crate::state::AppState;
use crate::vault::manager::INTERNAL_VAULTS;
use crate::vault::{biometric, fido2};
use crate::vault::{
    AppSettings, InviteInfo, MemberInfo, MemberRole, ReceivedShare, SecretCategory, SecretMetadata,
    ShareItemResult, SharedItemInfo, VaultInfo, VaultType,
};

// ==================== REACH'S OWN VAULTS ====================

/// The vault the internal vaults map onto while personal sync is on.
const UNIFIED_VAULT: &str = "__personal__";

/// Whether `vault_id` is one of the vaults Reach keeps for itself (sessions,
/// settings with the password-manager secrets, SSH keys, …), with `id_of`
/// mapping a vault name to its id. The secret commands below are for the
/// user's own vaults, which the vault panel lists; Reach's vaults are read
/// and written only through the commands made for them, which check what
/// they store (the settings commands refuse the password-manager keys, for
/// one). Their ids are random and never listed, but a guessed or leaked id
/// must not open them either.
fn is_internal_vault_id(vault_id: &str, id_of: impl Fn(&str) -> Option<String>) -> bool {
    INTERNAL_VAULTS
        .iter()
        .chain(std::iter::once(&UNIFIED_VAULT))
        .any(|name| id_of(name).as_deref() == Some(vault_id))
}

fn refuse_internal_vault(manager: &crate::vault::VaultManager, vault_id: &str) -> Result<(), String> {
    if is_internal_vault_id(vault_id, |name| manager.get_vault_id_by_name(name)) {
        Err("This vault is kept by Reach for its own data and cannot be opened here".into())
    } else {
        Ok(())
    }
}

// ==================== IDENTITY ====================

#[tauri::command]
#[tracing::instrument(skip(password, state))]
pub async fn vault_init_identity(
    password: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let mut manager = state.vault_manager.lock().await;
    // A new identity: entry lists cached for the old one must go, and a
    // passphrase typed for the old one.
    crate::pwmanager::forget_all();
    manager.init_identity(&password).await.map_err(|e| e.to_string())
}

#[tauri::command]
#[tracing::instrument(skip(password, state))]
pub async fn vault_unlock(
    password: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let mut manager = state.vault_manager.lock().await;
    manager.unlock(&password).await.map_err(|e| e.to_string())
}

#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn vault_lock(state: State<'_, AppState>) -> Result<(), String> {
    let mut manager = state.vault_manager.lock().await;
    manager.hold();
    // The password-manager settings live in the vault: what was listed with
    // them goes when it locks, and so does a passphrase kept in memory (and
    // any passphrase question still open is cancelled).
    crate::pwmanager::forget_all();
    Ok(())
}

/// Whether the vault is being kept locked until the user opens it again.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn vault_is_held(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(state.vault_manager.lock().await.is_held())
}

/// Open a held vault with the keychain, as the user's own act.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn vault_resume(state: State<'_, AppState>) -> Result<bool, String> {
    let mut manager = state.vault_manager.lock().await;
    manager.resume().await.map_err(|e| e.to_string())
}

/// The device unlock methods: the platform biometric (Windows Hello, Touch ID,
/// Android's fingerprint) and security keys, and which of them are on.
#[derive(serde::Serialize)]
pub struct UnlockMethods {
    /// The platform biometric this build offers, if any.
    platform: Option<&'static str>,
    platform_available: bool,
    keys_supported: bool,
    /// Reach asks for the key's PIN itself (macOS, Linux, Android); Windows asks.
    keys_ask_pin: bool,
    methods: Vec<biometric::SealInfo>,
}

/// What Android's fingerprint prompt says, in the user's language. The
/// desktop prompts are the system's own and ignore it.
#[derive(serde::Deserialize, serde::Serialize)]
pub struct PromptText {
    title: String,
    #[serde(default)]
    subtitle: Option<String>,
    cancel: String,
}

impl Default for PromptText {
    fn default() -> Self {
        PromptText { title: "Reach".into(), subtitle: None, cancel: "Cancel".into() }
    }
}

/// Where each device method's key comes from. On desktop, Reach's own code on
/// a blocking thread; on Android, the Kotlin plugin (tauri-plugin-reach-unlock),
/// which waits on the fingerprint or the key without holding a thread here.
mod device {
    use super::PromptText;
    #[cfg(not(target_os = "android"))]
    use crate::vault::biometric::{Seal, Unlockers};
    #[cfg(not(target_os = "android"))]
    use tauri::AppHandle;
    #[cfg(not(target_os = "android"))]
    use zeroize::Zeroizing;

    #[cfg(not(target_os = "android"))]
    async fn blocking<T: Send + 'static>(work: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
        tokio::task::spawn_blocking(work).await.map_err(|e| e.to_string())?
    }

    #[cfg(not(target_os = "android"))]
    pub async fn platform_available(_app: &AppHandle) -> bool {
        blocking(|| Ok(crate::vault::biometric::platform_available())).await.unwrap_or(false)
    }

    #[cfg(not(target_os = "android"))]
    pub async fn seal_biometric(
        _app: &AppHandle,
        unlockers: Unlockers,
        secret: Zeroizing<[u8; 32]>,
        _prompt: PromptText,
    ) -> Result<Seal, String> {
        blocking(move || unlockers.seal_with_platform(&*secret)).await
    }

    #[cfg(not(target_os = "android"))]
    pub async fn open_biometric(_app: &AppHandle, unlockers: Unlockers, _prompt: PromptText) -> Result<Zeroizing<Vec<u8>>, String> {
        blocking(move || unlockers.open_with_platform()).await
    }

    #[cfg(not(target_os = "android"))]
    pub async fn seal_key(
        _app: &AppHandle,
        unlockers: Unlockers,
        label: String,
        secret: Zeroizing<[u8; 32]>,
        pin: Option<Zeroizing<String>>,
    ) -> Result<Seal, String> {
        blocking(move || unlockers.seal_with_key(&label, &*secret, pin.as_deref().map(|p| p.as_str()))).await
    }

    #[cfg(not(target_os = "android"))]
    pub async fn open_key(
        _app: &AppHandle,
        unlockers: Unlockers,
        pin: Option<Zeroizing<String>>,
    ) -> Result<Zeroizing<Vec<u8>>, String> {
        blocking(move || unlockers.open_with_key(pin.as_deref().map(|p| p.as_str()))).await
    }

    #[cfg(not(target_os = "android"))]
    pub async fn forget(_app: &AppHandle, seal: Seal) {
        let _ = blocking(move || {
            crate::vault::biometric::forget(&seal);
            Ok(())
        })
        .await;
    }

    #[cfg(target_os = "android")]
    mod android {
        use super::PromptText;
        use crate::vault::biometric::{Seal, Unlockers, ANDROID_BIOMETRIC, ANDROID_KEY_ALIAS};
        use crate::vault::fido2::RP_ID;
        use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
        use serde::Deserialize;
        use serde_json::json;
        use tauri::{AppHandle, Manager};
        use tauri_plugin_reach_unlock::Unlock;
        use zeroize::Zeroizing;

        fn plugin(app: &AppHandle) -> tauri::State<'_, Unlock<tauri::Wry>> {
            app.state::<Unlock<tauri::Wry>>()
        }

        fn decode_32(text: &str) -> Result<Zeroizing<[u8; 32]>, String> {
            let bytes = Zeroizing::new(BASE64.decode(text).map_err(|e| e.to_string())?);
            let mut out = Zeroizing::new([0u8; 32]);
            if bytes.len() != 32 {
                return Err("The security key's answer has the wrong length".into());
            }
            out.copy_from_slice(&bytes);
            Ok(out)
        }

        #[derive(Deserialize)]
        struct Status {
            available: bool,
        }

        #[derive(Deserialize)]
        struct Sealed {
            iv: String,
            ciphertext: String,
        }

        #[derive(Deserialize)]
        struct Opened {
            secret: String,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Credential {
            credential_id: String,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct KeyOutput {
            credential_id: String,
            output: String,
        }

        pub async fn platform_available(app: &AppHandle) -> bool {
            plugin(app)
                .call::<Status>("biometricStatus", json!({}))
                .await
                .map(|s| s.available)
                .unwrap_or(false)
        }

        pub async fn seal_biometric(
            app: &AppHandle,
            unlockers: Unlockers,
            secret: Zeroizing<[u8; 32]>,
            prompt: PromptText,
        ) -> Result<Seal, String> {
            let payload = json!({
                "alias": ANDROID_KEY_ALIAS,
                "secret": BASE64.encode(*secret),
                "title": prompt.title,
                "subtitle": prompt.subtitle,
                "cancel": prompt.cancel,
            });
            let sealed: Sealed = plugin(app).call("biometricSeal", payload).await?;
            Ok(unlockers.android_seal(&sealed.iv, &sealed.ciphertext))
        }

        pub async fn open_biometric(app: &AppHandle, unlockers: Unlockers, prompt: PromptText) -> Result<Zeroizing<Vec<u8>>, String> {
            let (iv, ciphertext) = unlockers.android_sealed().ok_or("Fingerprint unlock is not turned on")?;
            let payload = json!({
                "alias": ANDROID_KEY_ALIAS,
                "iv": iv,
                "ciphertext": ciphertext,
                "title": prompt.title,
                "subtitle": prompt.subtitle,
                "cancel": prompt.cancel,
            });
            let opened: Opened = plugin(app).call("biometricOpen", payload).await?;
            Ok(Zeroizing::new(BASE64.decode(&opened.secret).map_err(|e| e.to_string())?))
        }

        pub async fn seal_key(
            app: &AppHandle,
            unlockers: Unlockers,
            label: String,
            secret: Zeroizing<[u8; 32]>,
            pin: Option<Zeroizing<String>>,
        ) -> Result<Seal, String> {
            let (existing, salt) = unlockers.key_request()?;
            let pin = pin.as_deref().map(|p| p.as_str());
            let made: Credential = plugin(app)
                .call(
                    "keyMakeCredential",
                    json!({
                        "rpId": RP_ID,
                        "userId": BASE64.encode(unlockers.user_uuid.as_bytes()),
                        "label": label,
                        "pin": pin,
                        "exclude": existing.iter().map(|id| BASE64.encode(id)).collect::<Vec<_>>(),
                    }),
                )
                .await?;
            let answer: KeyOutput = plugin(app)
                .call(
                    "keyHmacSecret",
                    json!({
                        "rpId": RP_ID,
                        "credentialIds": [made.credential_id],
                        "salt": BASE64.encode(salt),
                        "pin": pin,
                    }),
                )
                .await?;
            let credential = BASE64.decode(&answer.credential_id).map_err(|e| e.to_string())?;
            unlockers.seal_with_key_output(&label, &credential, &*decode_32(&answer.output)?, &*secret)
        }

        pub async fn open_key(
            app: &AppHandle,
            unlockers: Unlockers,
            pin: Option<Zeroizing<String>>,
        ) -> Result<Zeroizing<Vec<u8>>, String> {
            let (existing, salt) = unlockers.key_request()?;
            let answer: KeyOutput = plugin(app)
                .call(
                    "keyHmacSecret",
                    json!({
                        "rpId": RP_ID,
                        "credentialIds": existing.iter().map(|id| BASE64.encode(id)).collect::<Vec<_>>(),
                        "salt": BASE64.encode(salt),
                        "pin": pin.as_deref().map(|p| p.as_str()),
                    }),
                )
                .await?;
            let credential = BASE64.decode(&answer.credential_id).map_err(|e| e.to_string())?;
            unlockers.open_with_key_output(&credential, &*decode_32(&answer.output)?)
        }

        pub async fn forget(app: &AppHandle, seal: Seal) {
            if seal.kind == ANDROID_BIOMETRIC {
                let _ = plugin(app).call::<serde_json::Value>("biometricForget", json!({ "alias": ANDROID_KEY_ALIAS })).await;
            }
        }
    }

    #[cfg(target_os = "android")]
    pub use android::*;
}

#[tauri::command]
#[tracing::instrument(skip(state, app))]
pub async fn vault_unlock_methods(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<UnlockMethods, String> {
    let methods = state.vault_manager.lock().await.unlockers().map(|u| u.list()).unwrap_or_default();
    let platform = biometric::platform_method();
    let platform_available = platform.is_some() && device::platform_available(&app).await;
    Ok(UnlockMethods {
        platform,
        platform_available,
        keys_supported: fido2::supported(),
        keys_ask_pin: fido2::asks_pin_itself(),
        methods,
    })
}

/// Turn the platform biometric (Windows Hello, Touch ID, Android's
/// fingerprint) on. The vault must be open and a master password set. The
/// manager is not held while the system prompt is up.
#[tauri::command]
#[tracing::instrument(skip(state, app, prompt))]
pub async fn vault_biometric_enable(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    prompt: Option<PromptText>,
) -> Result<(), String> {
    let (mut unlockers, secret) = state.vault_manager.lock().await.unlock_enrolment().await.map_err(|e| e.to_string())?;
    let seal = device::seal_biometric(&app, unlockers.clone(), secret, prompt.unwrap_or_default()).await?;
    unlockers.add(seal);
    state.vault_manager.lock().await.save_unlockers(&unlockers).map_err(|e| e.to_string())
}

/// Open the vault with the platform biometric.
#[tauri::command]
#[tracing::instrument(skip(state, app, prompt))]
pub async fn vault_biometric_unlock(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    prompt: Option<PromptText>,
) -> Result<bool, String> {
    let unlockers = state.vault_manager.lock().await.unlockers().ok_or("Biometric unlock is not turned on")?;
    let secret = device::open_biometric(&app, unlockers, prompt.unwrap_or_default()).await?;
    let mut manager = state.vault_manager.lock().await;
    manager.unlock_with_device(&secret).await.map_err(|e| e.to_string())
}

/// Add a security key. The user touches it twice (and gives its PIN): once
/// to add Reach's credential to it, once for the secret that seals the key.
#[tauri::command]
#[tracing::instrument(skip(state, app, pin))]
pub async fn vault_security_key_add(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    label: String,
    pin: Option<String>,
) -> Result<(), String> {
    let label = match label.trim() {
        "" => "Security key".to_string(),
        l => l.chars().take(64).collect(),
    };
    let pin = pin.map(zeroize::Zeroizing::new);
    let (mut unlockers, secret) = state.vault_manager.lock().await.unlock_enrolment().await.map_err(|e| e.to_string())?;
    let seal = device::seal_key(&app, unlockers.clone(), label, secret, pin).await?;
    unlockers.add(seal);
    state.vault_manager.lock().await.save_unlockers(&unlockers).map_err(|e| e.to_string())
}

/// Open the vault with whichever added security key is plugged in.
#[tauri::command]
#[tracing::instrument(skip(state, app, pin))]
pub async fn vault_security_key_unlock(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    pin: Option<String>,
) -> Result<bool, String> {
    let pin = pin.map(zeroize::Zeroizing::new);
    let unlockers = state.vault_manager.lock().await.unlockers().ok_or("No security key has been added")?;
    let secret = device::open_key(&app, unlockers, pin).await?;
    let mut manager = state.vault_manager.lock().await;
    manager.unlock_with_device(&secret).await.map_err(|e| e.to_string())
}

/// Remove an unlock method. Removing the last one puts the key back in the
/// keychain, so Reach opens by itself at start-up again.
#[tauri::command]
#[tracing::instrument(skip(state, app))]
pub async fn vault_unlock_method_remove(app: tauri::AppHandle, state: State<'_, AppState>, id: String) -> Result<(), String> {
    let removed = state.vault_manager.lock().await.remove_unlocker(&id).map_err(|e| e.to_string())?;
    if let Some(seal) = removed {
        device::forget(&app, seal).await;
    }
    Ok(())
}

#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn vault_is_locked(state: State<'_, AppState>) -> Result<bool, String> {
    let manager = state.vault_manager.lock().await;
    Ok(manager.is_locked())
}

#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn vault_has_identity(state: State<'_, AppState>) -> Result<bool, String> {
    let manager = state.vault_manager.lock().await;
    Ok(manager.has_identity().await)
}

/// Auto-unlock using OS keychain (TLS-style, no password needed).
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn vault_auto_unlock(state: State<'_, AppState>) -> Result<bool, String> {
    let mut manager = state.vault_manager.lock().await;
    manager.auto_unlock().await.map_err(|e| e.to_string())
}

/// Reset vault - delete all local data and start fresh.
/// WARNING: This is destructive! All encrypted data will be lost.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn vault_reset(state: State<'_, AppState>) -> Result<(), String> {
    let mut manager = state.vault_manager.lock().await;
    // The settings the listings and a kept passphrase came from are gone.
    crate::pwmanager::forget_all();
    manager.reset().await.map_err(|e| e.to_string())
}

/// Export identity for backup/multi-device (returns base64 secret key).
/// WARNING: This is sensitive! User must protect this value.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn vault_export_identity(state: State<'_, AppState>) -> Result<String, String> {
    let manager = state.vault_manager.lock().await;
    manager.export_identity().map_err(|e| e.to_string())
}

/// Import identity from backup (for new device).
#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state, secret_key))]
pub async fn vault_import_identity(
    secret_key: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let mut manager = state.vault_manager.lock().await;
    // Another identity: as for a new one.
    crate::pwmanager::forget_all();
    manager.import_identity(&secret_key).await.map_err(|e| e.to_string())
}

#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn vault_get_public_key(state: State<'_, AppState>) -> Result<Option<String>, String> {
    let manager = state.vault_manager.lock().await;
    Ok(manager.get_public_key())
}

#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn vault_get_user_uuid(state: State<'_, AppState>) -> Result<Option<String>, String> {
    let manager = state.vault_manager.lock().await;
    Ok(manager.get_user_uuid())
}

// ==================== VAULT MANAGEMENT ====================

#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state, sync_token))]
pub async fn vault_create(
    name: String,
    vault_type: String,
    sync_url: Option<String>,
    sync_token: Option<String>,
    state: State<'_, AppState>,
) -> Result<VaultInfo, String> {
    let mut manager = state.vault_manager.lock().await;

    let vt = match vault_type.as_str() {
        "private" => VaultType::Private,
        "shared" => VaultType::Shared { members: vec![] },
        _ => return Err("Invalid vault type".to_string()),
    };

    manager.create_vault(&name, vt, sync_url.as_deref(), sync_token.as_deref()).await.map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state, token))]
pub async fn vault_open(
    vault_id: String,
    sync_url: Option<String>,
    token: Option<String>,
    state: State<'_, AppState>,
) -> Result<VaultInfo, String> {
    let mut manager = state.vault_manager.lock().await;
    manager
        .open_vault(&vault_id, sync_url.as_deref(), token.as_deref())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state))]
pub async fn vault_close(
    vault_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut manager = state.vault_manager.lock().await;
    manager.close_vault(&vault_id).await.map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state))]
pub async fn vault_delete(
    vault_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut manager = state.vault_manager.lock().await;
    manager.delete_vault(&vault_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn vault_list(state: State<'_, AppState>) -> Result<Vec<VaultInfo>, String> {
    let manager = state.vault_manager.lock().await;
    manager.list_vaults().await.map_err(|e| e.to_string())
}

/// Shared vaults joined more than once, grouped, for the user to choose
/// which entry of each to keep. Removing the others is `vault_delete`, which
/// only forgets the entry on this device.
#[tauri::command]
pub async fn vault_duplicates(state: State<'_, AppState>) -> Result<Vec<Vec<VaultInfo>>, String> {
    Ok(state.vault_manager.lock().await.duplicate_vaults().await)
}

#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state))]
pub async fn vault_unlock_vault(
    vault_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut manager = state.vault_manager.lock().await;
    manager.unlock_vault(&vault_id).await.map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state))]
pub async fn vault_lock_vault(
    vault_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut manager = state.vault_manager.lock().await;
    manager.lock_vault(&vault_id);
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state))]
pub async fn vault_sync(
    vault_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut manager = state.vault_manager.lock().await;
    manager.sync_vault(&vault_id).await.map_err(|e| e.to_string())
}

// ==================== SECRETS ====================

#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(value, state))]
pub async fn vault_secret_create(
    vault_id: String,
    name: String,
    category: String,
    value: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let manager = state.vault_manager.lock().await;
    refuse_internal_vault(&manager, &vault_id)?;

    let cat: SecretCategory = category.parse().map_err(|e: String| e)?;
    let plaintext = SecretBox::new(Box::new(value.into_bytes()));

    manager
        .create_secret(&vault_id, &name, cat, plaintext)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state))]
pub async fn vault_secret_read(
    vault_id: String,
    secret_id: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let manager = state.vault_manager.lock().await;
    refuse_internal_vault(&manager, &vault_id)?;

    let plaintext = manager
        .read_secret(&vault_id, &secret_id)
        .await
        .map_err(|e| e.to_string())?;

    use secrecy::ExposeSecret;
    String::from_utf8(plaintext.expose_secret().clone())
        .map_err(|e| format!("Invalid UTF-8: {}", e))
}

#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(value, state))]
pub async fn vault_secret_update(
    vault_id: String,
    secret_id: String,
    value: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let manager = state.vault_manager.lock().await;
    refuse_internal_vault(&manager, &vault_id)?;

    let plaintext = SecretBox::new(Box::new(value.into_bytes()));

    manager
        .update_secret(&vault_id, &secret_id, plaintext)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state))]
pub async fn vault_secret_delete(
    vault_id: String,
    secret_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let manager = state.vault_manager.lock().await;
    refuse_internal_vault(&manager, &vault_id)?;
    manager
        .delete_secret(&vault_id, &secret_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state))]
pub async fn vault_secret_list(
    vault_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<SecretMetadata>, String> {
    let manager = state.vault_manager.lock().await;
    refuse_internal_vault(&manager, &vault_id)?;
    let mut secrets = manager.list_secrets(&vault_id).await.map_err(|e| e.to_string())?;
    // A shared vault keeps its session folders; they are the sidebar's
    // structure, not secrets to list here.
    secrets.retain(|s| s.category != "folder");
    Ok(secrets)
}

// ==================== SHARING ====================

#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state))]
pub async fn vault_invite_member(
    vault_id: String,
    invitee_public_key: String,
    invitee_uuid: String,
    role: String,
    state: State<'_, AppState>,
) -> Result<InviteInfo, String> {
    let manager = state.vault_manager.lock().await;

    let pk_bytes = BASE64
        .decode(&invitee_public_key)
        .map_err(|e| format!("Invalid public key: {}", e))?;

    if pk_bytes.len() != 32 {
        return Err(format!("Invalid public key length: expected 32, got {}", pk_bytes.len()));
    }

    let mut public_key = [0u8; 32];
    public_key.copy_from_slice(&pk_bytes);

    let member_role: MemberRole = role.parse().map_err(|e: String| e)?;

    manager
        .invite_member(&vault_id, &public_key, &invitee_uuid, member_role)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(token, state))]
pub async fn vault_accept_invite(
    sync_url: String,
    token: String,
    state: State<'_, AppState>,
) -> Result<VaultInfo, String> {
    let mut manager = state.vault_manager.lock().await;
    manager
        .accept_invite(&sync_url, &token)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state))]
pub async fn vault_remove_member(
    vault_id: String,
    user_uuid: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let manager = state.vault_manager.lock().await;
    manager
        .remove_member(&vault_id, &user_uuid)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state))]
pub async fn vault_list_members(
    vault_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<MemberInfo>, String> {
    let manager = state.vault_manager.lock().await;
    manager.list_members(&vault_id).await.map_err(|e| e.to_string())
}

// ==================== SHARE INDIVIDUAL ITEMS ====================

/// Share a specific secret (session/credential) with another user.
#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state))]
pub async fn vault_share_item(
    vault_id: String,
    secret_id: String,
    recipient_uuid: String,
    recipient_public_key: String,
    expires_in_hours: Option<u64>,
    state: State<'_, AppState>,
) -> Result<ShareItemResult, String> {
    let manager = state.vault_manager.lock().await;

    let pk_bytes = BASE64
        .decode(&recipient_public_key)
        .map_err(|e| format!("Invalid public key: {}", e))?;

    if pk_bytes.len() != 32 {
        return Err(format!("Invalid public key length: expected 32, got {}", pk_bytes.len()));
    }

    let mut public_key = [0u8; 32];
    public_key.copy_from_slice(&pk_bytes);

    manager
        .share_item(&vault_id, &secret_id, &recipient_uuid, &public_key, expires_in_hours)
        .await
        .map_err(|e| e.to_string())
}

/// List items shared from a vault.
#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state))]
pub async fn vault_list_shared_items(
    vault_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<SharedItemInfo>, String> {
    let manager = state.vault_manager.lock().await;
    manager.list_shared_items(&vault_id).await.map_err(|e| e.to_string())
}

/// Revoke a shared item.
#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state))]
pub async fn vault_revoke_shared_item(
    vault_id: String,
    share_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let manager = state.vault_manager.lock().await;
    manager.revoke_shared_item(&vault_id, &share_id).await.map_err(|e| e.to_string())
}

/// Accept a shared item (copy to local vault).
#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state))]
pub async fn vault_accept_shared_item(
    source_vault_id: String,
    share_id: String,
    target_vault_id: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let manager = state.vault_manager.lock().await;
    manager
        .accept_shared_item(&source_vault_id, &share_id, &target_vault_id)
        .await
        .map_err(|e| e.to_string())
}

/// List items shared with me.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn vault_list_received_shares(
    state: State<'_, AppState>,
) -> Result<Vec<ReceivedShare>, String> {
    let manager = state.vault_manager.lock().await;
    manager.list_received_shares().await.map_err(|e| e.to_string())
}

// ==================== APP SETTINGS (ENCRYPTED) ====================

/// Get app settings.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn vault_get_settings(
    state: State<'_, AppState>,
) -> Result<AppSettings, String> {
    let manager = state.vault_manager.lock().await;
    manager.get_settings().await.map_err(|e| e.to_string())
}

/// Save app settings.
#[tauri::command]
#[tracing::instrument(skip(state, settings))]
pub async fn vault_save_settings(
    settings: AppSettings,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let manager = state.vault_manager.lock().await;
    manager.save_settings(&settings).await.map_err(|e| e.to_string())
}

/// Get Turso config.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn vault_get_turso_config(
    state: State<'_, AppState>,
) -> Result<(Option<String>, Option<String>), String> {
    let manager = state.vault_manager.lock().await;
    manager.get_turso_config().await.map_err(|e| e.to_string())
}

/// Set Turso config.
#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state, token))]
pub async fn vault_set_turso_config(
    org: Option<String>,
    token: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let manager = state.vault_manager.lock().await;
    manager.set_turso_config(org, token).await.map_err(|e| e.to_string())
}

// ==================== TURSO PLATFORM API ====================

use crate::vault::{turso_api, TursoDbInfo};

/// Create a new database in Turso (for shared vaults).
/// The database goes into the configured group; one is found or made
/// first, since a new Turso account has none (see `turso_api::choose_group`).
/// Failures are logged with Turso's own reason.
#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state), err)]
pub async fn turso_create_database(
    db_name: String,
    state: State<'_, AppState>,
) -> Result<TursoDbInfo, String> {
    let manager = state.vault_manager.lock().await;
    let settings = manager.get_settings().await.map_err(|e| e.to_string())?;

    let org = settings.turso_org.map(|o| o.trim().to_string()).filter(|o| !o.is_empty()).ok_or("Turso organization not configured")?;
    let api_token = settings.turso_api_token.map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).ok_or("Turso API token not configured")?;
    let wanted = settings.turso_group.map(|g| g.trim().to_string()).filter(|g| !g.is_empty()).unwrap_or_else(|| "default".to_string());

    let existing = turso_api::list_groups(&org, &api_token).await.map_err(|e| e.to_string())?;
    let group = match turso_api::choose_group(&existing, &wanted) {
        turso_api::GroupChoice::Use(g) => g,
        turso_api::GroupChoice::Create(g) => {
            turso_api::create_group(&org, &api_token, &g).await.map_err(|e| e.to_string())?;
            g
        }
    };

    turso_api::create_database(&org, &api_token, &db_name, &group)
        .await
        .map_err(|e| e.to_string())
}

/// Create an auth token for a Turso database.
#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state), err)]
pub async fn turso_create_database_token(
    db_name: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let manager = state.vault_manager.lock().await;
    let settings = manager.get_settings().await.map_err(|e| e.to_string())?;

    let org = settings.turso_org.map(|o| o.trim().to_string()).filter(|o| !o.is_empty()).ok_or("Turso organization not configured")?;
    let api_token = settings.turso_api_token.map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).ok_or("Turso API token not configured")?;

    turso_api::create_database_token(&org, &api_token, &db_name)
        .await
        .map_err(|e| e.to_string())
}

// ==================== FULL BACKUP ====================

use crate::vault::export::BackupPreview;

/// A file the user picked in the system's file dialog. On desktop that is a
/// path. On Android the picker hands back a `content://` link, which is not a
/// path the app may open: it is read and written through the ContentResolver,
/// in Reach's Android plugin.
pub(crate) mod picked_file {
    use tauri::AppHandle;

    #[cfg(target_os = "android")]
    fn is_link(path: &str) -> bool {
        path.starts_with("content://")
    }

    pub async fn read(_app: &AppHandle, path: &str) -> Result<Vec<u8>, String> {
        #[cfg(target_os = "android")]
        if is_link(path) {
            use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
            use tauri::Manager;
            #[derive(serde::Deserialize)]
            struct Read {
                data: String,
            }
            let read: Read = _app
                .state::<tauri_plugin_reach_unlock::Unlock<tauri::Wry>>()
                .call("readUri", serde_json::json!({ "uri": path }))
                .await?;
            return BASE64.decode(read.data).map_err(|e| e.to_string());
        }
        tokio::fs::read(path).await.map_err(|e| format!("Cannot read {path}: {e}"))
    }

    pub async fn write(_app: &AppHandle, path: &str, bytes: &[u8]) -> Result<(), String> {
        #[cfg(target_os = "android")]
        if is_link(path) {
            use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
            use tauri::Manager;
            return _app
                .state::<tauri_plugin_reach_unlock::Unlock<tauri::Wry>>()
                .call::<serde_json::Value>("writeUri", serde_json::json!({ "uri": path, "data": BASE64.encode(bytes) }))
                .await
                .map(|_| ());
        }
        tokio::fs::write(path, bytes).await.map_err(|e| format!("Cannot write {path}: {e}"))
    }
}

/// Export a full encrypted backup to a file.
#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(app, state, export_password))]
pub async fn vault_export_backup(
    app: tauri::AppHandle,
    export_password: String,
    file_path: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let sealed = state
        .vault_manager
        .lock()
        .await
        .export_full_backup(&export_password)
        .await
        .map_err(|e| e.to_string())?;
    picked_file::write(&app, &file_path, &sealed).await
}

/// Preview a backup file (validate and return metadata).
#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(app, export_password))]
pub async fn vault_preview_backup(
    app: tauri::AppHandle,
    file_path: String,
    export_password: String,
) -> Result<BackupPreview, String> {
    let data = picked_file::read(&app, &file_path).await?;
    tracing::info!("Checking a backup of {} bytes", data.len());
    // Checking a backup only decrypts the file. It needs neither the vault
    // nor its lock, and waiting for the lock is how Verify hung behind a
    // keychain call that never returned (issue #87). Argon2id over 256 MiB
    // takes seconds, so it runs off the async workers.
    let preview = tokio::task::spawn_blocking(move || crate::vault::export::preview_bundle(&data, &export_password))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string());
    tracing::info!("Backup check {}", if preview.is_ok() { "passed" } else { "failed" });
    preview
}

/// Import a full encrypted backup from a file.
#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(app, state, export_password, master_password))]
pub async fn vault_import_backup(
    app: tauri::AppHandle,
    file_path: String,
    export_password: String,
    master_password: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let data = picked_file::read(&app, &file_path).await?;
    let mut manager = state.vault_manager.lock().await;
    manager
        .import_full_backup(&data, &export_password, &master_password)
        .await
        .map_err(|e| e.to_string())
}

// ==================== PERSONAL SYNC CONFIG ====================

/// Set personal sync config (for cloud backup of ALL user data).
/// This stores the sync URL and token in the identity file.
#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state, sync_token))]
pub async fn vault_set_personal_sync(
    sync_url: Option<String>,
    sync_token: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut manager = state.vault_manager.lock().await;
    manager.set_personal_sync_config(sync_url, sync_token).await.map_err(|e| e.to_string())
}

/// Get personal sync config.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn vault_get_personal_sync(
    state: State<'_, AppState>,
) -> Result<(Option<String>, Option<String>), String> {
    let manager = state.vault_manager.lock().await;
    Ok(manager.get_personal_sync_config())
}

#[cfg(test)]
mod internal_vault_tests {
    use super::*;

    #[test]
    fn reach_s_own_vaults_are_refused_by_id() {
        let ids = |name: &str| match name {
            "__settings__" => Some("settings-id".to_string()),
            "__sessions__" => Some("sessions-id".to_string()),
            "__personal__" => Some("unified-id".to_string()),
            _ => None,
        };
        assert!(is_internal_vault_id("settings-id", ids));
        assert!(is_internal_vault_id("sessions-id", ids));
        assert!(is_internal_vault_id("unified-id", ids), "the vault they map onto with sync on");
        assert!(!is_internal_vault_id("my-team-vault", ids));
        assert!(!is_internal_vault_id("", ids));
        // Every internal vault, not only those named above.
        for name in INTERNAL_VAULTS {
            let id = format!("id-of-{name}");
            assert!(is_internal_vault_id(&id, |n| Some(format!("id-of-{n}"))), "{name}");
        }
        // No vault open yet: nothing is internal.
        assert!(!is_internal_vault_id("settings-id", |_| None));
    }
}
