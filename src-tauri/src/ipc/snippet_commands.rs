use tauri::State;
use secrecy::SecretBox;

use crate::state::{AppState, Snippet};
use crate::vault::types::SecretCategory;

const SNIPPETS_VAULT_NAME: &str = "__snippets__";

async fn ensure_snippets_vault(
    manager: &mut crate::vault::VaultManager,
) -> Result<String, String> {
    if let Some(vault_id) = manager.get_vault_id_by_name(SNIPPETS_VAULT_NAME) {
        let _ = manager.open_vault(&vault_id, None, None).await;
        manager
            .unlock_vault(&vault_id)
            .await
            .map_err(|e| e.to_string())?;
        Ok(vault_id)
    } else {
        let vault = manager
            .create_vault(
                SNIPPETS_VAULT_NAME,
                crate::vault::types::VaultType::Private,
                None,
                None,
            )
            .await
            .map_err(|e| e.to_string())?;
        Ok(vault.id)
    }
}

fn get_snippets_vault_id_if_exists(
    manager: &crate::vault::VaultManager,
) -> Option<String> {
    manager.get_vault_id_by_name(SNIPPETS_VAULT_NAME)
}

#[tauri::command]
pub async fn snippet_list(state: State<'_, AppState>) -> Result<Vec<Snippet>, String> {
    let manager = state.vault_manager.lock().await;

    if manager.is_locked() {
        return Ok(Vec::new());
    }

    let vault_id = match get_snippets_vault_id_if_exists(&manager) {
        Some(id) => id,
        None => return Ok(Vec::new()),
    };

    let secrets = manager
        .list_secrets(&vault_id)
        .await
        .map_err(|e| e.to_string())?;

    let mut snippets = Vec::new();
    for secret in secrets {
        if secret.category != "custom:snippet" {
            continue;
        }
        if let Ok(plaintext) = manager.read_secret(&vault_id, &secret.id).await {
            use secrecy::ExposeSecret;
            if let Ok(json) = String::from_utf8(plaintext.expose_secret().clone()) {
                if let Ok(snippet) = serde_json::from_str::<Snippet>(&json) {
                    snippets.push(snippet);
                }
            }
        }
    }

    Ok(snippets)
}

#[tauri::command]
pub async fn snippet_create(
    state: State<'_, AppState>,
    name: String,
    command: String,
    description: Option<String>,
    tags: Vec<String>,
) -> Result<Snippet, String> {
    let mut manager = state.vault_manager.lock().await;

    if manager.is_locked() {
        return Err("Vault is locked".to_string());
    }

    let vault_id = ensure_snippets_vault(&mut manager).await?;

    let snippet = Snippet {
        id: uuid::Uuid::new_v4().to_string(),
        name: name.clone(),
        command,
        description,
        tags,
    };

    let json = serde_json::to_string(&snippet).map_err(|e| e.to_string())?;
    let plaintext = SecretBox::new(Box::new(json.into_bytes()));

    manager
        .create_secret_with_id(
            &vault_id,
            &snippet.id,
            &name,
            SecretCategory::Custom("snippet".to_string()),
            plaintext,
        )
        .await
        .map_err(|e| e.to_string())?;

    Ok(snippet)
}

#[tauri::command]
pub async fn snippet_update(
    state: State<'_, AppState>,
    snippet: Snippet,
) -> Result<Snippet, String> {
    let manager = state.vault_manager.lock().await;

    if manager.is_locked() {
        return Err("Vault is locked".to_string());
    }

    let vault_id = match get_snippets_vault_id_if_exists(&manager) {
        Some(id) => id,
        None => return Err("Snippets vault not found".to_string()),
    };

    let json = serde_json::to_string(&snippet).map_err(|e| e.to_string())?;
    let plaintext = SecretBox::new(Box::new(json.into_bytes()));

    manager
        .update_secret(&vault_id, &snippet.id, plaintext)
        .await
        .map_err(|e| e.to_string())?;

    Ok(snippet)
}

#[tauri::command]
pub async fn snippet_delete(
    state: State<'_, AppState>,
    snippet_id: String,
) -> Result<(), String> {
    let manager = state.vault_manager.lock().await;

    if manager.is_locked() {
        return Err("Vault is locked".to_string());
    }

    let vault_id = match get_snippets_vault_id_if_exists(&manager) {
        Some(id) => id,
        None => return Ok(()),
    };

    manager
        .delete_secret(&vault_id, &snippet_id)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}
