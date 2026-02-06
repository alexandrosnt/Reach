use libsql::Connection;

use crate::vault::error::VaultError;

/// Initialize vault database schema.
pub async fn init_schema(conn: &Connection) -> Result<(), VaultError> {
    conn.execute_batch(
        r#"
        -- Vault header (single row per DB)
        CREATE TABLE IF NOT EXISTS vault_header (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            salt BLOB NOT NULL,
            user_uuid TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            vault_type TEXT NOT NULL,
            wrapped_master_dek TEXT NOT NULL
        );

        -- Members (for shared vaults)
        CREATE TABLE IF NOT EXISTS vault_members (
            user_uuid TEXT PRIMARY KEY,
            public_key BLOB NOT NULL,
            wrapped_master_dek TEXT NOT NULL,
            role TEXT NOT NULL,
            added_at INTEGER NOT NULL,
            inviter_public_key TEXT
        );

        -- Secrets (encrypted payloads)
        -- id is PRIMARY KEY for O(1) lookup
        CREATE TABLE IF NOT EXISTS secrets (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            category TEXT NOT NULL,
            nonce BLOB NOT NULL,
            ciphertext BLOB NOT NULL,
            wrapped_dek TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );

        -- Index for name lookup (still O(log n) but rarely needed)
        CREATE INDEX IF NOT EXISTS idx_secrets_name ON secrets(name);

        -- Shared items registry (for sharing individual secrets)
        -- When you share a session, it gets an entry here
        CREATE TABLE IF NOT EXISTS shared_items (
            id TEXT PRIMARY KEY,
            secret_id TEXT NOT NULL,
            recipient_uuid TEXT NOT NULL,
            recipient_public_key BLOB NOT NULL,
            wrapped_dek TEXT NOT NULL,
            expires_at INTEGER,
            created_at INTEGER NOT NULL,
            UNIQUE(secret_id, recipient_uuid)
        );

        CREATE INDEX IF NOT EXISTS idx_shared_items_recipient ON shared_items(recipient_uuid);
        CREATE INDEX IF NOT EXISTS idx_shared_items_secret ON shared_items(secret_id);
        "#,
    )
    .await?;

    Ok(())
}
