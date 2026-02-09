use crate::plugin::schema::PluginConfig;
use crate::vault::types::SecretCategory;
use secrecy::SecretBox;

pub const PLUGIN_VAULT_NAME: &str = "__plugins__";

/// Ensure the plugin vault exists, creating it if needed.
pub async fn ensure_plugin_vault(
    manager: &mut crate::vault::VaultManager,
) -> Result<String, String> {
    if let Some(vault_id) = manager.get_vault_id_by_name(PLUGIN_VAULT_NAME) {
        let _ = manager.open_vault(&vault_id, None, None).await;
        manager
            .unlock_vault(&vault_id)
            .await
            .map_err(|e| e.to_string())?;
        Ok(vault_id)
    } else {
        let vault = manager
            .create_vault(
                PLUGIN_VAULT_NAME,
                crate::vault::types::VaultType::Private,
                None,
                None,
            )
            .await
            .map_err(|e| e.to_string())?;
        Ok(vault.id)
    }
}

/// Get the plugin vault ID if it exists.
pub fn get_plugin_vault_id_if_exists(
    manager: &crate::vault::VaultManager,
) -> Option<String> {
    manager.get_vault_id_by_name(PLUGIN_VAULT_NAME)
}

/// Save a plugin config to the vault.
pub async fn save_plugin_config(
    manager: &mut crate::vault::VaultManager,
    config: &PluginConfig,
) -> Result<(), String> {
    let vault_id = ensure_plugin_vault(manager).await?;

    let json = serde_json::to_string(config).map_err(|e| e.to_string())?;
    let plaintext = SecretBox::new(Box::new(json.into_bytes()));

    // Check if config already exists
    let secrets = manager
        .list_secrets(&vault_id)
        .await
        .map_err(|e| e.to_string())?;

    let existing = secrets.iter().find(|s| s.name == config.id);

    if let Some(secret) = existing {
        manager
            .update_secret(&vault_id, &secret.id, plaintext)
            .await
            .map_err(|e| e.to_string())?;
    } else {
        let secret_id = uuid::Uuid::new_v4().to_string();
        manager
            .create_secret_with_id(
                &vault_id,
                &secret_id,
                &config.id,
                SecretCategory::Custom("plugin_config".to_string()),
                plaintext,
            )
            .await
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// Load all plugin configs from the vault.
pub async fn load_plugin_configs(
    manager: &crate::vault::VaultManager,
) -> Result<Vec<PluginConfig>, String> {
    let vault_id = match get_plugin_vault_id_if_exists(manager) {
        Some(id) => id,
        None => return Ok(Vec::new()),
    };

    let secrets = manager
        .list_secrets(&vault_id)
        .await
        .map_err(|e| e.to_string())?;

    let mut configs = Vec::new();
    for secret in secrets {
        if let Ok(plaintext) = manager.read_secret(&vault_id, &secret.id).await {
            use secrecy::ExposeSecret;
            if let Ok(json) = String::from_utf8(plaintext.expose_secret().clone()) {
                if let Ok(cfg) = serde_json::from_str::<PluginConfig>(&json) {
                    configs.push(cfg);
                }
            }
        }
    }

    Ok(configs)
}

/// Load a single plugin config from the vault.
pub async fn load_plugin_config(
    manager: &crate::vault::VaultManager,
    plugin_id: &str,
) -> Result<Option<PluginConfig>, String> {
    let configs = load_plugin_configs(manager).await?;
    Ok(configs.into_iter().find(|c| c.id == plugin_id))
}
