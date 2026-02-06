use thiserror::Error;

#[derive(Error, Debug)]
pub enum VaultError {
    #[error("Vault is locked")]
    Locked,

    #[error("Identity not initialized")]
    IdentityNotInitialized,

    #[error("Identity already exists")]
    IdentityAlreadyExists,

    #[error("Vault not found: {0}")]
    NotFound(String),

    #[error("Vault already exists: {0}")]
    AlreadyExists(String),

    #[error("Vault not unlocked: {0}")]
    NotUnlocked(String),

    #[error("Secret not found: {0}")]
    SecretNotFound(String),

    #[error("Member not found: {0}")]
    MemberNotFound(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Encryption error: {0}")]
    EncryptionError(String),

    #[error("Decryption error: {0}")]
    DecryptionError(String),

    #[error("KDF error: {0}")]
    KdfError(String),

    #[error("Invalid key length: expected {expected}, got {got}")]
    InvalidKeyLength { expected: usize, got: usize },

    #[error("Invalid nonce length: expected {expected}, got {got}")]
    InvalidNonceLength { expected: usize, got: usize },

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("IO error: {0}")]
    IoError(String),

    #[error("Sync error: {0}")]
    SyncError(String),

    #[error("Keychain error: {0}")]
    KeychainError(String),

    #[error("Key derivation failed")]
    KeyDerivationFailed,

    #[error("Keychain key missing - your data exists but the encryption key was lost from the OS keychain. Use 'Import Identity' to restore from backup.")]
    KeychainKeyMissing,

    #[error("Access denied: {0}")]
    AccessDenied(String),

    #[error("Crypto error: {0}")]
    CryptoError(String),

    #[error("Invalid export file format")]
    InvalidExportFormat,

    #[error("Unsupported export version: {0}")]
    UnsupportedExportVersion(u16),
}

impl From<libsql::Error> for VaultError {
    fn from(e: libsql::Error) -> Self {
        VaultError::DatabaseError(e.to_string())
    }
}

impl From<serde_json::Error> for VaultError {
    fn from(e: serde_json::Error) -> Self {
        VaultError::SerializationError(e.to_string())
    }
}

impl From<std::io::Error> for VaultError {
    fn from(e: std::io::Error) -> Self {
        VaultError::IoError(e.to_string())
    }
}
