use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
#[cfg(desktop)]
use std::sync::Mutex;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};

use crate::monitoring::collector::MonitoringCollector;
#[cfg(desktop)]
use crate::pty::manager::PtyManager;
#[cfg(desktop)]
use crate::serial::port::SerialManager;
use crate::ssh::client::SshManager;
use crate::ansible::project::AnsibleProjectManager;
use crate::tofu::project::TofuProjectManager;
use crate::tofu::types::SchemaCache;
use crate::tunnel::manager::TunnelManager;
use crate::vault::VaultManager;
use crate::plugin::manager::PluginManager;

/// Configuration for a jump host in a ProxyJump chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JumpHostConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_method: AuthMethod,
}

/// Configuration for a saved session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_method: AuthMethod,
    pub folder_id: Option<String>,
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detected_os: Option<String>,
    /// Which vault this session belongs to (None = private __sessions__ vault)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vault_id: Option<String>,
    /// ProxyJump chain: ordered list of jump hosts to tunnel through
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jump_chain: Option<Vec<JumpHostConfig>>,
}

/// Authentication method for an SSH session.
/// Credentials (password, passphrase, key_content) are stored encrypted in the vault.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AuthMethod {
    Password {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        password: Option<String>,
    },
    Key {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        path: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        passphrase: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        key_content: Option<String>, // Embedded key for sharing
    },
    Agent,
}

/// A folder used to organize sessions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Folder {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
}

/// Configuration for a port-forwarding tunnel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelConfig {
    pub id: String,
    pub tunnel_type: TunnelType,
    pub local_port: u16,
    pub remote_host: String,
    pub remote_port: u16,
    pub connection_id: String,
    pub active: bool,
}

/// The type of SSH tunnel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TunnelType {
    Local,
    Remote,
    Dynamic,
}

/// System statistics collected from a remote host.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemStats {
    pub cpu: f64,
    pub ram: f64,
    pub ram_total: u64,
    pub ram_used: u64,
    pub disk: f64,
    pub users: Vec<String>,
}

/// Main application state.
///
/// All collections are behind `Arc<RwLock<_>>` for safe concurrent access
/// from multiple Tauri command handlers.
///
/// Note: Sessions, folders, credentials, and settings are all
/// stored encrypted in the vault (SQLite + XChaCha20-Poly1305).
pub struct AppState {
    pub ssh_manager: Arc<tokio::sync::Mutex<SshManager>>,
    pub tunnels: Arc<RwLock<HashMap<String, TunnelConfig>>>,
    pub monitoring: Arc<RwLock<HashMap<String, SystemStats>>>,
    #[cfg(desktop)]
    pub pty_manager: Arc<Mutex<PtyManager>>,
    pub monitoring_collector: Arc<tokio::sync::Mutex<MonitoringCollector>>,
    pub tunnel_manager: Arc<tokio::sync::Mutex<TunnelManager>>,
    #[cfg(desktop)]
    pub serial_manager: Arc<tokio::sync::Mutex<SerialManager>>,
    pub vault_manager: Arc<tokio::sync::Mutex<VaultManager>>,
    pub plugin_manager: Arc<tokio::sync::Mutex<PluginManager>>,
    pub ansible_project_manager: Arc<tokio::sync::Mutex<AnsibleProjectManager>>,
    pub tofu_project_manager: Arc<tokio::sync::Mutex<TofuProjectManager>>,
    pub tofu_schema_cache: Arc<tokio::sync::Mutex<SchemaCache>>,
    pub close_to_tray: AtomicBool,
}

impl AppState {
    pub fn new() -> Self {
        // Get app data directory for vault storage
        let app_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("com.reach.app");

        Self {
            ssh_manager: Arc::new(tokio::sync::Mutex::new(SshManager::new())),
            tunnels: Arc::new(RwLock::new(HashMap::new())),
            monitoring: Arc::new(RwLock::new(HashMap::new())),
            #[cfg(desktop)]
            pty_manager: Arc::new(Mutex::new(PtyManager::new())),
            monitoring_collector: Arc::new(tokio::sync::Mutex::new(MonitoringCollector::new())),
            tunnel_manager: Arc::new(tokio::sync::Mutex::new(TunnelManager::new())),
            #[cfg(desktop)]
            serial_manager: Arc::new(tokio::sync::Mutex::new(SerialManager::new())),
            vault_manager: Arc::new(tokio::sync::Mutex::new(VaultManager::new(app_dir.clone()))),
            plugin_manager: Arc::new(tokio::sync::Mutex::new(PluginManager::new(app_dir.join("plugins")))),
            ansible_project_manager: Arc::new(tokio::sync::Mutex::new(AnsibleProjectManager::new())),
            tofu_project_manager: Arc::new(tokio::sync::Mutex::new(TofuProjectManager::new())),
            tofu_schema_cache: Arc::new(tokio::sync::Mutex::new(SchemaCache::default())),
            close_to_tray: AtomicBool::new(false),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
