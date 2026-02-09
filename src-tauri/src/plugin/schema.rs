use serde::{Deserialize, Serialize};

/// Manifest loaded from plugin.toml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    /// Entry Lua file relative to plugin directory
    #[serde(default = "default_entry")]
    pub entry: String,
    /// Permissions the plugin requests
    #[serde(default)]
    pub permissions: Vec<PluginPermission>,
    /// Hook events the plugin wants to receive
    #[serde(default)]
    pub hooks: Vec<String>,
}

fn default_entry() -> String {
    "main.lua".to_string()
}

/// Permissions a plugin can request
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginPermission {
    SshExec,
    SshListConnections,
    SftpList,
    SftpRead,
    SftpWrite,
    VaultRead,
    VaultWrite,
    TunnelManage,
    Http,
    Notify,
    Ui,
}

impl std::fmt::Display for PluginPermission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SshExec => write!(f, "ssh_exec"),
            Self::SshListConnections => write!(f, "ssh_list_connections"),
            Self::SftpList => write!(f, "sftp_list"),
            Self::SftpRead => write!(f, "sftp_read"),
            Self::SftpWrite => write!(f, "sftp_write"),
            Self::VaultRead => write!(f, "vault_read"),
            Self::VaultWrite => write!(f, "vault_write"),
            Self::TunnelManage => write!(f, "tunnel_manage"),
            Self::Http => write!(f, "http"),
            Self::Notify => write!(f, "notify"),
            Self::Ui => write!(f, "ui"),
        }
    }
}

/// Runtime status of a plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status")]
pub enum PluginStatus {
    Loaded,
    Running,
    Error { message: String },
    Disabled,
}

/// Full plugin info returned to the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInfo {
    pub manifest: PluginManifest,
    pub status: PluginStatus,
    pub granted_permissions: Vec<PluginPermission>,
    pub has_ui: bool,
}

/// Persisted plugin configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginConfig {
    pub id: String,
    pub enabled: bool,
    pub granted_permissions: Vec<PluginPermission>,
}

/// A single UI element that plugins can render
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum UiElement {
    Text { content: String, #[serde(default)] muted: bool },
    Heading { content: String, #[serde(default = "default_heading_level")] level: u8 },
    Button { label: String, action: String, #[serde(default)] variant: String },
    Input { label: String, key: String, #[serde(default)] value: String, #[serde(default)] placeholder: String },
    Toggle { label: String, key: String, #[serde(default)] checked: bool },
    Select { label: String, key: String, options: Vec<String>, #[serde(default)] selected: String },
    Table { headers: Vec<String>, rows: Vec<Vec<String>> },
    Code { content: String, #[serde(default)] language: String },
    Divider,
    Spacer,
    Row { children: Vec<UiElement> },
    Column { children: Vec<UiElement> },
    Alert { content: String, #[serde(default = "default_alert_level")] level: String },
    Progress { value: f64, #[serde(default)] label: String },
}

fn default_heading_level() -> u8 {
    2
}

fn default_alert_level() -> String {
    "info".to_string()
}

/// The UI state of a plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginUiState {
    pub plugin_id: String,
    pub title: String,
    pub elements: Vec<UiElement>,
}

/// A hook event dispatched to plugins
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookEvent {
    pub event_name: String,
    pub data: serde_json::Value,
}
