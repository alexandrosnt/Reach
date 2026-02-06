use chacha20poly1305::{
    aead::{Aead, KeyInit},
    XChaCha20Poly1305, XNonce,
};
use hkdf::Hkdf;
use rand::RngCore;
use sha2::Sha256;
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::Zeroizing;

use crate::vault::error::VaultError;
use crate::vault::types::{Dek, UserIdentity, WrappedDek};

/// Generate a new X25519 keypair.
pub fn generate_identity_keypair() -> (StaticSecret, PublicKey) {
    let secret = StaticSecret::random_from_rng(rand::thread_rng());
    let public = PublicKey::from(&secret);
    (secret, public)
}

/// Encrypt identity key with KEK for storage.
pub fn encrypt_identity_key(kek: &[u8; 32], secret_key: &StaticSecret) -> Result<(Vec<u8>, [u8; 24]), VaultError> {
    let cipher = XChaCha20Poly1305::new(kek.into());
    let mut nonce = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut nonce);
    let xnonce = XNonce::from_slice(&nonce);

    let ciphertext = cipher
        .encrypt(xnonce, secret_key.as_bytes().as_ref())
        .map_err(|e| VaultError::EncryptionError(e.to_string()))?;

    Ok((ciphertext, nonce))
}

/// Decrypt identity key with KEK.
pub fn decrypt_identity_key(kek: &[u8; 32], ciphertext: &[u8], nonce: &[u8; 24]) -> Result<StaticSecret, VaultError> {
    let cipher = XChaCha20Poly1305::new(kek.into());
    let xnonce = XNonce::from_slice(nonce);

    let plaintext = cipher
        .decrypt(xnonce, ciphertext)
        .map_err(|e| VaultError::DecryptionError(e.to_string()))?;

    if plaintext.len() != 32 {
        return Err(VaultError::InvalidKeyLength {
            expected: 32,
            got: plaintext.len(),
        });
    }

    let mut key_bytes = [0u8; 32];
    key_bytes.copy_from_slice(&plaintext);
    Ok(StaticSecret::from(key_bytes))
}

/// Wrap a DEK for a member using X25519 + HKDF + XChaCha20-Poly1305.
/// Used when inviting someone to a shared vault.
pub fn wrap_dek_for_member(dek: &Dek, member_public_key: &[u8; 32]) -> Result<WrappedDek, VaultError> {
    // Generate ephemeral X25519 keypair
    let ephemeral_secret = StaticSecret::random_from_rng(rand::thread_rng());
    let ephemeral_public = PublicKey::from(&ephemeral_secret);

    // Compute shared secret
    let member_pk = PublicKey::from(*member_public_key);
    let shared_secret = ephemeral_secret.diffie_hellman(&member_pk);

    // Derive wrapping key using HKDF
    let hkdf = Hkdf::<Sha256>::new(None, shared_secret.as_bytes());
    let mut wrapping_key = Zeroizing::new([0u8; 32]);
    hkdf.expand(b"vault-dek-wrap", wrapping_key.as_mut())
        .map_err(|e: hkdf::InvalidLength| VaultError::EncryptionError(e.to_string()))?;

    // Encrypt DEK with wrapping key
    let cipher = XChaCha20Poly1305::new(wrapping_key.as_ref().into());
    let mut nonce = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut nonce);
    let xnonce = XNonce::from_slice(&nonce);

    // Prepend ephemeral public key to ciphertext
    let mut plaintext_with_context = Vec::with_capacity(32 + dek.expose().len());
    plaintext_with_context.extend_from_slice(ephemeral_public.as_bytes());
    plaintext_with_context.extend_from_slice(dek.expose());

    let ciphertext = cipher
        .encrypt(xnonce, dek.expose().as_ref())
        .map_err(|e| VaultError::EncryptionError(e.to_string()))?;

    // Store ephemeral public key with ciphertext
    let mut full_ciphertext = Vec::with_capacity(32 + ciphertext.len());
    full_ciphertext.extend_from_slice(ephemeral_public.as_bytes());
    full_ciphertext.extend_from_slice(&ciphertext);

    Ok(WrappedDek {
        nonce,
        ciphertext: full_ciphertext,
    })
}

/// Unwrap a DEK using member's identity.
pub fn unwrap_dek_for_member(identity: &UserIdentity, wrapped: &WrappedDek) -> Result<Dek, VaultError> {
    if wrapped.ciphertext.len() < 32 {
        return Err(VaultError::DecryptionError("Invalid wrapped DEK".to_string()));
    }

    // Extract ephemeral public key
    let mut ephemeral_pk_bytes = [0u8; 32];
    ephemeral_pk_bytes.copy_from_slice(&wrapped.ciphertext[..32]);
    let ephemeral_public = PublicKey::from(ephemeral_pk_bytes);

    // Compute shared secret
    let shared_secret = identity.secret_key().diffie_hellman(&ephemeral_public);

    // Derive wrapping key using HKDF
    let hkdf = Hkdf::<Sha256>::new(None, shared_secret.as_bytes());
    let mut wrapping_key = Zeroizing::new([0u8; 32]);
    hkdf.expand(b"vault-dek-wrap", wrapping_key.as_mut())
        .map_err(|e: hkdf::InvalidLength| VaultError::DecryptionError(e.to_string()))?;

    // Decrypt DEK
    let cipher = XChaCha20Poly1305::new(wrapping_key.as_ref().into());
    let xnonce = XNonce::from_slice(&wrapped.nonce);

    let plaintext = cipher
        .decrypt(xnonce, &wrapped.ciphertext[32..])
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
