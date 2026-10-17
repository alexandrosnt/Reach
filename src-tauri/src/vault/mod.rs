#[cfg(test)]
mod compat_tests;
pub mod biometric;
pub mod cache;
pub mod crypto;
#[cfg(any(target_os = "linux", test))]
pub mod deadline;
pub mod error;
pub mod export;
pub mod fido2;
pub mod kdf;
#[cfg(target_os = "linux")]
pub mod linux_keychain;
pub mod manager;
pub mod schema;
pub mod shield;
pub mod sharing;
pub mod sync;
pub mod turso_api;
pub mod turso_tls;
pub mod types;

pub use error::VaultError;
pub use manager::{
    VaultManager, CREDENTIALS_VAULT, FOLDERS_VAULT, PLAYBOOKS_VAULT, SESSIONS_VAULT, SETTINGS_VAULT,
};
pub use types::{
    AppSettings, InviteInfo, MemberInfo, MemberRole, ReceivedShare, SecretCategory, SecretMetadata,
    ShareItemResult, SharedItemInfo, TursoDbInfo, VaultInfo, VaultType,
};
