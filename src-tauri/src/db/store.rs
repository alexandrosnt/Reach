//! Saved connections, kept in their own internal vault like OpenTofu projects.

use std::collections::HashMap;

use secrecy::{ExposeSecret, SecretBox};

use super::types::DbConnection;
use crate::vault::manager::DATABASES_VAULT;
use crate::vault::types::{SecretCategory, VaultType};
use crate::vault::VaultManager;

pub struct DbConnectionStore {
    connections: HashMap<String, DbConnection>,
    loaded: bool,
}

impl DbConnectionStore {
    pub fn new() -> Self {
        Self { connections: HashMap::new(), loaded: false }
    }

    /// Load from the vault once it is unlocked. A locked vault is not an
    /// error: the list is simply empty until it opens.
    pub async fn ensure_loaded(&mut self, vault: &mut VaultManager) -> Result<(), String> {
        if self.loaded || vault.is_locked() {
            return Ok(());
        }
        let vault_id = ensure_vault(vault).await?;
        for secret in vault.list_secrets(&vault_id).await.map_err(|e| e.to_string())? {
            let Ok(plain) = vault.read_secret(&vault_id, &secret.id).await else { continue };
            if let Ok(conn) = serde_json::from_slice::<DbConnection>(plain.expose_secret()) {
                self.connections.insert(conn.id.clone(), conn);
            }
        }
        self.loaded = true;
        Ok(())
    }

    /// Most recently used first, then by name.
    pub fn list(&self) -> Vec<DbConnection> {
        let mut all: Vec<DbConnection> = self.connections.values().cloned().collect();
        all.sort_by(|a, b| b.last_used_at.cmp(&a.last_used_at).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
        all
    }

    pub fn get(&self, id: &str) -> Option<&DbConnection> {
        self.connections.get(id)
    }

    /// Insert or replace. The vault has no update, so this deletes and
    /// re-creates, as the other stores do.
    pub async fn save(&mut self, conn: DbConnection, vault: &mut VaultManager) -> Result<(), String> {
        let vault_id = ensure_vault(vault).await?;
        let json = serde_json::to_vec(&conn).map_err(|e| e.to_string())?;
        let _ = vault.delete_secret(&vault_id, &conn.id).await;
        vault
            .create_secret_with_id(
                &vault_id,
                &conn.id,
                &conn.name,
                SecretCategory::Custom("db_connection".into()),
                SecretBox::new(Box::new(json)),
            )
            .await
            .map_err(|e| e.to_string())?;
        self.connections.insert(conn.id.clone(), conn);
        Ok(())
    }

    pub async fn delete(&mut self, id: &str, vault: &mut VaultManager) -> Result<(), String> {
        let vault_id = ensure_vault(vault).await?;
        vault.delete_secret(&vault_id, id).await.map_err(|e| e.to_string())?;
        self.connections.remove(id);
        Ok(())
    }
}

async fn ensure_vault(vault: &mut VaultManager) -> Result<String, String> {
    if let Some(id) = vault.get_vault_id_by_name(DATABASES_VAULT) {
        let _ = vault.open_vault(&id, None, None).await;
        vault.unlock_vault(&id).await.map_err(|e| e.to_string())?;
        return Ok(id);
    }
    let created = vault
        .create_vault(DATABASES_VAULT, VaultType::Private, None, None)
        .await
        .map_err(|e| e.to_string())?;
    Ok(created.id)
}
