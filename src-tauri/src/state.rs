use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};

use crate::monitoring::collector::MonitoringCollector;
use crate::pty::manager::PtyManager;
use crate::serial::port::SerialManager;
use crate::ssh::client::SshManager;
use crate::tunnel::manager::TunnelManager;

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
}

/// Authentication method for an SSH session.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AuthMethod {
    Password,
    Key { path: String },
    Agent,
}

/// A folder used to organize sessions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Folder {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
}

/// An encrypted credential stored in the vault.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedCredential {
    pub nonce: String,
    pub ciphertext: String,
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

/// A saved playbook definition (persisted to disk).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedPlaybook {
    pub id: String,
    pub name: String,
    pub yaml_content: String,
    pub created_at: u64,
    pub updated_at: u64,
}

/// Tracks the execution state of a playbook run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybookRun {
    pub id: String,
    pub playbook_name: String,
    pub status: PlaybookStatus,
    pub current_step: usize,
    pub total_steps: usize,
}

/// Status of a playbook execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlaybookStatus {
    Running,
    Completed,
    Failed,
    Stopped,
}

/// Main application state.
///
/// All collections are behind `Arc<RwLock<_>>` for safe concurrent access
/// from multiple Tauri command handlers.
pub struct AppState {
    pub ssh_manager: Arc<tokio::sync::Mutex<SshManager>>,
    pub sessions: Arc<RwLock<HashMap<String, SessionConfig>>>,
    pub folders: Arc<RwLock<HashMap<String, Folder>>>,
    pub credentials: Arc<RwLock<HashMap<String, EncryptedCredential>>>,
    pub tunnels: Arc<RwLock<HashMap<String, TunnelConfig>>>,
    pub monitoring: Arc<RwLock<HashMap<String, SystemStats>>>,
    pub saved_playbooks: Arc<RwLock<HashMap<String, SavedPlaybook>>>,
    pub playbook_runs: Arc<RwLock<HashMap<String, PlaybookRun>>>,
    pub master_key: Arc<RwLock<Option<Vec<u8>>>>,
    pub pty_manager: Arc<Mutex<PtyManager>>,
    pub monitoring_collector: Arc<tokio::sync::Mutex<MonitoringCollector>>,
    pub tunnel_manager: Arc<tokio::sync::Mutex<TunnelManager>>,
    pub serial_manager: Arc<tokio::sync::Mutex<SerialManager>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            ssh_manager: Arc::new(tokio::sync::Mutex::new(SshManager::new())),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            folders: Arc::new(RwLock::new(HashMap::new())),
            credentials: Arc::new(RwLock::new(HashMap::new())),
            tunnels: Arc::new(RwLock::new(HashMap::new())),
            monitoring: Arc::new(RwLock::new(HashMap::new())),
            saved_playbooks: Arc::new(RwLock::new(HashMap::new())),
            playbook_runs: Arc::new(RwLock::new(HashMap::new())),
            master_key: Arc::new(RwLock::new(None)),
            pty_manager: Arc::new(Mutex::new(PtyManager::new())),
            monitoring_collector: Arc::new(tokio::sync::Mutex::new(MonitoringCollector::new())),
            tunnel_manager: Arc::new(tokio::sync::Mutex::new(TunnelManager::new())),
            serial_manager: Arc::new(tokio::sync::Mutex::new(SerialManager::new())),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
