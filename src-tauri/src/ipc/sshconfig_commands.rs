//! IPC commands for parsing and importing SSH config files.

use crate::ssh::config::{self, SshHostEntry};

/// List all named hosts from ~/.ssh/config.
#[tauri::command]
pub async fn sshconfig_list_hosts() -> Result<Vec<SshHostEntry>, String> {
    let config = config::parse_ssh_config()?;

    match config {
        Some(cfg) => Ok(config::list_hosts(&cfg)),
        None => Ok(Vec::new()),
    }
}

/// Resolve a single host from ~/.ssh/config with full details.
#[tauri::command]
pub async fn sshconfig_resolve_host(hostname: String) -> Result<SshHostEntry, String> {
    let config = config::parse_ssh_config()?;

    match config {
        Some(cfg) => Ok(config::resolve_host(&cfg, &hostname)),
        None => Err("No SSH config file found".to_string()),
    }
}

/// Check if an SSH config file exists.
#[tauri::command]
pub async fn sshconfig_exists() -> Result<bool, String> {
    let path = dirs::home_dir()
        .map(|h| h.join(".ssh").join("config"));

    match path {
        Some(p) => Ok(p.exists()),
        None => Ok(false),
    }
}
