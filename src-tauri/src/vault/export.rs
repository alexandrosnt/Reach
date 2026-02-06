use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    XChaCha20Poly1305, XNonce,
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::vault::error::VaultError;
use crate::vault::types::AppSettings;

// ==================== FILE FORMAT ====================
// [8B magic "REACHBAK"][2B version u16 LE][32B salt][24B nonce][4B json_len u32 LE][...ciphertext...]

const MAGIC: &[u8; 8] = b"REACHBAK";
const FORMAT_VERSION: u16 = 1;
const HEADER_SIZE: usize = 8 + 2 + 32 + 24 + 4; // 70 bytes

// ==================== TYPES ====================

#[derive(Clone, Serialize, Deserialize)]
pub struct ExportBundle {
    pub version: u16,
    pub exported_at: i64,
    pub identity: ExportedIdentity,
    pub vaults: Vec<ExportedVault>,
    #[serde(default)]
    pub sync_config: Option<ExportedSyncConfig>,
    #[serde(default)]
    pub app_settings: Option<AppSettings>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ExportedIdentity {
    pub user_uuid: String,
    pub salt: String,
    pub encrypted_key: String,
    pub nonce: String,
    pub public_key: String,
    /// Raw secret key bytes (base64) — included in backup so it can be restored
    /// on a different machine. Protected by the export password encryption.
    #[serde(default)]
    pub secret_key: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ExportedVault {
    pub vault_id: String,
    pub name: String,
    pub vault_type: String,
    pub is_internal: bool,
    pub header: ExportedVaultHeader,
    pub secrets: Vec<ExportedSecret>,
    pub members: Vec<ExportedMember>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ExportedVaultHeader {
    pub id: String,
    pub name: String,
    pub salt: String,
    pub user_uuid: String,
    pub created_at: i64,
    pub vault_type: String,
    pub wrapped_master_dek: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ExportedSecret {
    pub id: String,
    pub name: String,
    pub category: String,
    pub nonce: String,
    pub ciphertext: String,
    pub wrapped_dek_json: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ExportedMember {
    pub user_uuid: String,
    pub public_key: String,
    pub wrapped_master_dek_json: String,
    pub role: String,
    pub added_at: i64,
    pub inviter_public_key: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ExportedSyncConfig {
    pub personal_sync_url: Option<String>,
    pub personal_sync_token: Option<String>,
    pub user_vaults: Vec<ExportedUserVault>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ExportedUserVault {
    pub id: String,
    pub name: String,
    pub vault_type: String,
    pub sync_url: Option<String>,
    pub sync_token: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupPreview {
    pub version: u16,
    pub exported_at: i64,
    pub user_uuid: String,
    pub vault_count: usize,
    pub secret_count: usize,
    pub has_sync_config: bool,
}

// ==================== SEAL / UNSEAL ====================

/// Derive a 32-byte key from the export password using Argon2id.
/// Uses strong parameters: 256MB memory, 4 iterations, 4 parallelism.
fn derive_export_key(password: &[u8], salt: &[u8; 32]) -> Result<Zeroizing<[u8; 32]>, VaultError> {
    let params = Params::new(262144, 4, 4, Some(32))
        .map_err(|e| VaultError::KdfError(e.to_string()))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut key = Zeroizing::new([0u8; 32]);
    argon2
        .hash_password_into(password, salt, key.as_mut())
        .map_err(|e| VaultError::KdfError(e.to_string()))?;

    // Domain separation via HKDF
    let hk = hkdf::Hkdf::<sha2::Sha256>::new(None, key.as_ref());
    let mut final_key = Zeroizing::new([0u8; 32]);
    hk.expand(b"reach-export-v1", final_key.as_mut())
        .map_err(|_| VaultError::KdfError("HKDF expand failed".to_string()))?;

    Ok(final_key)
}

/// Seal an ExportBundle into the binary `.reachbackup` format.
pub fn seal_bundle(bundle: &ExportBundle, password: &str) -> Result<Vec<u8>, VaultError> {
    if password.len() < 8 {
        return Err(VaultError::EncryptionError(
            "Export password must be at least 8 characters".to_string(),
        ));
    }

    // Serialize bundle to JSON
    let json = serde_json::to_vec(bundle)?;
    let json_len = json.len() as u32;

    // Generate salt and nonce
    let mut salt = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut salt);
    let mut nonce = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut nonce);

    // Derive key from password
    let key = derive_export_key(password.as_bytes(), &salt)?;

    // Encrypt JSON with XChaCha20-Poly1305
    let cipher = XChaCha20Poly1305::new(key.as_ref().into());
    let xnonce = XNonce::from_slice(&nonce);
    let ciphertext = cipher
        .encrypt(xnonce, json.as_ref())
        .map_err(|e| VaultError::EncryptionError(e.to_string()))?;

    // Build output: magic + version + salt + nonce + json_len + ciphertext
    let mut output = Vec::with_capacity(HEADER_SIZE + ciphertext.len());
    output.extend_from_slice(MAGIC);
    output.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    output.extend_from_slice(&salt);
    output.extend_from_slice(&nonce);
    output.extend_from_slice(&json_len.to_le_bytes());
    output.extend_from_slice(&ciphertext);

    Ok(output)
}

/// Unseal a `.reachbackup` file back into an ExportBundle.
pub fn unseal_bundle(data: &[u8], password: &str) -> Result<ExportBundle, VaultError> {
    if data.len() < HEADER_SIZE {
        return Err(VaultError::InvalidExportFormat);
    }

    // Validate magic
    if &data[0..8] != MAGIC {
        return Err(VaultError::InvalidExportFormat);
    }

    // Read version
    let version = u16::from_le_bytes([data[8], data[9]]);
    if version != FORMAT_VERSION {
        return Err(VaultError::UnsupportedExportVersion(version));
    }

    // Read salt
    let mut salt = [0u8; 32];
    salt.copy_from_slice(&data[10..42]);

    // Read nonce
    let mut nonce = [0u8; 24];
    nonce.copy_from_slice(&data[42..66]);

    // Read json_len (informational, we decrypt the rest)
    let _json_len = u32::from_le_bytes([data[66], data[67], data[68], data[69]]);

    // Ciphertext is everything after the header
    let ciphertext = &data[HEADER_SIZE..];

    // Derive key from password
    let key = derive_export_key(password.as_bytes(), &salt)?;

    // Decrypt
    let cipher = XChaCha20Poly1305::new(key.as_ref().into());
    let xnonce = XNonce::from_slice(&nonce);
    let plaintext = cipher
        .decrypt(xnonce, ciphertext)
        .map_err(|_| VaultError::DecryptionError("Invalid export password".to_string()))?;

    // Deserialize JSON
    let bundle: ExportBundle = serde_json::from_slice(&plaintext)?;

    Ok(bundle)
}

/// Preview a backup file — decrypt and return metadata without full import.
pub fn preview_bundle(data: &[u8], password: &str) -> Result<BackupPreview, VaultError> {
    let bundle = unseal_bundle(data, password)?;

    let secret_count: usize = bundle.vaults.iter().map(|v| v.secrets.len()).sum();

    Ok(BackupPreview {
        version: bundle.version,
        exported_at: bundle.exported_at,
        user_uuid: bundle.identity.user_uuid,
        vault_count: bundle.vaults.len(),
        secret_count,
        has_sync_config: bundle.sync_config.is_some(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine};

    #[test]
    fn test_seal_unseal_roundtrip() {
        let bundle = ExportBundle {
            version: FORMAT_VERSION,
            exported_at: 1700000000,
            identity: ExportedIdentity {
                user_uuid: "test-uuid".to_string(),
                salt: BASE64.encode([0xAA; 32]),
                encrypted_key: BASE64.encode([0xBB; 48]),
                nonce: BASE64.encode([0xCC; 24]),
                public_key: BASE64.encode([0xDD; 32]),
                secret_key: Some(BASE64.encode([0xEE; 32])),
            },
            vaults: vec![],
            sync_config: None,
            app_settings: None,
        };

        let password = "test-password-123";
        let sealed = seal_bundle(&bundle, password).unwrap();

        // Verify magic
        assert_eq!(&sealed[0..8], MAGIC);

        // Unseal
        let unsealed = unseal_bundle(&sealed, password).unwrap();
        assert_eq!(unsealed.identity.user_uuid, "test-uuid");
        assert_eq!(unsealed.exported_at, 1700000000);
    }

    #[test]
    fn test_wrong_password() {
        let bundle = ExportBundle {
            version: FORMAT_VERSION,
            exported_at: 0,
            identity: ExportedIdentity {
                user_uuid: "u".to_string(),
                salt: BASE64.encode([0; 32]),
                encrypted_key: BASE64.encode([0; 48]),
                nonce: BASE64.encode([0; 24]),
                public_key: BASE64.encode([0; 32]),
                secret_key: Some(BASE64.encode([0; 32])),
            },
            vaults: vec![],
            sync_config: None,
            app_settings: None,
        };

        let sealed = seal_bundle(&bundle, "correct-pass").unwrap();
        let result = unseal_bundle(&sealed, "wrong-pass");
        assert!(result.is_err());
    }

    #[test]
    fn test_short_password_rejected() {
        let bundle = ExportBundle {
            version: FORMAT_VERSION,
            exported_at: 0,
            identity: ExportedIdentity {
                user_uuid: "u".to_string(),
                salt: BASE64.encode([0; 32]),
                encrypted_key: BASE64.encode([0; 48]),
                nonce: BASE64.encode([0; 24]),
                public_key: BASE64.encode([0; 32]),
                secret_key: Some(BASE64.encode([0; 32])),
            },
            vaults: vec![],
            sync_config: None,
            app_settings: None,
        };

        let result = seal_bundle(&bundle, "short");
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_format() {
        let result = unseal_bundle(b"not a backup", "password123");
        assert!(result.is_err());
    }
}
