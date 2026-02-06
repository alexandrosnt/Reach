use chacha20poly1305::{
    aead::{Aead, KeyInit},
    XChaCha20Poly1305, XNonce,
};
use rand::RngCore;
use secrecy::SecretBox;
use zeroize::Zeroizing;

use crate::vault::error::VaultError;
use crate::vault::types::{Dek, EncryptedPayload, Kek, WrappedDek};

/// Generate a random 32-byte DEK.
pub fn generate_dek() -> Dek {
    let mut key = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut key);
    Dek::new(key)
}

/// Generate a random 24-byte nonce for XChaCha20.
fn generate_nonce() -> [u8; 24] {
    let mut nonce = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut nonce);
    nonce
}

/// Wrap a DEK with a KEK using XChaCha20-Poly1305.
pub fn wrap_dek(kek: &Kek, dek: &Dek) -> Result<WrappedDek, VaultError> {
    let cipher = XChaCha20Poly1305::new(kek.expose().into());
    let nonce = generate_nonce();
    let xnonce = XNonce::from_slice(&nonce);

    let ciphertext = cipher
        .encrypt(xnonce, dek.expose().as_ref())
        .map_err(|e| VaultError::EncryptionError(e.to_string()))?;

    Ok(WrappedDek { nonce, ciphertext })
}

/// Unwrap a DEK using a KEK.
pub fn unwrap_dek(kek: &Kek, wrapped: &WrappedDek) -> Result<Dek, VaultError> {
    let cipher = XChaCha20Poly1305::new(kek.expose().into());
    let xnonce = XNonce::from_slice(&wrapped.nonce);

    let plaintext = cipher
        .decrypt(xnonce, wrapped.ciphertext.as_ref())
        .map_err(|e| VaultError::DecryptionError(e.to_string()))?;

    if plaintext.len() != 32 {
        return Err(VaultError::InvalidKeyLength {
            expected: 32,
            got: plaintext.len(),
        });
    }

    let mut key = Zeroizing::new([0u8; 32]);
    key.copy_from_slice(&plaintext);
    Ok(Dek::new(*key))
}

/// Encrypt a secret payload using envelope encryption.
/// Generates a random DEK, encrypts payload with DEK, wraps DEK with master DEK.
pub fn encrypt_secret(master_dek: &Dek, plaintext: &[u8]) -> Result<EncryptedPayload, VaultError> {
    // Generate random DEK for this secret
    let dek = generate_dek();

    // Encrypt payload with DEK
    let cipher = XChaCha20Poly1305::new(dek.expose().into());
    let nonce = generate_nonce();
    let xnonce = XNonce::from_slice(&nonce);

    let ciphertext = cipher
        .encrypt(xnonce, plaintext)
        .map_err(|e| VaultError::EncryptionError(e.to_string()))?;

    // Wrap DEK with master DEK
    let wrapped_dek = wrap_dek_with_dek(master_dek, &dek)?;

    Ok(EncryptedPayload {
        nonce,
        ciphertext,
        wrapped_dek,
    })
}

/// Wrap a DEK with another DEK (master DEK).
pub fn wrap_dek_with_dek(master_dek: &Dek, dek: &Dek) -> Result<WrappedDek, VaultError> {
    let cipher = XChaCha20Poly1305::new(master_dek.expose().into());
    let nonce = generate_nonce();
    let xnonce = XNonce::from_slice(&nonce);

    let ciphertext = cipher
        .encrypt(xnonce, dek.expose().as_ref())
        .map_err(|e| VaultError::EncryptionError(e.to_string()))?;

    Ok(WrappedDek { nonce, ciphertext })
}

/// Decrypt a secret payload.
pub fn decrypt_secret(
    master_dek: &Dek,
    payload: &EncryptedPayload,
) -> Result<SecretBox<Vec<u8>>, VaultError> {
    // Unwrap DEK with master DEK
    let dek = unwrap_dek_with_dek(master_dek, &payload.wrapped_dek)?;

    // Decrypt payload with DEK
    let cipher = XChaCha20Poly1305::new(dek.expose().into());
    let xnonce = XNonce::from_slice(&payload.nonce);

    let plaintext = cipher
        .decrypt(xnonce, payload.ciphertext.as_ref())
        .map_err(|e| VaultError::DecryptionError(e.to_string()))?;

    Ok(SecretBox::new(Box::new(plaintext)))
}

/// Unwrap a DEK using master DEK.
pub fn unwrap_dek_with_dek(master_dek: &Dek, wrapped: &WrappedDek) -> Result<Dek, VaultError> {
    let cipher = XChaCha20Poly1305::new(master_dek.expose().into());
    let xnonce = XNonce::from_slice(&wrapped.nonce);

    let plaintext = cipher
        .decrypt(xnonce, wrapped.ciphertext.as_ref())
        .map_err(|e| VaultError::DecryptionError(e.to_string()))?;

    if plaintext.len() != 32 {
        return Err(VaultError::InvalidKeyLength {
            expected: 32,
            got: plaintext.len(),
        });
    }

    let mut key = Zeroizing::new([0u8; 32]);
    key.copy_from_slice(&plaintext);
    Ok(Dek::new(*key))
}

/// Wrap a DEK with a raw 32-byte key (for X25519 shared secret derived keys).
pub fn wrap_dek_with_key(wrapping_key: &[u8; 32], dek: &Dek) -> Result<WrappedDek, VaultError> {
    let cipher = XChaCha20Poly1305::new(wrapping_key.into());
    let nonce = generate_nonce();
    let xnonce = XNonce::from_slice(&nonce);

    let ciphertext = cipher
        .encrypt(xnonce, dek.expose().as_ref())
        .map_err(|e| VaultError::EncryptionError(e.to_string()))?;

    Ok(WrappedDek { nonce, ciphertext })
}

/// Unwrap a DEK using a raw 32-byte key (for X25519 shared secret derived keys).
pub fn unwrap_dek_with_key(wrapping_key: &[u8; 32], wrapped: &WrappedDek) -> Result<Dek, VaultError> {
    let cipher = XChaCha20Poly1305::new(wrapping_key.into());
    let xnonce = XNonce::from_slice(&wrapped.nonce);

    let plaintext = cipher
        .decrypt(xnonce, wrapped.ciphertext.as_ref())
        .map_err(|e| VaultError::DecryptionError(e.to_string()))?;

    if plaintext.len() != 32 {
        return Err(VaultError::InvalidKeyLength {
            expected: 32,
            got: plaintext.len(),
        });
    }

    let mut key = Zeroizing::new([0u8; 32]);
    key.copy_from_slice(&plaintext);
    Ok(Dek::new(*key))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrap_unwrap_dek() {
        let kek = Kek::new([0xAB; 32]);
        let dek = generate_dek();
        let original = *dek.expose();

        let wrapped = wrap_dek(&kek, &dek).unwrap();
        let unwrapped = unwrap_dek(&kek, &wrapped).unwrap();

        assert_eq!(*unwrapped.expose(), original);
    }

    #[test]
    fn test_encrypt_decrypt_secret() {
        let master_dek = Dek::new([0xCD; 32]);
        let plaintext = b"secret data";

        let payload = encrypt_secret(&master_dek, plaintext).unwrap();
        let decrypted = decrypt_secret(&master_dek, &payload).unwrap();

        use secrecy::ExposeSecret;
        assert_eq!(decrypted.expose_secret().as_slice(), plaintext);
    }
}
