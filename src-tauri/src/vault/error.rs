use thiserror::Error;

#[derive(Error, Debug)]
pub enum VaultError {
    #[error("Vault is locked")]
    Locked,

    #[error("Identity not initialized")]
    IdentityNotInitialized,

    #[error("Identity already exists")]
    IdentityAlreadyExists,

    #[error("No password is set for this identity. Unlock via system keychain, then set a password in Settings > Security.")]
    PasswordNotSet,

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

    #[error("No OS keychain available ({0}). On Linux this usually means no Secret Service provider is running - start gnome-keyring-daemon or kwallet, or unlock the vault with your password instead.")]
    KeychainUnavailable(String),

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

/// Turn a libsql error message into something worth putting in front of a user.
///
/// Shared vaults talk to Turso over Hrana, and libsql renders a rejected
/// request as ``Hrana: `api error: `<http body>``` — the raw response body,
/// nested in two layers of backticks. That whole blob reached the vault
/// panel's toast, where it was both unreadable and far too long to fit, so the
/// user saw ``Database error: Hrana `ap`` and nothing else.
///
/// Takes the rendered message (`e.to_string()`) rather than the error itself:
/// `libsql::Error::Hrana` wraps a private box that cannot be built outside the
/// crate, and the string is all this needs anyway.
pub fn describe_db_error(raw: &str) -> String {
    // ``Hrana: `api error: `<body>``` — peel both wrappers to reach <body>.
    let body = raw
        .strip_prefix("Hrana: `")
        .and_then(|s| s.strip_suffix('`'))
        .map(|s| {
            s.strip_prefix("api error: `")
                .and_then(|b| b.strip_suffix('`'))
                .unwrap_or(s)
        })
        .unwrap_or(raw);

    // Turso answers with {"error": "..."} on most rejections.
    let message = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v.get("error").and_then(|m| m.as_str()).map(str::to_string))
        .unwrap_or_else(|| body.to_string());

    let lower = message.to_lowercase();
    if lower.contains("unauthor") || lower.contains("expired") || lower.contains("invalid token") {
        format!("{message} (the sync token is no longer accepted — re-authenticate the vault)")
    } else if lower.contains("not found") || lower.contains("does not exist") {
        format!("{message} (the remote database is gone or was renamed)")
    } else {
        message
    }
}

#[cfg(test)]
mod tests {
    use super::describe_db_error;

    #[test]
    fn unwraps_the_hrana_json_body() {
        let raw = r#"Hrana: `api error: `{"error":"Something broke"}``"#;
        assert_eq!(describe_db_error(raw), "Something broke");
    }

    #[test]
    fn hints_at_the_token_on_an_auth_failure() {
        let raw = r#"Hrana: `api error: `{"error":"Unauthorized: token expired"}``"#;
        let msg = describe_db_error(raw);
        assert!(msg.starts_with("Unauthorized: token expired"), "got: {msg}");
        assert!(msg.contains("re-authenticate"), "got: {msg}");
    }

    #[test]
    fn hints_at_the_database_when_it_is_gone() {
        let raw = r#"Hrana: `api error: `{"error":"database not found"}``"#;
        assert!(describe_db_error(raw).contains("remote database is gone"));
    }

    #[test]
    fn passes_through_a_non_json_body() {
        let raw = "Hrana: `http error: `connection refused``";
        assert_eq!(describe_db_error(raw), "http error: `connection refused`");
    }

    /// A local (non-Hrana) failure has no wrapper to strip and no JSON to
    /// parse; it must survive unchanged rather than being mangled.
    #[test]
    fn leaves_a_plain_sqlite_error_alone() {
        let raw = "SQLite failure: `no such table: secrets`";
        assert_eq!(describe_db_error(raw), raw);
    }
}
