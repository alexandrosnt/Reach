use std::path::Path;

use libsql::{Builder, Database};

use crate::vault::error::VaultError;

/// Sync configuration for Turso.
pub struct SyncConfig {
    pub sync_url: String,
    pub auth_token: String,
}

/// Create a database - local-only or remote (Turso).
/// Note: Embedded replicas have issues on Windows (https://github.com/tursodatabase/libsql/issues/2074)
/// So we use remote-only for synced vaults instead of embedded replicas.
pub async fn create_replica(
    path: &Path,
    sync_config: Option<&SyncConfig>,
) -> Result<Database, VaultError> {
    let db = match sync_config {
        Some(config) => {
            // Remote-only connection to Turso (no local replica due to Windows bug)
            tracing::info!("Creating remote database connection: {}", config.sync_url);
            Builder::new_remote(
                config.sync_url.clone(),
                config.auth_token.clone(),
            )
            .build()
            .await
            .map_err(|e| VaultError::SyncError(format!("Failed to connect to Turso: {}", e)))?
        }
        None => {
            // Local-only database
            Builder::new_local(path)
                .build()
                .await
                .map_err(|e| VaultError::DatabaseError(e.to_string()))?
        }
    };

    Ok(db)
}

/// Sync vault with Turso.
/// Note: For remote-only connections, this is a no-op since data is always on the server.
pub async fn sync_vault(db: &Database) -> Result<(), VaultError> {
    // For remote databases, sync() may not be needed, but we call it anyway for consistency
    // It will be a no-op for remote-only connections
    match db.sync().await {
        Ok(_) => {
            tracing::info!("Vault sync complete");
        }
        Err(e) => {
            // For remote-only connections, sync might fail - that's ok
            tracing::debug!("Sync call returned: {} (may be expected for remote-only)", e);
        }
    }
    Ok(())
}
