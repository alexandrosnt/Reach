use argon2::{
    password_hash::SaltString, Algorithm, Argon2, Params, PasswordHash, PasswordHasher,
    PasswordVerifier, Version,
};
use rand::rngs::OsRng;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MasterKeyError {
    #[error("Key derivation failed: {0}")]
    DerivationFailed(String),
    #[error("Verification failed: {0}")]
    VerificationFailed(String),
    #[error("Invalid parameters: {0}")]
    InvalidParams(String),
}

/// Derive a 32-byte encryption key from a master password and salt using Argon2id.
///
/// Parameters: t=3 iterations, m=65536 KiB memory, p=4 parallelism.
pub fn derive_key(password: &str, salt: &[u8]) -> Result<Vec<u8>, MasterKeyError> {
    let params = Params::new(65536, 3, 4, Some(32))
        .map_err(|e| MasterKeyError::InvalidParams(e.to_string()))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut output = vec![0u8; 32];
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut output)
        .map_err(|e| MasterKeyError::DerivationFailed(e.to_string()))?;

    Ok(output)
}

/// Generate a cryptographically secure random salt (16 bytes).
pub fn generate_salt() -> Vec<u8> {
    let salt_string = SaltString::generate(&mut OsRng);
    salt_string.as_str().as_bytes().to_vec()
}

/// Verify a master password against a stored Argon2id hash.
///
/// `verify_hash` should be a PHC-formatted Argon2 hash string.
pub fn verify_master(
    password: &str,
    _salt: &[u8],
    verify_hash: &[u8],
) -> Result<bool, MasterKeyError> {
    let hash_str = std::str::from_utf8(verify_hash)
        .map_err(|e| MasterKeyError::VerificationFailed(e.to_string()))?;
    let parsed_hash = PasswordHash::new(hash_str)
        .map_err(|e| MasterKeyError::VerificationFailed(e.to_string()))?;

    let params = Params::new(65536, 3, 4, Some(32))
        .map_err(|e| MasterKeyError::InvalidParams(e.to_string()))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    match argon2.verify_password(password.as_bytes(), &parsed_hash) {
        Ok(()) => Ok(true),
        Err(argon2::password_hash::Error::Password) => Ok(false),
        Err(e) => Err(MasterKeyError::VerificationFailed(e.to_string())),
    }
}

/// Hash a master password using Argon2id and return the PHC-formatted hash.
///
/// This is useful for storing the hash so it can be verified later.
pub fn hash_master_password(password: &str) -> Result<String, MasterKeyError> {
    let salt = SaltString::generate(&mut OsRng);
    let params = Params::new(65536, 3, 4, Some(32))
        .map_err(|e| MasterKeyError::InvalidParams(e.to_string()))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| MasterKeyError::DerivationFailed(e.to_string()))?;

    Ok(hash.to_string())
}
