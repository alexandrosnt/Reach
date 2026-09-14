//! Private keys kept in the vault instead of on disk.
//!
//! A key you imported is a secret in the `__ssh_keys__` vault, so it is
//! encrypted at rest and — when personal sync is on — reaches every machine
//! you sign in from. A session then points at the key by id rather than at a
//! path, which is what makes the same session work on a laptop that has never
//! seen your `~/.ssh` directory.
//!
//! The passphrase travels with the key, not with the session: one key unlocked
//! once is one key that works everywhere it is used. Key material only ever
//! leaves this module through `resolve`, on its way into an SSH handshake —
//! `list` returns what a person needs to recognise a key and nothing more.

use serde::{Deserialize, Serialize};

/// What is stored, encrypted, under one secret.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredKeyMaterial {
    /// The private key exactly as the file contained it, newlines and all.
    pub private_key: String,
    /// The passphrase that opens it, when the user chose to save one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub passphrase: Option<String>,
    /// The matching public key, when the import had one to offer. Kept so the
    /// user can copy it to a server's `authorized_keys` without the original
    /// `.pub` file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_key: Option<String>,
}

/// What a person sees in a list of keys. Never any key material.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredKeyInfo {
    pub id: String,
    pub name: String,
    /// `ssh-ed25519`, `ssh-rsa`, … as the key itself declares it.
    pub algo: Option<String>,
    /// The SHA-256 fingerprint OpenSSH would print, so a key can be told apart
    /// from another one with a similar name.
    pub fingerprint: Option<String>,
    /// Whether the key material itself is passphrase-protected.
    pub encrypted: bool,
    /// Whether a passphrase is stored alongside it — an encrypted key without
    /// one will ask at connect time.
    pub has_passphrase: bool,
    pub public_key: Option<String>,
    pub created_at: i64,
}

/// Everything a handshake needs from a stored key.
pub struct ResolvedKey {
    pub private_key: String,
    pub passphrase: Option<String>,
}

/// What an import could work out about the key before saving it.
#[derive(Debug)]
pub struct KeyFacts {
    pub algo: Option<String>,
    pub fingerprint: Option<String>,
    pub encrypted: bool,
    pub public_key: Option<String>,
}

/// Read what the key says about itself, and refuse anything that is not a
/// private key at all. An encrypted key is still described — we want its
/// algorithm and fingerprint in the list even when it cannot be opened yet.
pub fn inspect(private_key: &str) -> Result<KeyFacts, String> {
    use ssh_key::PrivateKey;

    let trimmed = private_key.trim();
    if trimmed.is_empty() {
        return Err("The private key is empty.".into());
    }
    if trimmed.starts_with("ssh-") || trimmed.starts_with("ecdsa-") {
        return Err(
            "That is an OpenSSH public key. Import the private key — the file without the .pub extension."
                .into(),
        );
    }

    // OpenSSH's own container tells us the most, including whether it is
    // encrypted, without needing the passphrase.
    if let Ok(pk) = PrivateKey::from_openssh(trimmed) {
        let encrypted = pk.is_encrypted();
        let public_key = pk.public_key().to_openssh().ok();
        return Ok(KeyFacts {
            algo: Some(pk.algorithm().as_str().to_string()),
            fingerprint: Some(pk.fingerprint(Default::default()).to_string()),
            encrypted,
            public_key,
        });
    }

    // PEM and PKCS#8 keys: russh can still read them, so accept them as long as
    // one of the two passes reads. An encrypted PEM key announces itself in the
    // header rather than in a parsed field.
    let looks_encrypted = trimmed.contains("ENCRYPTED")
        || trimmed.contains("Proc-Type: 4,ENCRYPTED")
        || trimmed.contains("DEK-Info:");
    if russh_keys::decode_secret_key(trimmed, None).is_ok() {
        return Ok(KeyFacts {
            algo: None,
            fingerprint: None,
            encrypted: false,
            public_key: None,
        });
    }
    if looks_encrypted || trimmed.contains("PRIVATE KEY") {
        return Ok(KeyFacts {
            algo: None,
            fingerprint: None,
            encrypted: true,
            public_key: None,
        });
    }

    Err("That does not look like a private key file.".into())
}

/// Check that a passphrase actually opens a key, so an import fails here
/// rather than at connect time on a server three hops away.
pub fn verify_passphrase(private_key: &str, passphrase: Option<&str>) -> Result<(), String> {
    let pass = passphrase.filter(|p| !p.is_empty());
    match crate::ssh::client::decode_key(private_key, pass) {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("The key could not be opened: {}", e)),
    }
}

/// Build the info a list shows from the material and the name it was saved
/// under.
pub fn describe(id: &str, name: &str, created_at: i64, material: &StoredKeyMaterial) -> StoredKeyInfo {
    let facts = inspect(&material.private_key).ok();
    StoredKeyInfo {
        id: id.to_string(),
        name: name.to_string(),
        algo: facts.as_ref().and_then(|f| f.algo.clone()),
        fingerprint: facts.as_ref().and_then(|f| f.fingerprint.clone()),
        encrypted: facts.as_ref().map(|f| f.encrypted).unwrap_or(false),
        has_passphrase: material
            .passphrase
            .as_deref()
            .map(|p| !p.is_empty())
            .unwrap_or(false),
        public_key: material
            .public_key
            .clone()
            .or_else(|| facts.as_ref().and_then(|f| f.public_key.clone())),
        created_at,
    }
}

impl From<&StoredKeyMaterial> for ResolvedKey {
    fn from(m: &StoredKeyMaterial) -> Self {
        ResolvedKey {
            private_key: m.private_key.clone(),
            passphrase: m.passphrase.clone().filter(|p| !p.is_empty()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Throwaway keys generated for these tests and used on no host anywhere.
    const ED25519: &str = "-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAAAMwAAAAtzc2gtZW\nQyNTUxOQAAACCwsCZ1pFDo+/0R+iXN9Emzi+emmN0GEbCoIL9hE1eafgAAAJhbzNn6W8zZ\n+gAAAAtzc2gtZWQyNTUxOQAAACCwsCZ1pFDo+/0R+iXN9Emzi+emmN0GEbCoIL9hE1eafg\nAAAEDs2ThleIPEbrfMS5+KvabXLE0113+oUG6NIEaDtPChvLCwJnWkUOj7/RH6Jc30SbOL\n56aY3QYRsKggv2ETV5p+AAAAEnJlYWNoLXRlc3RAZXhhbXBsZQECAw==\n-----END OPENSSH PRIVATE KEY-----\n";
    const ED25519_ENCRYPTED: &str = "-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaC1rZXktdjEAAAAACmFlczI1Ni1jdHIAAAAGYmNyeXB0AAAAGAAAABCcqk0FV4\nKt6ydcd+Fwf0h1AAAAGAAAAAEAAAAzAAAAC3NzaC1lZDI1NTE5AAAAIJk+zQU8O+k3l0Y9\nx4epBs/36Jir8BpbhqTJH0+tr864AAAAoPYA20IkAMre5HEKk0iDCkEk72vCMulAvfM7LK\nO3hbWbXJAzOgl5v1AzwoTzi73AvzN7rTxU+wsQ9vuWntsc7WjaqdQeg/ZHi3pNKRkyX+Ay\nGmXQTdgeHjjkQ/mc7GTjpqYFV32+LlFRoC+zP50ivDKINyUXgzd0zz/G2xR5zQjU/boKQU\n719oRMkiGz8P1eib8piYDdiJ3fY0MDqbac8QM=\n-----END OPENSSH PRIVATE KEY-----\n";
    const PASSPHRASE: &str = "hunter2";

    #[test]
    fn reads_an_unencrypted_ed25519_key() {
        let facts = inspect(ED25519).expect("should parse");
        assert_eq!(facts.algo.as_deref(), Some("ssh-ed25519"));
        assert!(!facts.encrypted);
        assert!(facts.fingerprint.unwrap().starts_with("SHA256:"));
        assert!(facts.public_key.unwrap().starts_with("ssh-ed25519 "));
    }

    #[test]
    fn reads_an_encrypted_key_without_the_passphrase() {
        // The list has to show an encrypted key before anyone can unlock it.
        let facts = inspect(ED25519_ENCRYPTED).expect("should parse");
        assert_eq!(facts.algo.as_deref(), Some("ssh-ed25519"));
        assert!(facts.encrypted);
    }

    #[test]
    fn refuses_a_public_key() {
        let err = inspect("ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAILCwJnWkUOj7/RH6Jc30SbOL56aY3QYRsKggv2ETV5p+ reach-test@example").unwrap_err();
        assert!(err.contains("public key"), "{}", err);
    }

    #[test]
    fn refuses_nonsense() {
        assert!(inspect("hello world").is_err());
        assert!(inspect("   ").is_err());
    }

    #[test]
    fn a_passphrase_on_an_unencrypted_key_is_not_an_error() {
        // The second half of issue #46: someone types something into a
        // passphrase box for a key that never had one. That used to be a hard
        // failure, because russh hands the passphrase to a decrypt() that
        // refuses keys which were never encrypted.
        verify_passphrase(ED25519, Some("not the passphrase")).expect("should still open");
        verify_passphrase(ED25519, None).expect("should open");
        verify_passphrase(ED25519, Some("")).expect("should open");
    }

    #[test]
    fn an_encrypted_key_still_needs_the_right_passphrase() {
        verify_passphrase(ED25519_ENCRYPTED, Some(PASSPHRASE)).expect("should open");
        assert!(verify_passphrase(ED25519_ENCRYPTED, Some("wrong")).is_err());
        assert!(verify_passphrase(ED25519_ENCRYPTED, None).is_err());
    }

    #[test]
    fn describe_reports_whether_a_passphrase_is_saved() {
        let bare = StoredKeyMaterial {
            private_key: ED25519.to_string(),
            passphrase: None,
            public_key: None,
        };
        let info = describe("id1", "laptop", 0, &bare);
        assert!(!info.has_passphrase);
        assert!(!info.encrypted);
        assert_eq!(info.name, "laptop");
        // The public half is recovered from the private key, so a session can
        // show what to paste into authorized_keys without the .pub file.
        assert!(info.public_key.unwrap().starts_with("ssh-ed25519 "));

        let saved = StoredKeyMaterial {
            private_key: ED25519_ENCRYPTED.to_string(),
            passphrase: Some(PASSPHRASE.into()),
            public_key: None,
        };
        let info = describe("id2", "work", 0, &saved);
        assert!(info.has_passphrase);
        assert!(info.encrypted);
    }

    #[test]
    fn stored_material_round_trips_through_json() {
        // The vault holds JSON; a key that cannot be read back is a key that
        // silently stops working on the next machine.
        let material = StoredKeyMaterial {
            private_key: ED25519.to_string(),
            passphrase: Some(PASSPHRASE.into()),
            public_key: Some("ssh-ed25519 AAAA test".into()),
        };
        let bytes = serde_json::to_vec(&material).unwrap();
        let back: StoredKeyMaterial = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(back.private_key, material.private_key);
        assert_eq!(back.passphrase, material.passphrase);
        assert_eq!(back.public_key, material.public_key);
    }
}
