use crate::playbook::schema::SavedPlaybookProject;
use crate::vault::types::SecretCategory;
use secrecy::SecretBox;

pub const PLAYBOOK_VAULT_NAME: &str = "__playbooks__";

/// Ensure the playbook vault exists, creating it if needed.
pub async fn ensure_playbook_vault(
    manager: &mut crate::vault::VaultManager,
) -> Result<String, String> {
    if let Some(vault_id) = manager.get_vault_id_by_name(PLAYBOOK_VAULT_NAME) {
        let _ = manager.open_vault(&vault_id, None, None).await;
        manager
            .unlock_vault(&vault_id)
            .await
            .map_err(|e| e.to_string())?;
        Ok(vault_id)
    } else {
        let vault = manager
            .create_vault(
                PLAYBOOK_VAULT_NAME,
                crate::vault::types::VaultType::Private,
                None,
                None,
            )
            .await
            .map_err(|e| e.to_string())?;
        Ok(vault.id)
    }
}

/// Get the playbook vault ID if it exists.
pub fn get_playbook_vault_id_if_exists(
    manager: &crate::vault::VaultManager,
) -> Option<String> {
    manager.get_vault_id_by_name(PLAYBOOK_VAULT_NAME)
}

/// Save a playbook project to the vault.
pub async fn save_project(
    manager: &mut crate::vault::VaultManager,
    project: &SavedPlaybookProject,
    existing_id: Option<&str>,
) -> Result<SavedPlaybookProject, String> {
    let vault_id = ensure_playbook_vault(manager).await?;

    let json = serde_json::to_string(project).map_err(|e| e.to_string())?;
    let plaintext = SecretBox::new(Box::new(json.into_bytes()));

    if let Some(eid) = existing_id {
        manager
            .update_secret(&vault_id, eid, plaintext)
            .await
            .map_err(|e| e.to_string())?;
    } else {
        manager
            .create_secret_with_id(
                &vault_id,
                &project.id,
                &project.name,
                SecretCategory::Custom("playbook".to_string()),
                plaintext,
            )
            .await
            .map_err(|e| e.to_string())?;
    }

    Ok(project.clone())
}

/// List all saved playbook projects.
pub async fn list_projects(
    manager: &crate::vault::VaultManager,
) -> Result<Vec<SavedPlaybookProject>, String> {
    let vault_id = match get_playbook_vault_id_if_exists(manager) {
        Some(id) => id,
        None => return Ok(Vec::new()),
    };

    let secrets = manager
        .list_secrets(&vault_id)
        .await
        .map_err(|e| e.to_string())?;

    let mut projects = Vec::with_capacity(secrets.len());
    for secret in secrets {
        if let Ok(plaintext) = manager.read_secret(&vault_id, &secret.id).await {
            use secrecy::ExposeSecret;
            if let Ok(json) = String::from_utf8(plaintext.expose_secret().clone()) {
                if let Ok(proj) = serde_json::from_str::<SavedPlaybookProject>(&json) {
                    projects.push(proj);
                }
            }
        }
    }

    Ok(projects)
}

/// Delete a saved playbook project.
pub async fn delete_project(
    manager: &crate::vault::VaultManager,
    id: &str,
) -> Result<(), String> {
    let vault_id = match get_playbook_vault_id_if_exists(manager) {
        Some(id) => id,
        None => return Ok(()),
    };

    manager
        .delete_secret(&vault_id, id)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}
