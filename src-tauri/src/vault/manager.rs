use std::collections::HashMap;
use std::path::PathBuf;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use hkdf::Hkdf;
use libsql::{Connection, Database};
use rand::RngCore;
use secrecy::{ExposeSecret, SecretBox};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use x25519_dalek::{PublicKey, StaticSecret};

use crate::vault::crypto::{
    decrypt_secret, encrypt_secret, generate_dek, unwrap_dek, wrap_dek, wrap_dek_with_key, unwrap_dek_with_key,
};
use crate::vault::error::VaultError;
use crate::vault::schema::init_schema;
use crate::vault::sync::{create_replica, SyncConfig};
use crate::vault::types::{
    AppSettings, Dek, EncryptedPayload, InviteInfo, Kek, MemberInfo, MemberRole, ReceivedShare,
    SecretCategory, SecretMetadata, ShareItemResult, SharedItemInfo, UserIdentity, VaultHeader,
    VaultInfo, VaultType, WrappedDek,
};

/// Internal vault names for encrypted app data.
pub const SESSIONS_VAULT: &str = "__sessions__";
pub const CREDENTIALS_VAULT: &str = "__credentials__";
pub const FOLDERS_VAULT: &str = "__folders__";
pub const PLAYBOOKS_VAULT: &str = "__playbooks__";
pub const SETTINGS_VAULT: &str = "__settings__";

/// Vault connection state.
pub struct VaultConnection {
    pub db: Database,
    pub conn: Connection,
    pub header: VaultHeader,
    pub master_dek: Option<Dek>,
    pub sync_url: Option<String>,
    pub auth_token: Option<String>,
}

/// Stored vault reference (for reopening after restart).
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct StoredVaultRef {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) vault_type: String,
    pub(crate) sync_url: Option<String>,
    pub(crate) sync_token: Option<String>,
}

/// Stored identity (persisted to disk).
#[derive(Serialize, Deserialize)]
pub(crate) struct StoredIdentity {
    pub(crate) user_uuid: String,
    pub(crate) salt: String,
    pub(crate) encrypted_key: String,
    pub(crate) nonce: String,
    pub(crate) public_key: String,
    #[serde(default)]
    pub(crate) personal_sync_url: Option<String>,
    #[serde(default)]
    pub(crate) personal_sync_token: Option<String>,
    #[serde(default)]
    pub(crate) internal_vault_ids: HashMap<String, String>,
    /// User-created vaults (shared, private) - persisted for reopening after restart
    #[serde(default)]
    pub(crate) user_vaults: Vec<StoredVaultRef>,
}

/// Vault manager - core state machine for encrypted vault operations.
pub struct VaultManager {
    /// Open vault connections: vault_id -> VaultConnection
    vaults: HashMap<String, VaultConnection>,

    /// Vault name -> vault_id mapping for O(1) lookup by name
    vault_names: HashMap<String, String>,

    /// User's derived KEK (from master password or keychain)
    kek: Option<Kek>,

    /// User's X25519 identity keypair
    identity: Option<UserIdentity>,

    /// App data directory for local storage
    app_dir: PathBuf,

    /// User's UUID
    user_uuid: Option<String>,

    /// User's public key bytes
    identity_public_key: Option<[u8; 32]>,

    /// Personal sync URL (for cloud backup of ALL user data)
    personal_sync_url: Option<String>,

    /// Personal sync token
    personal_sync_token: Option<String>,

    /// Stored internal vault IDs for persistence
    internal_vault_ids: HashMap<String, String>,

    /// User-created vaults (shared, private) - for reopening after restart
    user_vaults: Vec<StoredVaultRef>,
}

impl VaultManager {
    /// Create a new vault manager.
    pub fn new(app_dir: PathBuf) -> Self {
        Self {
            vaults: HashMap::new(),
            vault_names: HashMap::new(),
            kek: None,
            identity: None,
            app_dir,
            user_uuid: None,
            identity_public_key: None,
            personal_sync_url: None,
            personal_sync_token: None,
            internal_vault_ids: HashMap::new(),
            user_vaults: Vec::new(),
        }
    }

    // ==================== IDENTITY ====================

    /// Initialize a new identity with password.
    pub async fn init_identity(&mut self, password: &str) -> Result<String, VaultError> {
        let identity_path = self.app_dir.join("vault_identity.json");
        if identity_path.exists() {
            return Err(VaultError::IdentityAlreadyExists);
        }

        tracing::info!("init_identity: starting");

        // Generate X25519 keypair
        let secret_key = StaticSecret::random_from_rng(rand::thread_rng());
        let public_key = PublicKey::from(&secret_key);

        // Generate user UUID
        let user_uuid = uuid::Uuid::new_v4().to_string();

        // Generate salt for KDF
        let salt = generate_salt();

        // Derive KEK from secret key (TLS-style)
        let kek = derive_kek_from_secret_key(secret_key.as_bytes(), &salt)?;
        self.kek = Some(kek);

        // Store identity
        self.identity = Some(UserIdentity::new(user_uuid.clone(), secret_key.clone()));
        self.user_uuid = Some(user_uuid.clone());
        self.identity_public_key = Some(public_key.to_bytes());

        // Store secret key in OS keychain
        if let Err(e) = store_key_in_keychain(&user_uuid, secret_key.as_bytes()) {
            tracing::warn!("Failed to store key in keychain: {}", e);
        }

        // Encrypt secret key with password for backup
        let password_kek = derive_kek_from_password(password.as_bytes(), &salt)?;
        let (_encrypted_key, _nonce) = encrypt_with_password(&password_kek, secret_key.as_bytes())?;

        self.save_identity(&salt).await?;
        tracing::info!("init_identity: saved identity file");

        // Create internal vaults
        self.ensure_internal_vaults().await?;
        tracing::info!("init_identity: created internal vaults");

        // Migrate old sessions.json if exists
        self.migrate_legacy_data().await?;
        tracing::info!("init_identity: complete");

        Ok(user_uuid)
    }

    /// Unlock vault with password.
    pub async fn unlock(&mut self, password: &str) -> Result<bool, VaultError> {
        let identity_path = self.app_dir.join("vault_identity.json");
        if !identity_path.exists() {
            return Err(VaultError::IdentityNotInitialized);
        }

        let data = tokio::fs::read_to_string(&identity_path).await?;
        let stored: StoredIdentity = serde_json::from_str(&data)?;

        // Decode salt
        let salt_bytes = BASE64
            .decode(&stored.salt)
            .map_err(|e| VaultError::SerializationError(e.to_string()))?;
        if salt_bytes.len() != 32 {
            return Err(VaultError::InvalidKeyLength {
                expected: 32,
                got: salt_bytes.len(),
            });
        }
        let mut salt = [0u8; 32];
        salt.copy_from_slice(&salt_bytes);

        // Decrypt secret key with password
        let encrypted_key = BASE64
            .decode(&stored.encrypted_key)
            .map_err(|e| VaultError::SerializationError(e.to_string()))?;
        let nonce = BASE64
            .decode(&stored.nonce)
            .map_err(|e| VaultError::SerializationError(e.to_string()))?;

        if nonce.len() != 24 {
            return Err(VaultError::InvalidNonceLength {
                expected: 24,
                got: nonce.len(),
            });
        }

        let password_kek = derive_kek_from_password(password.as_bytes(), &salt)?;
        let secret_key_bytes =
            decrypt_with_password(&password_kek, &encrypted_key, &nonce)?;

        if secret_key_bytes.len() != 32 {
            return Err(VaultError::InvalidKeyLength {
                expected: 32,
                got: secret_key_bytes.len(),
            });
        }

        let mut sk_array = [0u8; 32];
        sk_array.copy_from_slice(&secret_key_bytes);
        let secret_key = StaticSecret::from(sk_array);
        let public_key = PublicKey::from(&secret_key);

        // Derive KEK from secret key
        let kek = derive_kek_from_secret_key(secret_key.as_bytes(), &salt)?;
        self.kek = Some(kek);

        self.identity = Some(UserIdentity::new(stored.user_uuid.clone(), secret_key.clone()));
        self.user_uuid = Some(stored.user_uuid.clone());
        self.identity_public_key = Some(public_key.to_bytes());

        // Store in keychain for auto-unlock
        if let Err(e) = store_key_in_keychain(&stored.user_uuid, secret_key.as_bytes()) {
            tracing::warn!("Failed to store key in keychain: {}", e);
        }

        // Load personal sync config
        self.personal_sync_url = stored.personal_sync_url;
        self.personal_sync_token = stored.personal_sync_token;

        // Load stored internal vault IDs
        self.internal_vault_ids = stored.internal_vault_ids;

        // Load user-created vaults
        self.user_vaults = stored.user_vaults;

        // Open and unlock internal vaults (will use personal sync if configured)
        self.ensure_internal_vaults().await?;

        // Reopen user-created vaults (shared, private)
        self.reopen_user_vaults().await?;

        Ok(true)
    }

    /// Auto-unlock using OS keychain (TLS-style, no password needed).
    pub async fn auto_unlock(&mut self) -> Result<bool, VaultError> {
        let identity_path = self.app_dir.join("vault_identity.json");
        if !identity_path.exists() {
            return Err(VaultError::IdentityNotInitialized);
        }

        let data = tokio::fs::read_to_string(&identity_path).await?;
        let stored: StoredIdentity = serde_json::from_str(&data)?;

        // Get secret key from keychain
        let secret_key_bytes = get_key_from_keychain(&stored.user_uuid)?;

        if secret_key_bytes.len() != 32 {
            return Err(VaultError::InvalidKeyLength {
                expected: 32,
                got: secret_key_bytes.len(),
            });
        }

        let mut sk_array = [0u8; 32];
        sk_array.copy_from_slice(&secret_key_bytes);
        let secret_key = StaticSecret::from(sk_array);
        let public_key = PublicKey::from(&secret_key);

        // Decode salt
        let salt_bytes = BASE64
            .decode(&stored.salt)
            .map_err(|e| VaultError::SerializationError(e.to_string()))?;
        if salt_bytes.len() != 32 {
            return Err(VaultError::InvalidKeyLength {
                expected: 32,
                got: salt_bytes.len(),
            });
        }
        let mut salt = [0u8; 32];
        salt.copy_from_slice(&salt_bytes);

        // Derive KEK from secret key
        let kek = derive_kek_from_secret_key(secret_key.as_bytes(), &salt)?;
        self.kek = Some(kek);

        self.identity = Some(UserIdentity::new(stored.user_uuid.clone(), secret_key));
        self.user_uuid = Some(stored.user_uuid);
        self.identity_public_key = Some(public_key.to_bytes());

        // Load personal sync config
        self.personal_sync_url = stored.personal_sync_url;
        self.personal_sync_token = stored.personal_sync_token;

        // Load stored internal vault IDs
        self.internal_vault_ids = stored.internal_vault_ids;

        // Load user-created vaults
        self.user_vaults = stored.user_vaults;

        // Open internal vaults
        self.ensure_internal_vaults().await?;

        // Reopen user-created vaults (shared, private)
        self.reopen_user_vaults().await?;

        Ok(true)
    }

    /// Lock the vault manager.
    pub fn lock(&mut self) {
        self.kek = None;
        self.identity = None;
        for vault in self.vaults.values_mut() {
            vault.master_dek = None;
        }
    }

    /// Check if locked.
    pub fn is_locked(&self) -> bool {
        self.kek.is_none()
    }

    /// Check if identity exists.
    pub async fn has_identity(&self) -> bool {
        self.app_dir.join("vault_identity.json").exists()
    }

    /// Get public key (base64).
    pub fn get_public_key(&self) -> Option<String> {
        self.identity_public_key.map(|pk| BASE64.encode(pk))
    }

    /// Get user UUID.
    pub fn get_user_uuid(&self) -> Option<String> {
        self.user_uuid.clone()
    }

    /// Export identity for backup.
    pub fn export_identity(&self) -> Result<String, VaultError> {
        let identity = self.identity.as_ref().ok_or(VaultError::Locked)?;
        Ok(BASE64.encode(identity.secret_key().as_bytes()))
    }

    /// Reset vault - delete all local data.
    pub async fn reset(&mut self) -> Result<(), VaultError> {
        // Close all vaults
        self.vaults.clear();
        self.vault_names.clear();
        self.kek = None;
        self.identity = None;
        self.user_uuid = None;
        self.identity_public_key = None;
        self.personal_sync_url = None;
        self.personal_sync_token = None;
        self.internal_vault_ids.clear();

        // Delete identity file
        let identity_path = self.app_dir.join("vault_identity.json");
        if identity_path.exists() {
            tokio::fs::remove_file(&identity_path).await?;
        }

        // Delete vaults directory (best-effort on Windows due to file locks)
        let vault_dir = self.app_dir.join("vaults");
        if vault_dir.exists() {
            if let Err(e) = tokio::fs::remove_dir_all(&vault_dir).await {
                tracing::warn!("Could not delete vaults directory (files may be locked): {}", e);
                // Try deleting individual files instead
                if let Ok(mut entries) = tokio::fs::read_dir(&vault_dir).await {
                    while let Ok(Some(entry)) = entries.next_entry().await {
                        if let Err(e) = tokio::fs::remove_file(entry.path()).await {
                            tracing::warn!("Could not delete {}: {}", entry.path().display(), e);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    // ==================== PERSONAL SYNC ====================

    /// Set personal sync config (for cloud backup of ALL user data).
    pub async fn set_personal_sync_config(
        &mut self,
        sync_url: Option<String>,
        sync_token: Option<String>,
    ) -> Result<(), VaultError> {
        let was_sync_enabled =
            self.personal_sync_url.is_some() && self.personal_sync_token.is_some();

        self.personal_sync_url = sync_url.clone();
        self.personal_sync_token = sync_token.clone();

        // Read existing salt from identity file
        let identity_path = self.app_dir.join("vault_identity.json");
        if identity_path.exists() {
            let data = tokio::fs::read_to_string(&identity_path).await?;
            let stored: StoredIdentity = serde_json::from_str(&data)?;
            let salt_bytes = BASE64
                .decode(&stored.salt)
                .map_err(|e| VaultError::SerializationError(e.to_string()))?;
            if salt_bytes.len() == 32 {
                let mut salt = [0u8; 32];
                salt.copy_from_slice(&salt_bytes);
                self.save_identity(&salt).await?;
            }
        }

        // If sync is NOW enabled (wasn't before), migrate local data to cloud
        if !was_sync_enabled && sync_url.is_some() && sync_token.is_some() {
            tracing::info!("Personal sync enabled for first time - migrating local data to cloud");

            // Ensure we're unlocked to decrypt local secrets
            let _ = self.kek.as_ref().ok_or(VaultError::Locked)?;
            let internal_names =
                [SESSIONS_VAULT, CREDENTIALS_VAULT, FOLDERS_VAULT, PLAYBOOKS_VAULT, SETTINGS_VAULT];

            // Step 1: Read and decrypt all secrets from local vaults
            let mut all_secrets: Vec<(String, String, SecretCategory, SecretBox<Vec<u8>>)> =
                Vec::new();

            for name in internal_names {
                if let Some(vault_id) = self.vault_names.get(name).cloned() {
                    if let Some(vault) = self.vaults.get(&vault_id) {
                        if let Some(ref master_dek) = vault.master_dek {
                            // Read all secrets from this vault
                            let mut rows = vault
                                .conn
                                .query(
                                    "SELECT id, name, category, nonce, ciphertext, wrapped_dek FROM secrets",
                                    (),
                                )
                                .await?;

                            while let Some(row) = rows.next().await? {
                                let id: String = row.get(0)?;
                                let secret_name: String = row.get(1)?;
                                let category_str: String = row.get(2)?;
                                let nonce: Vec<u8> = row.get(3)?;
                                let ciphertext: Vec<u8> = row.get(4)?;
                                let wrapped_dek_json: String = row.get(5)?;

                                // Decrypt the secret
                                let wrapped_dek: WrappedDek =
                                    serde_json::from_str(&wrapped_dek_json)?;
                                let nonce_arr: [u8; 24] = nonce.try_into().map_err(|_| {
                                    VaultError::InvalidNonceLength {
                                        expected: 24,
                                        got: 0,
                                    }
                                })?;
                                let payload = EncryptedPayload {
                                    nonce: nonce_arr,
                                    ciphertext,
                                    wrapped_dek,
                                };

                                match decrypt_secret(master_dek, &payload) {
                                    Ok(plaintext) => {
                                        let category: SecretCategory = category_str
                                            .parse()
                                            .unwrap_or(SecretCategory::Custom("unknown".to_string()));
                                        // Use a prefixed ID to avoid collisions
                                        let prefixed_id = format!("{}_{}", name, id);
                                        all_secrets.push((prefixed_id, secret_name, category, plaintext));
                                    }
                                    Err(e) => {
                                        tracing::warn!(
                                            "Failed to decrypt secret {} from {}: {}",
                                            id,
                                            name,
                                            e
                                        );
                                    }
                                }
                            }

                            tracing::info!(
                                "Read {} secrets from local vault {}",
                                all_secrets.len(),
                                name
                            );
                        }
                    }
                }
            }

            // Step 2: Close all local vaults
            let vault_ids: Vec<String> = self.vaults.keys().cloned().collect();
            for vault_id in vault_ids {
                let _ = self.close_vault(&vault_id).await;
            }
            self.vault_names.clear();

            // Step 3: Create the unified vault in the cloud
            tracing::info!(
                "Creating unified personal vault in cloud with {} secrets to migrate",
                all_secrets.len()
            );
            self.ensure_internal_vaults().await?;

            // Step 4: Re-encrypt and store all secrets in unified vault
            let unified_vault_id = self
                .vault_names
                .get(SETTINGS_VAULT)
                .cloned()
                .ok_or_else(|| VaultError::NotFound("Unified vault not created".to_string()))?;

            for (id, name, category, plaintext) in all_secrets {
                match self
                    .create_secret_with_id(&unified_vault_id, &id, &name, category, plaintext)
                    .await
                {
                    Ok(_) => {
                        tracing::debug!("Migrated secret: {}", id);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to migrate secret {}: {}", id, e);
                    }
                }
            }

            tracing::info!("Migration to cloud complete");
            return Ok(());
        }

        // If sync config changed (but was already enabled), just reopen vaults
        if sync_url.is_some() && sync_token.is_some() {
            // Close existing vaults and reopen with new config
            let vault_ids: Vec<String> = self.vaults.keys().cloned().collect();
            for vault_id in vault_ids {
                let _ = self.close_vault(&vault_id).await;
            }
            self.vault_names.clear();
            self.ensure_internal_vaults().await?;
        }

        Ok(())
    }

    /// Get personal sync config.
    pub fn get_personal_sync_config(&self) -> (Option<String>, Option<String>) {
        (self.personal_sync_url.clone(), self.personal_sync_token.clone())
    }

    /// Ensure internal vaults exist and are open.
    /// Uses personal sync config if available for cloud backup of ALL user data.
    async fn ensure_internal_vaults(&mut self) -> Result<(), VaultError> {
        // Get personal sync config (if configured, ALL data syncs to cloud)
        let sync_url = self.personal_sync_url.clone();
        let sync_token = self.personal_sync_token.clone();
        let has_sync = sync_url.is_some() && sync_token.is_some();

        tracing::info!(
            "ensure_internal_vaults: has_sync={}, sync_url={:?}",
            has_sync,
            sync_url.as_ref().map(|_| "[redacted]")
        );

        if has_sync {
            tracing::info!("Personal sync enabled - using single vault for all internal data");

            // With personal sync, ALL internal vaults share ONE vault in the cloud
            // This avoids table conflicts since all data goes to the same database
            const UNIFIED_VAULT_NAME: &str = "__personal__";

            // Check if we already have the unified vault open
            if self.vault_names.get(UNIFIED_VAULT_NAME).is_some() {
                let vault_id = self.vault_names.get(UNIFIED_VAULT_NAME).unwrap().clone();
                tracing::info!(
                    "Unified vault {} already open, mapping all internal vaults",
                    vault_id
                );
                // Map all internal vault names to this unified vault
                for name in [
                    SESSIONS_VAULT,
                    CREDENTIALS_VAULT,
                    FOLDERS_VAULT,
                    PLAYBOOKS_VAULT,
                    SETTINGS_VAULT,
                ] {
                    self.vault_names.insert(name.to_string(), vault_id.clone());
                }
                // Verify vault is unlocked
                if let Some(vault) = self.vaults.get(&vault_id) {
                    tracing::info!("Unified vault unlocked: {}", vault.master_dek.is_some());
                }
                return Ok(());
            }

            // Check if we have a stored vault ID for the unified vault
            let mut vault_id = self.internal_vault_ids.get(UNIFIED_VAULT_NAME).cloned();
            tracing::info!("Stored unified vault ID: {:?}", vault_id);

            if let Some(ref id) = vault_id {
                tracing::info!("Opening stored unified vault with id {}", id);
                match self
                    .open_vault(id, sync_url.as_deref(), sync_token.as_deref())
                    .await
                {
                    Ok(info) => {
                        tracing::info!(
                            "Opened unified vault: {} (secrets: {})",
                            info.id,
                            info.secret_count
                        );
                        match self.unlock_vault(id).await {
                            Ok(_) => {
                                tracing::info!("Unified vault unlocked successfully");
                            }
                            Err(e) => {
                                tracing::error!("Failed to unlock unified vault: {}", e);
                                return Err(e);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to open stored unified vault: {}, will recreate",
                            e
                        );
                        vault_id = None; // Will create new
                    }
                }
            }

            if vault_id.is_none() {
                // Create the unified vault
                tracing::info!("Creating unified personal vault");
                let vault = self
                    .create_vault(
                        UNIFIED_VAULT_NAME,
                        VaultType::Private,
                        sync_url.as_deref(),
                        sync_token.as_deref(),
                    )
                    .await?;
                tracing::info!("Created unified vault: {}", vault.id);
                self.unlock_vault(&vault.id).await?;
                tracing::info!("Unlocked new unified vault");
                vault_id = Some(vault.id.clone());

                // Save the unified vault ID
                self.internal_vault_ids
                    .insert(UNIFIED_VAULT_NAME.to_string(), vault.id);
                self.save_identity_current().await?;
            }

            // Map all internal vault names to the unified vault
            let unified_id = vault_id.unwrap();
            tracing::info!(
                "Mapping all internal vault names to unified vault: {}",
                unified_id
            );
            for name in [
                SESSIONS_VAULT,
                CREDENTIALS_VAULT,
                FOLDERS_VAULT,
                PLAYBOOKS_VAULT,
                SETTINGS_VAULT,
            ] {
                self.vault_names.insert(name.to_string(), unified_id.clone());
            }
            tracing::info!(
                "vault_names after mapping: {:?}",
                self.vault_names.keys().collect::<Vec<_>>()
            );

            return Ok(());
        }

        // Local-only mode: each internal vault is separate
        let mut ids_changed = false;

        for name in [
            SESSIONS_VAULT,
            CREDENTIALS_VAULT,
            FOLDERS_VAULT,
            PLAYBOOKS_VAULT,
            SETTINGS_VAULT,
        ] {
            if self.vault_names.get(name).is_some() {
                continue; // Already open
            }

            // First check if we have a stored vault ID for this name
            // (e.g. from a backup import where multiple names map to one unified vault)
            if let Some(stored_id) = self.internal_vault_ids.get(name).cloned() {
                // Check if this vault is already open (shared with another internal name)
                if self.vaults.contains_key(&stored_id) {
                    tracing::info!("Mapping {} to already-open vault {}", name, stored_id);
                    self.vault_names.insert(name.to_string(), stored_id);
                    continue;
                }

                let vault_dir = self.app_dir.join("vaults");
                let db_path = vault_dir.join(format!("{}.db", stored_id));
                if db_path.exists() {
                    tracing::info!("Opening stored internal vault {} -> {}", name, stored_id);
                    self.open_vault(&stored_id, None, None).await?;
                    self.unlock_vault(&stored_id).await?;
                    continue;
                }
            }

            // Try to find in local files by scanning DB headers
            let vault_dir = self.app_dir.join("vaults");
            let mut found_id = None;

            if vault_dir.exists() {
                let mut entries = tokio::fs::read_dir(&vault_dir).await?;
                while let Some(entry) = entries.next_entry().await? {
                    let path = entry.path();
                    if path.extension().map_or(false, |e| e == "db") {
                        if let Some(stem) = path.file_stem() {
                            let vault_id = stem.to_string_lossy().to_string();
                            if let Ok(db) = create_replica(&path, None).await {
                                if let Ok(conn) = db.connect() {
                                    if let Ok(mut rows) =
                                        conn.query("SELECT name FROM vault_header LIMIT 1", ()).await
                                    {
                                        if let Ok(Some(row)) = rows.next().await {
                                            let db_name: String = row.get(0).unwrap_or_default();
                                            if db_name == name {
                                                found_id = Some(vault_id);
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if let Some(vault_id) = found_id {
                tracing::info!("Found internal vault {} in local files: {}", name, vault_id);
                self.open_vault(&vault_id, None, None).await?;
                self.unlock_vault(&vault_id).await?;
                self.internal_vault_ids.insert(name.to_string(), vault_id);
                ids_changed = true;
            } else {
                // Create new local vault
                tracing::info!("Creating new internal vault: {}", name);
                let vault = self.create_vault(name, VaultType::Private, None, None).await?;
                self.unlock_vault(&vault.id).await?;
                self.internal_vault_ids.insert(name.to_string(), vault.id);
                ids_changed = true;
            }
        }

        if ids_changed {
            self.save_identity_current().await?;
        }

        Ok(())
    }

    /// Reopen user-created vaults (shared, private) after unlock.
    async fn reopen_user_vaults(&mut self) -> Result<(), VaultError> {
        let vaults_to_open = self.user_vaults.clone();

        for vault_ref in vaults_to_open {
            // Skip if already open
            if self.vaults.contains_key(&vault_ref.id) {
                continue;
            }

            tracing::info!("Reopening user vault: {} ({})", vault_ref.name, vault_ref.id);

            match self
                .open_vault(
                    &vault_ref.id,
                    vault_ref.sync_url.as_deref(),
                    vault_ref.sync_token.as_deref(),
                )
                .await
            {
                Ok(_) => {
                    if let Err(e) = self.unlock_vault(&vault_ref.id).await {
                        tracing::warn!("Failed to unlock user vault {}: {}", vault_ref.name, e);
                    } else {
                        tracing::info!("User vault {} reopened and unlocked", vault_ref.name);
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to reopen user vault {}: {}", vault_ref.name, e);
                }
            }
        }

        Ok(())
    }

    /// Save identity to file.
    async fn save_identity(&self, salt: &[u8; 32]) -> Result<(), VaultError> {
        let identity = self.identity.as_ref().ok_or(VaultError::Locked)?;
        let public_key = identity.public_key;

        // For encrypted_key and nonce, we need the password-encrypted version
        // which should already be stored. Read existing if available.
        let identity_path = self.app_dir.join("vault_identity.json");
        let (encrypted_key, nonce) = if identity_path.exists() {
            let data = tokio::fs::read_to_string(&identity_path).await?;
            let stored: StoredIdentity = serde_json::from_str(&data)?;
            (stored.encrypted_key, stored.nonce)
        } else {
            // New identity - encrypted_key should be set during init
            (String::new(), String::new())
        };

        let stored = StoredIdentity {
            user_uuid: identity.uuid.clone(),
            salt: BASE64.encode(salt),
            encrypted_key,
            nonce,
            public_key: BASE64.encode(public_key.as_bytes()),
            personal_sync_url: self.personal_sync_url.clone(),
            personal_sync_token: self.personal_sync_token.clone(),
            internal_vault_ids: self.internal_vault_ids.clone(),
            user_vaults: self.user_vaults.clone(),
        };

        let json = serde_json::to_string_pretty(&stored)?;
        tokio::fs::create_dir_all(&self.app_dir).await?;
        tokio::fs::write(&identity_path, json).await?;

        Ok(())
    }

    /// Save identity with current salt.
    async fn save_identity_current(&self) -> Result<(), VaultError> {
        let identity_path = self.app_dir.join("vault_identity.json");
        if identity_path.exists() {
            let data = tokio::fs::read_to_string(&identity_path).await?;
            let stored: StoredIdentity = serde_json::from_str(&data)?;
            let salt_bytes = BASE64
                .decode(&stored.salt)
                .map_err(|e| VaultError::SerializationError(e.to_string()))?;
            if salt_bytes.len() == 32 {
                let mut salt = [0u8; 32];
                salt.copy_from_slice(&salt_bytes);
                self.save_identity(&salt).await?;
            }
        }
        Ok(())
    }

    /// Migrate legacy JSON data.
    async fn migrate_legacy_data(&mut self) -> Result<(), VaultError> {
        // Placeholder for migrating old sessions.json, etc.
        Ok(())
    }

    // ==================== VAULT MANAGEMENT ====================

    /// Create a new vault.
    pub async fn create_vault(
        &mut self,
        name: &str,
        vault_type: VaultType,
        sync_url: Option<&str>,
        sync_token: Option<&str>,
    ) -> Result<VaultInfo, VaultError> {
        let kek = self.kek.as_ref().ok_or(VaultError::Locked)?;
        let user_uuid = self
            .user_uuid
            .clone()
            .ok_or(VaultError::IdentityNotInitialized)?;

        let vault_id = uuid::Uuid::new_v4().to_string();
        let salt = generate_salt();
        let master_dek = generate_dek();
        let wrapped_master_dek = wrap_dek(kek, &master_dek)?;

        let header = VaultHeader {
            id: vault_id.clone(),
            name: name.to_string(),
            salt,
            user_uuid,
            created_at: now_timestamp(),
            vault_type: vault_type.clone(),
        };

        // Create vault directory
        let vault_dir = self.app_dir.join("vaults");
        tokio::fs::create_dir_all(&vault_dir).await?;
        let db_path = vault_dir.join(format!("{}.db", vault_id));

        let sync_config = match (sync_url, sync_token) {
            (Some(url), Some(token)) => Some(SyncConfig {
                sync_url: url.to_string(),
                auth_token: token.to_string(),
            }),
            _ => None,
        };

        let db = create_replica(&db_path, sync_config.as_ref()).await?;
        let conn = db.connect().map_err(|e| VaultError::DatabaseError(e.to_string()))?;

        init_schema(&conn).await?;

        // Store header
        let wrapped_dek_json = serde_json::to_string(&wrapped_master_dek)?;
        let vault_type_json = serde_json::to_string(&header.vault_type)?;

        conn.execute(
            "INSERT INTO vault_header (id, name, salt, user_uuid, created_at, vault_type, wrapped_master_dek) VALUES (?, ?, ?, ?, ?, ?, ?)",
            (
                header.id.as_str(),
                header.name.as_str(),
                header.salt.as_slice(),
                header.user_uuid.as_str(),
                header.created_at,
                vault_type_json,
                wrapped_dek_json,
            ),
        ).await?;

        // Initial sync to push to remote
        if sync_config.is_some() {
            tracing::info!("Initial sync for new vault: {}", vault_id);
            if let Err(e) = db.sync().await {
                tracing::warn!("Initial sync failed: {}", e);
            }
        }

        let member_count = match &vault_type {
            VaultType::Private => None,
            VaultType::Shared { members } => Some(members.len()),
        };

        // Store in memory (O(1))
        self.vault_names.insert(name.to_string(), vault_id.clone());
        self.vaults.insert(
            vault_id.clone(),
            VaultConnection {
                db,
                conn,
                header,
                master_dek: Some(master_dek),
                sync_url: sync_url.map(|s| s.to_string()),
                auth_token: sync_token.map(|s| s.to_string()),
            },
        );

        let vault_type_str = match &vault_type {
            VaultType::Private => "private".to_string(),
            VaultType::Shared { .. } => "shared".to_string(),
        };

        // Save user-created vaults (not internal ones) for persistence across restarts
        if !name.starts_with("__") {
            tracing::info!("Saving user vault {} to identity for persistence", name);
            self.user_vaults.push(StoredVaultRef {
                id: vault_id.clone(),
                name: name.to_string(),
                vault_type: vault_type_str.clone(),
                sync_url: sync_url.map(|s| s.to_string()),
                sync_token: sync_token.map(|s| s.to_string()),
            });
            // Persist to disk
            if let Err(e) = self.save_identity_current().await {
                tracing::warn!("Failed to persist user vault: {}", e);
            }
        }

        Ok(VaultInfo {
            id: vault_id,
            name: name.to_string(),
            vault_type: vault_type_str,
            member_count,
            secret_count: 0,
            last_sync: None,
        })
    }

    /// Open an existing vault.
    pub async fn open_vault(
        &mut self,
        vault_id: &str,
        sync_url: Option<&str>,
        token: Option<&str>,
    ) -> Result<VaultInfo, VaultError> {
        if self.vaults.contains_key(vault_id) {
            // Already open, return info
            let vault = self.vaults.get(vault_id).unwrap();
            let member_count = match &vault.header.vault_type {
                VaultType::Private => None,
                VaultType::Shared { members } => Some(members.len()),
            };
            return Ok(VaultInfo {
                id: vault_id.to_string(),
                name: vault.header.name.clone(),
                vault_type: match &vault.header.vault_type {
                    VaultType::Private => "private".to_string(),
                    VaultType::Shared { .. } => "shared".to_string(),
                },
                member_count,
                secret_count: 0,
                last_sync: None,
            });
        }

        let vault_dir = self.app_dir.join("vaults");
        let db_path = vault_dir.join(format!("{}.db", vault_id));

        let sync_config = match (sync_url, token) {
            (Some(url), Some(t)) => Some(SyncConfig {
                sync_url: url.to_string(),
                auth_token: t.to_string(),
            }),
            _ => None,
        };

        // For local-only vaults, check if file exists
        // For synced vaults, we connect to remote directly (no local file needed)
        if sync_config.is_none() && !db_path.exists() {
            return Err(VaultError::NotFound(vault_id.to_string()));
        }

        let db = create_replica(&db_path, sync_config.as_ref()).await?;

        // Sync to pull latest data from remote
        if sync_config.is_some() {
            tracing::info!("Syncing vault on open: {}", vault_id);
            if let Err(e) = db.sync().await {
                tracing::warn!("Sync on open failed: {}", e);
            }
        }

        let conn = db.connect().map_err(|e| VaultError::DatabaseError(e.to_string()))?;

        // Load header
        let mut rows = conn
            .query(
                "SELECT id, name, salt, user_uuid, created_at, vault_type FROM vault_header LIMIT 1",
                (),
            )
            .await?;

        let row = rows
            .next()
            .await?
            .ok_or_else(|| VaultError::NotFound(vault_id.to_string()))?;

        let id: String = row.get(0)?;
        let name: String = row.get(1)?;
        let salt_blob: Vec<u8> = row.get(2)?;
        let user_uuid: String = row.get(3)?;
        let created_at: i64 = row.get(4)?;
        let vault_type_json: String = row.get(5)?;

        if salt_blob.len() != 32 {
            return Err(VaultError::InvalidKeyLength {
                expected: 32,
                got: salt_blob.len(),
            });
        }
        let mut salt = [0u8; 32];
        salt.copy_from_slice(&salt_blob);

        let vault_type: VaultType = serde_json::from_str(&vault_type_json)?;

        let header = VaultHeader {
            id,
            name: name.clone(),
            salt,
            user_uuid,
            created_at,
            vault_type: vault_type.clone(),
        };

        let member_count = match &vault_type {
            VaultType::Private => None,
            VaultType::Shared { members } => Some(members.len()),
        };

        // Count secrets
        let mut count_rows = conn.query("SELECT COUNT(*) FROM secrets", ()).await?;
        let secret_count: i64 = if let Some(row) = count_rows.next().await? {
            row.get(0)?
        } else {
            0
        };

        // Store in memory (O(1))
        self.vault_names.insert(name.clone(), vault_id.to_string());
        self.vaults.insert(
            vault_id.to_string(),
            VaultConnection {
                db,
                conn,
                header,
                master_dek: None,
                sync_url: sync_url.map(|s| s.to_string()),
                auth_token: token.map(|s| s.to_string()),
            },
        );

        Ok(VaultInfo {
            id: vault_id.to_string(),
            name,
            vault_type: match vault_type {
                VaultType::Private => "private".to_string(),
                VaultType::Shared { .. } => "shared".to_string(),
            },
            member_count,
            secret_count: secret_count as usize,
            last_sync: None,
        })
    }

    /// Unlock a vault (derive master DEK).
    /// For vault owners: unwrap using KEK from vault_header.
    /// For invitees: unwrap using X25519 shared secret from vault_members.
    pub async fn unlock_vault(&mut self, vault_id: &str) -> Result<(), VaultError> {
        let kek = self.kek.as_ref().ok_or(VaultError::Locked)?;
        let identity = self.identity.as_ref().ok_or(VaultError::Locked)?;
        let my_uuid = self.user_uuid.clone().ok_or(VaultError::IdentityNotInitialized)?;

        let vault = self
            .vaults
            .get_mut(vault_id)
            .ok_or_else(|| VaultError::NotFound(vault_id.to_string()))?;

        if vault.master_dek.is_some() {
            return Ok(()); // Already unlocked
        }

        // Check if we're the vault owner
        let mut header_rows = vault
            .conn
            .query("SELECT user_uuid, wrapped_master_dek FROM vault_header LIMIT 1", ())
            .await?;
        let header_row = header_rows
            .next()
            .await?
            .ok_or_else(|| VaultError::NotFound(vault_id.to_string()))?;
        let owner_uuid: String = header_row.get(0)?;
        let owner_wrapped_dek_json: String = header_row.get(1)?;

        if owner_uuid == my_uuid {
            // We're the owner - unwrap with KEK
            tracing::info!("Unlocking vault {} as owner", vault_id);
            let wrapped_dek: WrappedDek = serde_json::from_str(&owner_wrapped_dek_json)?;
            let master_dek = unwrap_dek(kek, &wrapped_dek)?;
            vault.master_dek = Some(master_dek);
            return Ok(());
        }

        // We're not the owner - check vault_members table
        tracing::info!("Unlocking vault {} as member (uuid: {})", vault_id, my_uuid);
        let mut member_rows = vault
            .conn
            .query(
                "SELECT wrapped_master_dek, inviter_public_key FROM vault_members WHERE user_uuid = ?",
                [my_uuid.as_str()],
            )
            .await?;

        let member_row = member_rows.next().await?.ok_or_else(|| {
            tracing::error!("User {} not found in vault_members for vault {}", my_uuid, vault_id);
            VaultError::AccessDenied(format!("You are not a member of this vault: {}", vault_id))
        })?;

        let member_wrapped_dek_json: String = member_row.get(0)?;
        let inviter_pk_b64: String = member_row.get(1)?;

        // Decode inviter's public key
        let inviter_pk_bytes = BASE64
            .decode(&inviter_pk_b64)
            .map_err(|e| VaultError::SerializationError(format!("Invalid inviter public key: {}", e)))?;

        if inviter_pk_bytes.len() != 32 {
            return Err(VaultError::InvalidKeyLength {
                expected: 32,
                got: inviter_pk_bytes.len(),
            });
        }

        let mut inviter_pk_arr = [0u8; 32];
        inviter_pk_arr.copy_from_slice(&inviter_pk_bytes);
        let inviter_pk = x25519_dalek::PublicKey::from(inviter_pk_arr);

        // Compute shared secret using our private key and inviter's public key
        let shared_secret = identity.secret_key().diffie_hellman(&inviter_pk);

        // Derive the same wrapping key used during invite
        let mut wrapping_key = [0u8; 32];
        let hk = hkdf::Hkdf::<sha2::Sha256>::new(None, shared_secret.as_bytes());
        hk.expand(b"vault-member-dek", &mut wrapping_key)
            .map_err(|_| VaultError::CryptoError("HKDF expand failed".to_string()))?;

        // Unwrap the DEK
        let wrapped_dek: WrappedDek = serde_json::from_str(&member_wrapped_dek_json)?;
        let master_dek = unwrap_dek_with_key(&wrapping_key, &wrapped_dek)?;
        vault.master_dek = Some(master_dek);

        tracing::info!("Successfully unlocked vault {} as member", vault_id);
        Ok(())
    }

    /// Lock a vault.
    pub fn lock_vault(&mut self, vault_id: &str) {
        if let Some(vault) = self.vaults.get_mut(vault_id) {
            vault.master_dek = None;
        }
    }

    /// Close a vault.
    pub async fn close_vault(&mut self, vault_id: &str) -> Result<(), VaultError> {
        if self.vaults.contains_key(vault_id) {
            // Remove from name mapping
            self.vault_names.retain(|_, v| v != vault_id);
        }
        self.vaults.remove(vault_id);
        Ok(())
    }

    /// Delete a vault.
    pub async fn delete_vault(&mut self, vault_id: &str) -> Result<(), VaultError> {
        self.close_vault(vault_id).await?;

        // Remove from user_vaults if present
        self.user_vaults.retain(|v| v.id != vault_id);
        if let Err(e) = self.save_identity_current().await {
            tracing::warn!("Failed to persist user vault removal: {}", e);
        }

        // Delete local file
        let vault_dir = self.app_dir.join("vaults");
        let db_path = vault_dir.join(format!("{}.db", vault_id));
        if db_path.exists() {
            tokio::fs::remove_file(&db_path).await?;
        }

        Ok(())
    }

    /// List all vaults.
    pub async fn list_vaults(&self) -> Result<Vec<VaultInfo>, VaultError> {
        let mut vaults = Vec::new();
        for (vault_id, vault) in &self.vaults {
            // Skip internal vaults
            if vault.header.name.starts_with("__") {
                continue;
            }

            let member_count = match &vault.header.vault_type {
                VaultType::Private => None,
                VaultType::Shared { members } => Some(members.len()),
            };

            // Count secrets
            let mut count_rows = vault.conn.query("SELECT COUNT(*) FROM secrets", ()).await?;
            let secret_count: i64 = if let Some(row) = count_rows.next().await? {
                row.get(0)?
            } else {
                0
            };

            vaults.push(VaultInfo {
                id: vault_id.clone(),
                name: vault.header.name.clone(),
                vault_type: match &vault.header.vault_type {
                    VaultType::Private => "private".to_string(),
                    VaultType::Shared { .. } => "shared".to_string(),
                },
                member_count,
                secret_count: secret_count as usize,
                last_sync: None,
            });
        }
        Ok(vaults)
    }

    /// Sync vault with remote.
    pub async fn sync_vault(&mut self, vault_id: &str) -> Result<(), VaultError> {
        let vault = self
            .vaults
            .get(vault_id)
            .ok_or_else(|| VaultError::NotFound(vault_id.to_string()))?;
        if let Err(e) = vault.db.sync().await {
            tracing::warn!("Sync failed: {}", e);
        }
        Ok(())
    }

    // ==================== SECRETS ====================

    /// Get internal vault ID helper.
    fn get_internal_vault_id(&self, name: &str) -> Result<String, VaultError> {
        self.vault_names
            .get(name)
            .cloned()
            .ok_or_else(|| VaultError::NotFound(name.to_string()))
    }

    pub fn sessions_vault_id(&self) -> Result<String, VaultError> {
        self.get_internal_vault_id(SESSIONS_VAULT)
    }

    pub fn credentials_vault_id(&self) -> Result<String, VaultError> {
        self.get_internal_vault_id(CREDENTIALS_VAULT)
    }

    pub fn folders_vault_id(&self) -> Result<String, VaultError> {
        self.get_internal_vault_id(FOLDERS_VAULT)
    }

    pub fn playbooks_vault_id(&self) -> Result<String, VaultError> {
        self.get_internal_vault_id(PLAYBOOKS_VAULT)
    }

    pub fn settings_vault_id(&self) -> Result<String, VaultError> {
        self.get_internal_vault_id(SETTINGS_VAULT)
    }

    /// Create a secret (O(1) by ID).
    pub async fn create_secret(
        &self,
        vault_id: &str,
        name: &str,
        category: SecretCategory,
        plaintext: SecretBox<Vec<u8>>,
    ) -> Result<String, VaultError> {
        let secret_id = uuid::Uuid::new_v4().to_string();
        self.create_secret_with_id(vault_id, &secret_id, name, category, plaintext)
            .await?;
        Ok(secret_id)
    }

    /// Create a secret with specific ID.
    pub async fn create_secret_with_id(
        &self,
        vault_id: &str,
        secret_id: &str,
        name: &str,
        category: SecretCategory,
        plaintext: SecretBox<Vec<u8>>,
    ) -> Result<(), VaultError> {
        let vault = self
            .vaults
            .get(vault_id)
            .ok_or_else(|| VaultError::NotFound(vault_id.to_string()))?;
        let master_dek = vault
            .master_dek
            .as_ref()
            .ok_or_else(|| VaultError::NotUnlocked(vault_id.to_string()))?;

        let payload = encrypt_secret(master_dek, plaintext.expose_secret())?;
        let now = now_timestamp();
        let wrapped_dek_json = serde_json::to_string(&payload.wrapped_dek)?;

        vault.conn.execute(
            "INSERT OR REPLACE INTO secrets (id, name, category, nonce, ciphertext, wrapped_dek, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            (
                secret_id,
                name,
                category.to_string(),
                payload.nonce.as_slice(),
                payload.ciphertext.as_slice(),
                wrapped_dek_json,
                now,
                now,
            ),
        ).await?;

        // Auto-sync if this is a synced vault
        if vault.sync_url.is_some() {
            if let Err(e) = vault.db.sync().await {
                tracing::warn!("Auto-sync after create failed: {}", e);
            }
        }

        Ok(())
    }

    /// Read a secret (O(1) by ID).
    pub async fn read_secret(
        &self,
        vault_id: &str,
        secret_id: &str,
    ) -> Result<SecretBox<Vec<u8>>, VaultError> {
        let vault = self
            .vaults
            .get(vault_id)
            .ok_or_else(|| VaultError::NotFound(vault_id.to_string()))?;
        let master_dek = vault
            .master_dek
            .as_ref()
            .ok_or_else(|| VaultError::NotUnlocked(vault_id.to_string()))?;

        let mut rows = vault
            .conn
            .query(
                "SELECT nonce, ciphertext, wrapped_dek FROM secrets WHERE id = ?",
                [secret_id],
            )
            .await?;

        let row = rows
            .next()
            .await?
            .ok_or_else(|| VaultError::SecretNotFound(secret_id.to_string()))?;

        let nonce: Vec<u8> = row.get(0)?;
        let ciphertext: Vec<u8> = row.get(1)?;
        let wrapped_dek_json: String = row.get(2)?;

        if nonce.len() != 24 {
            return Err(VaultError::InvalidNonceLength {
                expected: 24,
                got: nonce.len(),
            });
        }
        let mut nonce_arr = [0u8; 24];
        nonce_arr.copy_from_slice(&nonce);

        let wrapped_dek: WrappedDek = serde_json::from_str(&wrapped_dek_json)?;

        let payload = EncryptedPayload {
            nonce: nonce_arr,
            ciphertext,
            wrapped_dek,
        };

        decrypt_secret(master_dek, &payload)
    }

    /// Update a secret.
    pub async fn update_secret(
        &self,
        vault_id: &str,
        secret_id: &str,
        plaintext: SecretBox<Vec<u8>>,
    ) -> Result<(), VaultError> {
        let vault = self
            .vaults
            .get(vault_id)
            .ok_or_else(|| VaultError::NotFound(vault_id.to_string()))?;
        let master_dek = vault
            .master_dek
            .as_ref()
            .ok_or_else(|| VaultError::NotUnlocked(vault_id.to_string()))?;

        let payload = encrypt_secret(master_dek, plaintext.expose_secret())?;
        let now = now_timestamp();
        let wrapped_dek_json = serde_json::to_string(&payload.wrapped_dek)?;

        vault
            .conn
            .execute(
                "UPDATE secrets SET nonce = ?, ciphertext = ?, wrapped_dek = ?, updated_at = ? WHERE id = ?",
                (
                    payload.nonce.as_slice(),
                    payload.ciphertext.as_slice(),
                    wrapped_dek_json,
                    now,
                    secret_id,
                ),
            )
            .await?;

        // Auto-sync if this is a synced vault
        if vault.sync_url.is_some() {
            if let Err(e) = vault.db.sync().await {
                tracing::warn!("Auto-sync after update failed: {}", e);
            }
        }

        Ok(())
    }

    /// Delete a secret.
    pub async fn delete_secret(&self, vault_id: &str, secret_id: &str) -> Result<(), VaultError> {
        let vault = self
            .vaults
            .get(vault_id)
            .ok_or_else(|| VaultError::NotFound(vault_id.to_string()))?;

        vault
            .conn
            .execute("DELETE FROM secrets WHERE id = ?", [secret_id])
            .await?;

        // Auto-sync
        if vault.sync_url.is_some() {
            if let Err(e) = vault.db.sync().await {
                tracing::warn!("Auto-sync after delete failed: {}", e);
            }
        }

        Ok(())
    }

    /// Check if a secret exists.
    pub async fn secret_exists(&self, vault_id: &str, secret_id: &str) -> bool {
        let Some(vault) = self.vaults.get(vault_id) else {
            return false;
        };

        match vault
            .conn
            .query("SELECT 1 FROM secrets WHERE id = ?", [secret_id])
            .await
        {
            Ok(mut rows) => matches!(rows.next().await, Ok(Some(_))),
            Err(_) => false,
        }
    }

    /// Get vault ID by name.
    pub fn get_vault_id_by_name(&self, name: &str) -> Option<String> {
        self.vault_names.get(name).cloned()
    }

    /// List secrets (metadata only).
    pub async fn list_secrets(&self, vault_id: &str) -> Result<Vec<SecretMetadata>, VaultError> {
        let vault = self
            .vaults
            .get(vault_id)
            .ok_or_else(|| VaultError::NotFound(vault_id.to_string()))?;

        let mut rows = vault
            .conn
            .query(
                "SELECT id, name, category, created_at, updated_at FROM secrets",
                (),
            )
            .await?;

        let mut secrets = Vec::new();
        while let Some(row) = rows.next().await? {
            secrets.push(SecretMetadata {
                id: row.get(0)?,
                name: row.get(1)?,
                category: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            });
        }

        Ok(secrets)
    }

    // ==================== SHARING ====================

    /// Invite member to shared vault.
    pub async fn invite_member(
        &self,
        vault_id: &str,
        invitee_public_key: &[u8; 32],
        invitee_uuid: &str,
        role: MemberRole,
    ) -> Result<InviteInfo, VaultError> {
        let identity = self.identity.as_ref().ok_or(VaultError::Locked)?;
        let vault = self
            .vaults
            .get(vault_id)
            .ok_or_else(|| VaultError::NotFound(vault_id.to_string()))?;

        let master_dek = vault.master_dek.as_ref().ok_or(VaultError::Locked)?;

        // Re-wrap master DEK for invitee using X25519
        let invitee_pk = x25519_dalek::PublicKey::from(*invitee_public_key);
        let shared_secret = identity.secret_key().diffie_hellman(&invitee_pk);

        // Derive wrapping key from shared secret
        let mut wrapping_key = [0u8; 32];
        let hk = hkdf::Hkdf::<sha2::Sha256>::new(None, shared_secret.as_bytes());
        hk.expand(b"vault-member-dek", &mut wrapping_key)
            .map_err(|_| VaultError::CryptoError("HKDF expand failed".to_string()))?;

        // Wrap DEK with derived key
        let wrapped_for_invitee = wrap_dek_with_key(&wrapping_key, master_dek)?;

        // Store in vault_members table
        let wrapped_dek_json = serde_json::to_string(&wrapped_for_invitee)?;
        let role_str = match role {
            MemberRole::Owner => "owner",
            MemberRole::Admin => "admin",
            MemberRole::Member => "member",
            MemberRole::ReadOnly => "readonly",
        };
        let inviter_pk_b64 = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            identity.public_key.as_bytes(),
        );

        vault
            .conn
            .execute(
                "INSERT OR REPLACE INTO vault_members (user_uuid, public_key, wrapped_master_dek, role, added_at, inviter_public_key) VALUES (?, ?, ?, ?, ?, ?)",
                (
                    invitee_uuid,
                    invitee_public_key.as_slice(),
                    wrapped_dek_json.as_str(),
                    role_str,
                    now_timestamp(),
                    inviter_pk_b64.as_str(),
                ),
            )
            .await?;

        // Sync to push member to remote
        if let Err(e) = vault.db.sync().await {
            tracing::warn!("Failed to sync after adding member: {}", e);
        }

        let sync_url = vault.sync_url.clone().unwrap_or_default();
        let token = vault.auth_token.clone().unwrap_or_default();

        tracing::info!("Invited {} to vault {} with role {}", invitee_uuid, vault_id, role_str);

        Ok(InviteInfo {
            vault_id: vault_id.to_string(),
            sync_url,
            token,
        })
    }

    /// Accept invite to shared vault.
    pub async fn accept_invite(
        &mut self,
        sync_url: &str,
        token: &str,
    ) -> Result<VaultInfo, VaultError> {
        // Generate new vault ID for local tracking
        let vault_id = uuid::Uuid::new_v4().to_string();

        // Open the vault
        let vault_info = self.open_vault(&vault_id, Some(sync_url), Some(token)).await?;

        // Unlock it
        self.unlock_vault(&vault_id).await?;

        // Save to user_vaults for persistence across restarts
        tracing::info!("Saving accepted invite vault {} for persistence", vault_info.name);
        self.user_vaults.push(StoredVaultRef {
            id: vault_id.clone(),
            name: vault_info.name.clone(),
            vault_type: vault_info.vault_type.clone(),
            sync_url: Some(sync_url.to_string()),
            sync_token: Some(token.to_string()),
        });

        // Persist to identity file
        if let Err(e) = self.save_identity_current().await {
            tracing::warn!("Failed to persist accepted invite: {}", e);
        }

        Ok(vault_info)
    }

    /// Remove member from vault.
    pub async fn remove_member(&self, vault_id: &str, user_uuid: &str) -> Result<(), VaultError> {
        let vault = self
            .vaults
            .get(vault_id)
            .ok_or_else(|| VaultError::NotFound(vault_id.to_string()))?;

        vault
            .conn
            .execute(
                "DELETE FROM vault_members WHERE user_uuid = ?",
                [user_uuid],
            )
            .await?;

        Ok(())
    }

    /// List vault members.
    pub async fn list_members(&self, vault_id: &str) -> Result<Vec<MemberInfo>, VaultError> {
        let vault = self
            .vaults
            .get(vault_id)
            .ok_or_else(|| VaultError::NotFound(vault_id.to_string()))?;

        let mut rows = vault
            .conn
            .query(
                "SELECT user_uuid, public_key, role, added_at FROM vault_members",
                (),
            )
            .await?;

        let mut members = Vec::new();
        while let Some(row) = rows.next().await? {
            let pk_blob: Vec<u8> = row.get(1)?;
            members.push(MemberInfo {
                user_uuid: row.get(0)?,
                public_key: BASE64.encode(&pk_blob),
                role: row.get(2)?,
                added_at: row.get(3)?,
            });
        }

        Ok(members)
    }

    // ==================== SHARE INDIVIDUAL ITEMS ====================

    /// Share a specific secret with another user.
    pub async fn share_item(
        &self,
        vault_id: &str,
        secret_id: &str,
        recipient_uuid: &str,
        recipient_public_key: &[u8; 32],
        expires_in_hours: Option<u64>,
    ) -> Result<ShareItemResult, VaultError> {
        let vault = self
            .vaults
            .get(vault_id)
            .ok_or_else(|| VaultError::NotFound(vault_id.to_string()))?;

        let share_id = uuid::Uuid::new_v4().to_string();
        let now = now_timestamp();
        let expires_at = expires_in_hours.map(|h| now + (h as i64 * 3600));

        // TODO: Re-wrap DEK with recipient's public key

        vault.conn.execute(
            "INSERT INTO shared_items (id, secret_id, recipient_uuid, recipient_public_key, wrapped_dek, expires_at, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
            (
                share_id.as_str(),
                secret_id,
                recipient_uuid,
                recipient_public_key.as_slice(),
                "{}",  // TODO: Wrapped DEK
                expires_at,
                now,
            ),
        ).await?;

        Ok(ShareItemResult {
            share_id,
            secret_id: secret_id.to_string(),
            recipient_uuid: recipient_uuid.to_string(),
            sync_url: vault.sync_url.clone(),
            expires_at,
        })
    }

    /// List shared items from a vault.
    pub async fn list_shared_items(&self, vault_id: &str) -> Result<Vec<SharedItemInfo>, VaultError> {
        let vault = self
            .vaults
            .get(vault_id)
            .ok_or_else(|| VaultError::NotFound(vault_id.to_string()))?;

        let mut rows = vault
            .conn
            .query(
                "SELECT id, secret_id, recipient_uuid, expires_at, created_at FROM shared_items",
                (),
            )
            .await?;

        let mut items = Vec::new();
        while let Some(row) = rows.next().await? {
            items.push(SharedItemInfo {
                id: row.get(0)?,
                secret_id: row.get(1)?,
                recipient_uuid: row.get(2)?,
                expires_at: row.get(3)?,
                created_at: row.get(4)?,
            });
        }

        Ok(items)
    }

    /// Revoke a shared item.
    pub async fn revoke_shared_item(&self, vault_id: &str, share_id: &str) -> Result<(), VaultError> {
        let vault = self
            .vaults
            .get(vault_id)
            .ok_or_else(|| VaultError::NotFound(vault_id.to_string()))?;

        vault
            .conn
            .execute("DELETE FROM shared_items WHERE id = ?", [share_id])
            .await?;

        Ok(())
    }

    /// Accept a shared item.
    pub async fn accept_shared_item(
        &self,
        _source_vault_id: &str,
        share_id: &str,
        _target_vault_id: &str,
    ) -> Result<String, VaultError> {
        // TODO: Implement copying shared secret to target vault
        Ok(share_id.to_string())
    }

    /// List items shared with me.
    pub async fn list_received_shares(&self) -> Result<Vec<ReceivedShare>, VaultError> {
        // TODO: Implement querying across vaults
        Ok(Vec::new())
    }

    // ==================== APP SETTINGS ====================

    /// Import identity from backup.
    pub async fn import_identity(&mut self, secret_key_b64: &str) -> Result<String, VaultError> {
        let identity_path = self.app_dir.join("vault_identity.json");
        if identity_path.exists() {
            return Err(VaultError::IdentityAlreadyExists);
        }

        let secret_key_bytes = BASE64
            .decode(secret_key_b64)
            .map_err(|e| VaultError::SerializationError(e.to_string()))?;

        if secret_key_bytes.len() != 32 {
            return Err(VaultError::InvalidKeyLength {
                expected: 32,
                got: secret_key_bytes.len(),
            });
        }

        let mut sk_array = [0u8; 32];
        sk_array.copy_from_slice(&secret_key_bytes);
        let secret_key = StaticSecret::from(sk_array);
        let public_key = PublicKey::from(&secret_key);

        let user_uuid = uuid::Uuid::new_v4().to_string();
        let salt = generate_salt();

        let kek = derive_kek_from_secret_key(secret_key.as_bytes(), &salt)?;
        self.kek = Some(kek);

        self.identity = Some(UserIdentity::new(user_uuid.clone(), secret_key.clone()));
        self.user_uuid = Some(user_uuid.clone());
        self.identity_public_key = Some(public_key.to_bytes());

        // Store in keychain
        if let Err(e) = store_key_in_keychain(&user_uuid, secret_key.as_bytes()) {
            tracing::warn!("Failed to store key in keychain: {}", e);
        }

        self.save_identity(&salt).await?;

        // Create internal vaults
        self.ensure_internal_vaults().await?;

        Ok(user_uuid)
    }

    /// Get app settings from encrypted __settings__ vault.
    pub async fn get_settings(&self) -> Result<AppSettings, VaultError> {
        let vault_id = match self.settings_vault_id() {
            Ok(id) => id,
            Err(e) => {
                tracing::warn!(
                    "Settings vault not available: {}. vault_names keys: {:?}",
                    e,
                    self.vault_names.keys().collect::<Vec<_>>()
                );
                return Ok(AppSettings::default());
            }
        };

        // Settings stored under key "app_settings"
        let settings_key = "app_settings";

        let vault = match self.vaults.get(&vault_id) {
            Some(v) => v,
            None => {
                tracing::warn!(
                    "Settings vault {} not in vaults HashMap. Available: {:?}",
                    vault_id,
                    self.vaults.keys().collect::<Vec<_>>()
                );
                return Ok(AppSettings::default());
            }
        };

        // Check if vault is unlocked
        if vault.master_dek.is_none() {
            tracing::warn!("Settings vault {} is not unlocked", vault_id);
            return Ok(AppSettings::default());
        }

        let mut rows = vault
            .conn
            .query("SELECT id FROM secrets WHERE name = ?", [settings_key])
            .await?;

        if let Some(row) = rows.next().await? {
            let secret_id: String = row.get(0)?;
            let data = self.read_secret(&vault_id, &secret_id).await?;
            let settings: AppSettings = serde_json::from_slice(data.expose_secret())?;
            Ok(settings)
        } else {
            Ok(AppSettings::default())
        }
    }

    /// Save app settings to encrypted __settings__ vault.
    pub async fn save_settings(&self, settings: &AppSettings) -> Result<(), VaultError> {
        let vault_id = self.settings_vault_id().map_err(|e| {
            tracing::error!(
                "Cannot save settings: vault not available. vault_names keys: {:?}",
                self.vault_names.keys().collect::<Vec<_>>()
            );
            e
        })?;
        let settings_key = "app_settings";

        let vault = self.vaults.get(&vault_id).ok_or_else(|| {
            tracing::error!(
                "Cannot save settings: vault {} not in vaults HashMap. Available: {:?}",
                vault_id,
                self.vaults.keys().collect::<Vec<_>>()
            );
            VaultError::NotFound(vault_id.clone())
        })?;

        // Check if vault is unlocked
        if vault.master_dek.is_none() {
            tracing::error!("Cannot save settings: vault {} is not unlocked", vault_id);
            return Err(VaultError::NotUnlocked(vault_id));
        }

        let data = serde_json::to_vec(settings)?;
        let secret_data = SecretBox::new(Box::new(data));

        // Check if exists
        let mut rows = vault
            .conn
            .query("SELECT id FROM secrets WHERE name = ?", [settings_key])
            .await?;

        if let Some(row) = rows.next().await? {
            let secret_id: String = row.get(0)?;
            self.update_secret(&vault_id, &secret_id, secret_data)
                .await?;
        } else {
            self.create_secret(&vault_id, settings_key, SecretCategory::Setting, secret_data)
                .await?;
        }

        tracing::info!("Settings saved successfully to vault {}", vault_id);
        Ok(())
    }

    /// Get Turso config.
    pub async fn get_turso_config(&self) -> Result<(Option<String>, Option<String>), VaultError> {
        let settings = self.get_settings().await?;
        Ok((settings.turso_org, settings.turso_api_token))
    }

    /// Set Turso config.
    pub async fn set_turso_config(
        &self,
        org: Option<String>,
        token: Option<String>,
    ) -> Result<(), VaultError> {
        let mut settings = self.get_settings().await.unwrap_or_default();
        settings.turso_org = org;
        settings.turso_api_token = token;
        settings.sync_enabled = settings.turso_org.is_some() && settings.turso_api_token.is_some();
        self.save_settings(&settings).await
    }

    // ==================== FULL BACKUP EXPORT/IMPORT ====================

    /// Export a full backup to file. Rust handles file I/O directly.
    #[tracing::instrument(skip(self, export_password))]
    pub async fn export_full_backup(
        &self,
        export_password: &str,
        file_path: &str,
    ) -> Result<(), VaultError> {
        use crate::vault::export::*;

        let _kek = self.kek.as_ref().ok_or(VaultError::Locked)?;

        // Read identity file
        let identity_path = self.app_dir.join("vault_identity.json");
        let identity_data = tokio::fs::read_to_string(&identity_path).await?;
        let stored: StoredIdentity = serde_json::from_str(&identity_data)?;

        // Include raw secret key in backup — on the target machine, the OS
        // keychain won't have it, so we need it in the bundle.  The bundle
        // itself is sealed with the export password (XChaCha20-Poly1305).
        let secret_key_b64 = {
            let id = self.identity.as_ref().ok_or(VaultError::Locked)?;
            BASE64.encode(id.secret_key().as_bytes())
        };

        let identity = ExportedIdentity {
            user_uuid: stored.user_uuid.clone(),
            salt: stored.salt.clone(),
            encrypted_key: stored.encrypted_key.clone(),
            nonce: stored.nonce.clone(),
            public_key: stored.public_key.clone(),
            secret_key: Some(secret_key_b64),
        };

        // Export all vaults (internal + user)
        let mut exported_vaults = Vec::new();

        for (vault_id, vault) in &self.vaults {
            let is_internal = vault.header.name.starts_with("__");
            let vault_type_json = serde_json::to_string(&vault.header.vault_type)?;

            // Read wrapped_master_dek from header
            let mut header_rows = vault
                .conn
                .query("SELECT wrapped_master_dek FROM vault_header LIMIT 1", ())
                .await?;
            let wrapped_master_dek = if let Some(row) = header_rows.next().await? {
                let dek: String = row.get(0)?;
                dek
            } else {
                String::new()
            };

            let header = ExportedVaultHeader {
                id: vault.header.id.clone(),
                name: vault.header.name.clone(),
                salt: BASE64.encode(vault.header.salt),
                user_uuid: vault.header.user_uuid.clone(),
                created_at: vault.header.created_at,
                vault_type: vault_type_json.clone(),
                wrapped_master_dek,
            };

            // Export secrets (ciphertext only — never decrypted)
            let mut secrets = Vec::new();
            let mut secret_rows = vault
                .conn
                .query(
                    "SELECT id, name, category, nonce, ciphertext, wrapped_dek, created_at, updated_at FROM secrets",
                    (),
                )
                .await?;

            while let Some(row) = secret_rows.next().await? {
                let nonce_blob: Vec<u8> = row.get(3)?;
                let ct_blob: Vec<u8> = row.get(4)?;
                secrets.push(ExportedSecret {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    category: row.get(2)?,
                    nonce: BASE64.encode(&nonce_blob),
                    ciphertext: BASE64.encode(&ct_blob),
                    wrapped_dek_json: row.get(5)?,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                });
            }

            // Export members
            let mut members = Vec::new();
            let mut member_rows = vault
                .conn
                .query(
                    "SELECT user_uuid, public_key, wrapped_master_dek, role, added_at, inviter_public_key FROM vault_members",
                    (),
                )
                .await?;

            while let Some(row) = member_rows.next().await? {
                let pk_blob: Vec<u8> = row.get(1)?;
                let inviter_pk: Option<String> = row.get(5).ok();
                members.push(ExportedMember {
                    user_uuid: row.get(0)?,
                    public_key: BASE64.encode(&pk_blob),
                    wrapped_master_dek_json: row.get(2)?,
                    role: row.get(3)?,
                    added_at: row.get(4)?,
                    inviter_public_key: inviter_pk,
                });
            }

            exported_vaults.push(ExportedVault {
                vault_id: vault_id.clone(),
                name: vault.header.name.clone(),
                vault_type: vault_type_json,
                is_internal,
                header,
                secrets,
                members,
            });
        }

        // Sync config
        let sync_config = if stored.personal_sync_url.is_some() || !stored.user_vaults.is_empty() {
            Some(ExportedSyncConfig {
                personal_sync_url: stored.personal_sync_url,
                personal_sync_token: stored.personal_sync_token,
                user_vaults: stored
                    .user_vaults
                    .iter()
                    .map(|v| ExportedUserVault {
                        id: v.id.clone(),
                        name: v.name.clone(),
                        vault_type: v.vault_type.clone(),
                        sync_url: v.sync_url.clone(),
                        sync_token: v.sync_token.clone(),
                    })
                    .collect(),
            })
        } else {
            None
        };

        // Read current app settings (decrypted) to include in backup
        let app_settings = match self.get_settings().await {
            Ok(s) => {
                tracing::info!("Including app settings in backup (turso_org={:?}, sync_enabled={})", s.turso_org.as_ref().map(|_| "[set]"), s.sync_enabled);
                Some(s)
            }
            Err(e) => {
                tracing::warn!("Could not read app settings for backup: {}", e);
                None
            }
        };

        let bundle = ExportBundle {
            version: 1,
            exported_at: now_timestamp(),
            identity,
            vaults: exported_vaults,
            sync_config,
            app_settings,
        };

        let sealed = seal_bundle(&bundle, export_password)?;
        tokio::fs::write(file_path, sealed).await?;

        tracing::info!("Full backup exported to {}", file_path);
        Ok(())
    }

    /// Preview a backup file — validate and return metadata.
    #[tracing::instrument(skip(self, export_password))]
    pub async fn preview_backup(
        &self,
        file_path: &str,
        export_password: &str,
    ) -> Result<crate::vault::export::BackupPreview, VaultError> {
        let data = tokio::fs::read(file_path).await?;
        crate::vault::export::preview_bundle(&data, export_password)
    }

    /// Import a full backup.
    ///
    /// Simple approach:
    /// 1. Decrypt bundle with export password
    /// 2. Extract the secret key from the bundle
    /// 3. Store secret key in OS keychain
    /// 4. Write vault_identity.json with FULL sync config
    /// 5. Create local vault DBs as fallback
    /// 6. Return — frontend restarts the app
    ///
    /// On restart, `auto_unlock()` reads identity → gets key from keychain →
    /// connects to Turso (if sync enabled) → everything works.
    #[tracing::instrument(skip(self, export_password, _master_password))]
    pub async fn import_full_backup(
        &mut self,
        file_path: &str,
        export_password: &str,
        _master_password: &str,
    ) -> Result<String, VaultError> {
        use crate::vault::export::*;

        // Step 1: Decrypt bundle
        let data = tokio::fs::read(file_path).await?;
        let bundle = unseal_bundle(&data, export_password)?;
        tracing::info!("Bundle decrypted: {} vaults, sync={}", bundle.vaults.len(), bundle.sync_config.is_some());

        // Step 2: Extract secret key from bundle
        let secret_key_b64 = bundle.identity.secret_key.as_ref().ok_or_else(|| {
            VaultError::EncryptionError(
                "Backup does not contain the secret key. Please re-export from the source machine.".to_string(),
            )
        })?;
        let secret_key_bytes = BASE64
            .decode(secret_key_b64)
            .map_err(|e| VaultError::SerializationError(e.to_string()))?;
        if secret_key_bytes.len() != 32 {
            return Err(VaultError::InvalidKeyLength {
                expected: 32,
                got: secret_key_bytes.len(),
            });
        }
        tracing::info!("Secret key extracted from bundle (32 bytes)");

        // Step 3: Clear in-memory state
        self.vaults.clear();
        self.vault_names.clear();
        self.kek = None;
        self.identity = None;
        self.user_uuid = None;
        self.identity_public_key = None;
        self.personal_sync_url = None;
        self.personal_sync_token = None;
        self.internal_vault_ids.clear();
        self.user_vaults.clear();

        // Delete old identity file
        let identity_path = self.app_dir.join("vault_identity.json");
        if identity_path.exists() {
            tokio::fs::remove_file(&identity_path).await?;
        }

        // Step 4: Store secret key in OS keychain (for auto_unlock on restart)
        store_key_in_keychain(&bundle.identity.user_uuid, &secret_key_bytes)?;
        tracing::info!("Secret key stored in OS keychain for user {}", bundle.identity.user_uuid);

        // Step 5: Build internal_vault_ids from bundle
        let mut internal_vault_ids = HashMap::new();
        let mut unified_vault_id: Option<String> = None;
        for exported_vault in &bundle.vaults {
            if exported_vault.is_internal {
                if exported_vault.name == "__personal__" {
                    unified_vault_id = Some(exported_vault.vault_id.clone());
                }
                internal_vault_ids
                    .insert(exported_vault.name.clone(), exported_vault.vault_id.clone());
            }
        }
        // If backup had unified vault, map all internal names to it
        if let Some(ref uid) = unified_vault_id {
            for name in [
                SESSIONS_VAULT,
                CREDENTIALS_VAULT,
                FOLDERS_VAULT,
                PLAYBOOKS_VAULT,
                SETTINGS_VAULT,
            ] {
                internal_vault_ids.insert(name.to_string(), uid.clone());
            }
        }

        // Step 6: Build user_vaults WITH sync URLs
        let user_vaults: Vec<StoredVaultRef> = if let Some(ref sc) = bundle.sync_config {
            sc.user_vaults
                .iter()
                .map(|v| StoredVaultRef {
                    id: v.id.clone(),
                    name: v.name.clone(),
                    vault_type: v.vault_type.clone(),
                    sync_url: v.sync_url.clone(),
                    sync_token: v.sync_token.clone(),
                })
                .collect()
        } else {
            bundle
                .vaults
                .iter()
                .filter(|v| !v.is_internal)
                .map(|v| StoredVaultRef {
                    id: v.vault_id.clone(),
                    name: v.name.clone(),
                    vault_type: v.vault_type.clone(),
                    sync_url: None,
                    sync_token: None,
                })
                .collect()
        };

        // Step 7: Write vault_identity.json with FULL sync config
        let stored = StoredIdentity {
            user_uuid: bundle.identity.user_uuid.clone(),
            salt: bundle.identity.salt.clone(),
            encrypted_key: bundle.identity.encrypted_key.clone(),
            nonce: bundle.identity.nonce.clone(),
            public_key: bundle.identity.public_key.clone(),
            personal_sync_url: bundle.sync_config.as_ref().and_then(|sc| sc.personal_sync_url.clone()),
            personal_sync_token: bundle.sync_config.as_ref().and_then(|sc| sc.personal_sync_token.clone()),
            internal_vault_ids,
            user_vaults,
        };

        let identity_json = serde_json::to_string_pretty(&stored)?;
        tokio::fs::create_dir_all(&self.app_dir).await?;
        tokio::fs::write(&identity_path, &identity_json).await?;
        tracing::info!("Identity file written with full sync config");

        // Step 8: Create local vault DBs from bundle (fallback for offline / non-sync)
        let vault_dir = self.app_dir.join("vaults");
        tokio::fs::create_dir_all(&vault_dir).await?;

        for exported_vault in &bundle.vaults {
            let db_path = vault_dir.join(format!("{}.db", exported_vault.vault_id));

            // If DB file already exists, remove it
            if db_path.exists() {
                let _ = tokio::fs::remove_file(&db_path).await;
                let _ = tokio::fs::remove_file(vault_dir.join(format!("{}.db-wal", exported_vault.vault_id))).await;
                let _ = tokio::fs::remove_file(vault_dir.join(format!("{}.db-shm", exported_vault.vault_id))).await;
            }

            let db = crate::vault::sync::create_replica(&db_path, None).await?;
            let conn = db.connect().map_err(|e| VaultError::DatabaseError(e.to_string()))?;
            crate::vault::schema::init_schema(&conn).await?;

            // Insert vault header
            let vault_salt = BASE64
                .decode(&exported_vault.header.salt)
                .map_err(|e| VaultError::SerializationError(e.to_string()))?;

            conn.execute(
                "INSERT INTO vault_header (id, name, salt, user_uuid, created_at, vault_type, wrapped_master_dek) VALUES (?, ?, ?, ?, ?, ?, ?)",
                (
                    exported_vault.header.id.as_str(),
                    exported_vault.header.name.as_str(),
                    vault_salt.as_slice(),
                    exported_vault.header.user_uuid.as_str(),
                    exported_vault.header.created_at,
                    exported_vault.header.vault_type.as_str(),
                    exported_vault.header.wrapped_master_dek.as_str(),
                ),
            ).await?;

            for secret in &exported_vault.secrets {
                let secret_nonce = BASE64
                    .decode(&secret.nonce)
                    .map_err(|e| VaultError::SerializationError(e.to_string()))?;
                let ct_bytes = BASE64
                    .decode(&secret.ciphertext)
                    .map_err(|e| VaultError::SerializationError(e.to_string()))?;

                conn.execute(
                    "INSERT INTO secrets (id, name, category, nonce, ciphertext, wrapped_dek, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                    (
                        secret.id.as_str(),
                        secret.name.as_str(),
                        secret.category.as_str(),
                        secret_nonce.as_slice(),
                        ct_bytes.as_slice(),
                        secret.wrapped_dek_json.as_str(),
                        secret.created_at,
                        secret.updated_at,
                    ),
                ).await?;
            }

            for member in &exported_vault.members {
                let pk_bytes = BASE64
                    .decode(&member.public_key)
                    .map_err(|e| VaultError::SerializationError(e.to_string()))?;
                let inviter_pk = member.inviter_public_key.as_deref().unwrap_or("");

                conn.execute(
                    "INSERT INTO vault_members (user_uuid, public_key, wrapped_master_dek, role, added_at, inviter_public_key) VALUES (?, ?, ?, ?, ?, ?)",
                    (
                        member.user_uuid.as_str(),
                        pk_bytes.as_slice(),
                        member.wrapped_master_dek_json.as_str(),
                        member.role.as_str(),
                        member.added_at,
                        inviter_pk,
                    ),
                ).await?;
            }

            tracing::info!(
                "Restored vault DB: {} ({}, {} secrets, {} members)",
                exported_vault.vault_id,
                exported_vault.name,
                exported_vault.secrets.len(),
                exported_vault.members.len(),
            );
        }

        tracing::info!("Backup import complete. App should restart now.");
        Ok(bundle.identity.user_uuid)
    }
}

// ==================== HELPER FUNCTIONS ====================

/// Generate a random 32-byte salt.
fn generate_salt() -> [u8; 32] {
    let mut salt = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut salt);
    salt
}

/// Get current timestamp.
fn now_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

/// Derive KEK from X25519 secret key using HKDF (TLS-style).
fn derive_kek_from_secret_key(secret_key: &[u8], salt: &[u8; 32]) -> Result<Kek, VaultError> {
    let hk = Hkdf::<Sha256>::new(Some(salt), secret_key);
    let mut kek_bytes = [0u8; 32];
    hk.expand(b"reach-vault-kek", &mut kek_bytes)
        .map_err(|_| VaultError::KeyDerivationFailed)?;
    Ok(Kek::new(kek_bytes))
}

/// Derive KEK from password using Argon2id.
fn derive_kek_from_password(password: &[u8], salt: &[u8; 32]) -> Result<Kek, VaultError> {
    use argon2::{Algorithm, Argon2, Params, Version};

    let params = Params::new(65536, 3, 4, Some(32)).map_err(|e| VaultError::KdfError(e.to_string()))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut kek_bytes = [0u8; 32];
    argon2
        .hash_password_into(password, salt, &mut kek_bytes)
        .map_err(|e| VaultError::KdfError(e.to_string()))?;

    Ok(Kek::new(kek_bytes))
}

/// Encrypt data with password-derived KEK.
fn encrypt_with_password(kek: &Kek, plaintext: &[u8]) -> Result<(String, String), VaultError> {
    use chacha20poly1305::{aead::Aead, KeyInit, XChaCha20Poly1305, XNonce};

    let cipher = XChaCha20Poly1305::new(kek.expose().into());
    let mut nonce = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut nonce);
    let xnonce = XNonce::from_slice(&nonce);

    let ciphertext = cipher
        .encrypt(xnonce, plaintext)
        .map_err(|e| VaultError::EncryptionError(e.to_string()))?;

    Ok((BASE64.encode(&ciphertext), BASE64.encode(&nonce)))
}

/// Decrypt data with password-derived KEK.
fn decrypt_with_password(kek: &Kek, ciphertext: &[u8], nonce: &[u8]) -> Result<Vec<u8>, VaultError> {
    use chacha20poly1305::{aead::Aead, KeyInit, XChaCha20Poly1305, XNonce};

    let cipher = XChaCha20Poly1305::new(kek.expose().into());
    let xnonce = XNonce::from_slice(nonce);

    cipher
        .decrypt(xnonce, ciphertext)
        .map_err(|e| VaultError::DecryptionError(e.to_string()))
}

/// Store key in OS keychain.
fn store_key_in_keychain(user_uuid: &str, key: &[u8]) -> Result<(), VaultError> {
    let entry = keyring::Entry::new("reach-vault", user_uuid)
        .map_err(|e| VaultError::KeychainError(e.to_string()))?;
    entry
        .set_password(&BASE64.encode(key))
        .map_err(|e| VaultError::KeychainError(e.to_string()))?;
    Ok(())
}

/// Get key from OS keychain.
fn get_key_from_keychain(user_uuid: &str) -> Result<Vec<u8>, VaultError> {
    let entry = keyring::Entry::new("reach-vault", user_uuid)
        .map_err(|e| VaultError::KeychainError(e.to_string()))?;
    let password = entry.get_password().map_err(|e| {
        if e.to_string().contains("No matching entry") {
            VaultError::KeychainKeyMissing
        } else {
            VaultError::KeychainError(e.to_string())
        }
    })?;
    BASE64
        .decode(&password)
        .map_err(|e| VaultError::SerializationError(e.to_string()))
}
