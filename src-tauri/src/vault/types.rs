use serde::{Deserialize, Serialize};
use std::fmt;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// 32-byte Key Encryption Key (derived from password via Argon2id).
/// Zeroized on drop. Never logged or serialized.
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct Kek([u8; 32]);

impl Kek {
    pub fn new(key: [u8; 32]) -> Self {
        Self(key)
    }

    pub fn expose(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for Kek {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED KEK]")
    }
}

/// 32-byte Data Encryption Key (random per secret).
/// Zeroized on drop. Never logged or serialized.
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct Dek([u8; 32]);

impl Dek {
    pub fn new(key: [u8; 32]) -> Self {
        Self(key)
    }

    pub fn expose(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for Dek {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED DEK]")
    }
}

/// Wrapped DEK (encrypted with KEK or master DEK).
#[derive(Clone, Serialize, Deserialize)]
pub struct WrappedDek {
    pub nonce: [u8; 24],
    pub ciphertext: Vec<u8>,
}

/// Encrypted secret payload.
#[derive(Clone, Serialize, Deserialize)]
pub struct EncryptedPayload {
    pub nonce: [u8; 24],
    pub ciphertext: Vec<u8>,
    pub wrapped_dek: WrappedDek,
}

/// Vault header metadata.
#[derive(Clone, Serialize, Deserialize)]
pub struct VaultHeader {
    pub id: String,
    pub name: String,
    pub salt: [u8; 32],
    pub user_uuid: String,
    pub created_at: i64,
    pub vault_type: VaultType,
}

/// Type of vault.
#[derive(Clone, Serialize, Deserialize)]
pub enum VaultType {
    Private,
    Shared { members: Vec<String> },
}

/// Vault info returned to frontend.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultInfo {
    pub id: String,
    pub name: String,
    pub vault_type: String,
    pub member_count: Option<usize>,
    pub secret_count: usize,
    pub last_sync: Option<i64>,
}

/// Secret metadata (without payload).
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretMetadata {
    pub id: String,
    pub name: String,
    pub category: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Secret category.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SecretCategory {
    Password,
    SshKey,
    ApiToken,
    Certificate,
    Note,
    Session,
    Credential,
    Folder,
    Playbook,
    Setting,
    Custom(String),
}

impl fmt::Display for SecretCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SecretCategory::Password => write!(f, "password"),
            SecretCategory::SshKey => write!(f, "ssh_key"),
            SecretCategory::ApiToken => write!(f, "api_token"),
            SecretCategory::Certificate => write!(f, "certificate"),
            SecretCategory::Note => write!(f, "note"),
            SecretCategory::Session => write!(f, "session"),
            SecretCategory::Credential => write!(f, "credential"),
            SecretCategory::Folder => write!(f, "folder"),
            SecretCategory::Playbook => write!(f, "playbook"),
            SecretCategory::Setting => write!(f, "setting"),
            SecretCategory::Custom(s) => write!(f, "custom:{}", s),
        }
    }
}

impl std::str::FromStr for SecretCategory {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "password" => Ok(SecretCategory::Password),
            "ssh_key" => Ok(SecretCategory::SshKey),
            "api_token" => Ok(SecretCategory::ApiToken),
            "certificate" => Ok(SecretCategory::Certificate),
            "note" => Ok(SecretCategory::Note),
            "session" => Ok(SecretCategory::Session),
            "credential" => Ok(SecretCategory::Credential),
            "folder" => Ok(SecretCategory::Folder),
            "playbook" => Ok(SecretCategory::Playbook),
            "setting" => Ok(SecretCategory::Setting),
            s if s.starts_with("custom:") => Ok(SecretCategory::Custom(s[7..].to_string())),
            _ => Err(format!("Unknown category: {}", s)),
        }
    }
}

/// Member role in shared vault.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MemberRole {
    Owner,
    Admin,
    Member,
    ReadOnly,
}

impl fmt::Display for MemberRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MemberRole::Owner => write!(f, "owner"),
            MemberRole::Admin => write!(f, "admin"),
            MemberRole::Member => write!(f, "member"),
            MemberRole::ReadOnly => write!(f, "readonly"),
        }
    }
}

impl std::str::FromStr for MemberRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "owner" => Ok(MemberRole::Owner),
            "admin" => Ok(MemberRole::Admin),
            "member" => Ok(MemberRole::Member),
            "readonly" => Ok(MemberRole::ReadOnly),
            _ => Err(format!("Unknown role: {}", s)),
        }
    }
}

/// Member info.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemberInfo {
    pub user_uuid: String,
    pub public_key: String,
    pub role: String,
    pub added_at: i64,
}

/// Invite info for sharing.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InviteInfo {
    pub vault_id: String,
    pub sync_url: String,
    pub token: String,
}

/// User identity (X25519 keypair).
/// Secret key is zeroized on drop.
pub struct UserIdentity {
    pub uuid: String,
    secret_key: x25519_dalek::StaticSecret,
    pub public_key: x25519_dalek::PublicKey,
}

impl UserIdentity {
    pub fn new(uuid: String, secret_key: x25519_dalek::StaticSecret) -> Self {
        let public_key = x25519_dalek::PublicKey::from(&secret_key);
        Self {
            uuid,
            secret_key,
            public_key,
        }
    }

    pub fn secret_key(&self) -> &x25519_dalek::StaticSecret {
        &self.secret_key
    }
}

impl fmt::Debug for UserIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UserIdentity")
            .field("uuid", &self.uuid)
            .field("secret_key", &"[REDACTED]")
            .field("public_key", &self.public_key.as_bytes())
            .finish()
    }
}

/// Shared item info (session/credential shared with another user).
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedItemInfo {
    pub id: String,
    pub secret_id: String,
    pub recipient_uuid: String,
    pub expires_at: Option<i64>,
    pub created_at: i64,
}

/// Share item request (returned to sharer with details for recipient).
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareItemResult {
    pub share_id: String,
    pub secret_id: String,
    pub recipient_uuid: String,
    /// If Turso sync enabled, provide sync URL for recipient
    pub sync_url: Option<String>,
    pub expires_at: Option<i64>,
}

/// Received share (what recipient sees).
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceivedShare {
    pub share_id: String,
    pub secret_id: String,
    pub secret_name: String,
    pub category: String,
    pub sharer_uuid: String,
    pub received_at: i64,
}

/// App settings (stored encrypted in __settings__ vault).
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    /// Turso organization slug
    pub turso_org: Option<String>,
    /// Turso Platform API token (for creating databases)
    pub turso_api_token: Option<String>,
    /// Turso database group
    pub turso_group: Option<String>,
    /// Personal database URL (for syncing user's own data)
    pub personal_db_url: Option<String>,
    /// Personal database token
    pub personal_db_token: Option<String>,
    /// Sync enabled
    pub sync_enabled: bool,
    /// Theme preference
    pub theme: Option<String>,
    /// Additional custom settings
    #[serde(flatten)]
    pub custom: std::collections::HashMap<String, serde_json::Value>,
}

/// Turso database info returned after creation.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TursoDbInfo {
    pub db_id: String,
    pub hostname: String,
    pub name: String,
}
