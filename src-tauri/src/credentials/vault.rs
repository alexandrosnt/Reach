use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use rand::RngCore;
use thiserror::Error;

use crate::state::EncryptedCredential;

#[derive(Debug, Error)]
pub enum VaultError {
    #[error("Encryption failed: {0}")]
    EncryptionFailed(String),
    #[error("Decryption failed: {0}")]
    DecryptionFailed(String),
    #[error("Invalid key length: expected 32 bytes, got {0}")]
    InvalidKeyLength(usize),
    #[error("Invalid nonce: {0}")]
    InvalidNonce(String),
    #[error("Base64 decode error: {0}")]
    Base64Error(String),
}

/// Encrypt plaintext bytes using AES-256-GCM with the provided 32-byte key.
///
/// Returns an `EncryptedCredential` containing the base64-encoded nonce and ciphertext.
pub fn encrypt(key: &[u8], plaintext: &[u8]) -> Result<EncryptedCredential, VaultError> {
    if key.len() != 32 {
        return Err(VaultError::InvalidKeyLength(key.len()));
    }

    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| VaultError::EncryptionFailed(e.to_string()))?;

    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| VaultError::EncryptionFailed(e.to_string()))?;

    Ok(EncryptedCredential {
        nonce: BASE64.encode(nonce_bytes),
        ciphertext: BASE64.encode(ciphertext),
    })
}

/// Decrypt an `EncryptedCredential` using AES-256-GCM with the provided 32-byte key.
///
/// Returns the original plaintext bytes.
pub fn decrypt(key: &[u8], credential: &EncryptedCredential) -> Result<Vec<u8>, VaultError> {
    if key.len() != 32 {
        return Err(VaultError::InvalidKeyLength(key.len()));
    }

    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| VaultError::DecryptionFailed(e.to_string()))?;

    let nonce_bytes = BASE64
        .decode(&credential.nonce)
        .map_err(|e| VaultError::Base64Error(e.to_string()))?;

    if nonce_bytes.len() != 12 {
        return Err(VaultError::InvalidNonce(format!(
            "expected 12 bytes, got {}",
            nonce_bytes.len()
        )));
    }

    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = BASE64
        .decode(&credential.ciphertext)
        .map_err(|e| VaultError::Base64Error(e.to_string()))?;

    let plaintext = cipher
        .decrypt(nonce, ciphertext.as_ref())
        .map_err(|e| VaultError::DecryptionFailed(e.to_string()))?;

    Ok(plaintext)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = [0xABu8; 32];
        let plaintext = b"super secret password";

        let encrypted = encrypt(&key, plaintext).expect("encryption should succeed");
        let decrypted = decrypt(&key, &encrypted).expect("decryption should succeed");

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_wrong_key_fails() {
        let key = [0xABu8; 32];
        let wrong_key = [0xCDu8; 32];
        let plaintext = b"super secret password";

        let encrypted = encrypt(&key, plaintext).expect("encryption should succeed");
        let result = decrypt(&wrong_key, &encrypted);

        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_key_length() {
        let short_key = [0xABu8; 16];
        let plaintext = b"test";

        let result = encrypt(&short_key, plaintext);
        assert!(matches!(result, Err(VaultError::InvalidKeyLength(16))));
    }
}
