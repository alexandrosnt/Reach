use serde::{Deserialize, Serialize};
use ssh2_config::{ParseRule, SshConfig};
use std::io::BufReader;
use std::path::PathBuf;

/// A resolved SSH host from ~/.ssh/config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshHostEntry {
    /// The Host pattern name (e.g. "myserver")
    pub name: String,
    /// Resolved hostname (HostName directive, falls back to name)
    pub hostname: String,
    /// Port (default 22)
    pub port: u16,
    /// Username (User directive)
    pub user: String,
    /// Identity files (IdentityFile directives)
    pub identity_files: Vec<String>,
    /// ProxyJump chain, ordered outermost-first
    pub proxy_jump: Vec<JumpHostEntry>,
}

/// A single jump host in a ProxyJump chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JumpHostEntry {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub identity_files: Vec<String>,
}

/// Get the path to ~/.ssh/config (cross-platform).
fn ssh_config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".ssh").join("config"))
}

/// Parse the user's SSH config file. Returns None if file doesn't exist.
pub fn parse_ssh_config() -> Result<Option<SshConfig>, String> {
    let path = match ssh_config_path() {
        Some(p) => p,
        None => return Ok(None),
    };

    if !path.exists() {
        return Ok(None);
    }

    let file = std::fs::File::open(&path)
        .map_err(|e| format!("Failed to open SSH config: {}", e))?;
    let mut reader = BufReader::new(file);

    let config = SshConfig::default()
        .parse(&mut reader, ParseRule::ALLOW_UNKNOWN_FIELDS)
        .map_err(|e| format!("Failed to parse SSH config: {}", e))?;

    Ok(Some(config))
}

/// List all named (non-wildcard) hosts in the SSH config.
pub fn list_hosts(config: &SshConfig) -> Vec<SshHostEntry> {
    let mut entries = Vec::new();

    for host in config.get_hosts() {
        for clause in &host.pattern {
            // Skip wildcard patterns and negated patterns
            if clause.negated || clause.pattern.contains('*') || clause.pattern.contains('?') {
                continue;
            }

            let name = clause.pattern.clone();
            let params = config.query(&name);

            let hostname = params
                .host_name
                .clone()
                .unwrap_or_else(|| name.clone());
            let port = params.port.unwrap_or(22);
            let user = params
                .user
                .clone()
                .unwrap_or_else(|| whoami().unwrap_or_else(|| "root".to_string()));
            let identity_files = params
                .identity_file
                .as_ref()
                .map(|files| {
                    files
                        .iter()
                        .map(|p| resolve_tilde(p).display().to_string())
                        .collect()
                })
                .unwrap_or_default();

            // Resolve ProxyJump chain
            let proxy_jump = resolve_proxy_jump(config, &params);

            entries.push(SshHostEntry {
                name,
                hostname,
                port,
                user,
                identity_files,
                proxy_jump,
            });
        }
    }

    entries
}

/// Resolve a single host by name and return its full config.
pub fn resolve_host(config: &SshConfig, hostname: &str) -> SshHostEntry {
    let params = config.query(hostname);

    let resolved_hostname = params
        .host_name
        .clone()
        .unwrap_or_else(|| hostname.to_string());
    let port = params.port.unwrap_or(22);
    let user = params
        .user
        .clone()
        .unwrap_or_else(|| whoami().unwrap_or_else(|| "root".to_string()));
    let identity_files = params
        .identity_file
        .as_ref()
        .map(|files| {
            files
                .iter()
                .map(|p| resolve_tilde(p).display().to_string())
                .collect()
        })
        .unwrap_or_default();

    let proxy_jump = resolve_proxy_jump(config, &params);

    SshHostEntry {
        name: hostname.to_string(),
        hostname: resolved_hostname,
        port,
        user,
        identity_files,
        proxy_jump,
    }
}

/// Resolve the ProxyJump chain from a host's params.
/// Returns jump hosts ordered outermost-first (connect first hop first).
fn resolve_proxy_jump(
    config: &SshConfig,
    params: &ssh2_config::HostParams,
) -> Vec<JumpHostEntry> {
    let jump_specs = match &params.proxy_jump {
        Some(specs) if !specs.is_empty() => specs.clone(),
        _ => return Vec::new(),
    };

    let mut chain = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for spec in &jump_specs {
        resolve_jump_recursive(config, spec, &mut chain, &mut seen);
    }

    chain
}

/// Recursively resolve a jump host spec (which may itself have ProxyJump).
fn resolve_jump_recursive(
    config: &SshConfig,
    spec: &str,
    chain: &mut Vec<JumpHostEntry>,
    seen: &mut std::collections::HashSet<String>,
) {
    // Prevent infinite loops
    if seen.contains(spec) {
        return;
    }
    seen.insert(spec.to_string());

    // Parse "user@host:port" or "host:port" or "host"
    let (user_part, host_port) = if let Some(at_pos) = spec.find('@') {
        (Some(&spec[..at_pos]), &spec[at_pos + 1..])
    } else {
        (None, spec)
    };

    let (host, port) = if let Some(colon_pos) = host_port.rfind(':') {
        let port_str = &host_port[colon_pos + 1..];
        if let Ok(p) = port_str.parse::<u16>() {
            (&host_port[..colon_pos], p)
        } else {
            (host_port, 22u16)
        }
    } else {
        (host_port, 22u16)
    };

    // Query the config for this jump host to resolve its own settings
    let jump_params = config.query(host);

    let resolved_host = jump_params
        .host_name
        .clone()
        .unwrap_or_else(|| host.to_string());
    let resolved_port = jump_params.port.unwrap_or(port);
    let resolved_user = user_part
        .map(|u| u.to_string())
        .or_else(|| jump_params.user.clone())
        .unwrap_or_else(|| whoami().unwrap_or_else(|| "root".to_string()));
    let identity_files = jump_params
        .identity_file
        .as_ref()
        .map(|files| {
            files
                .iter()
                .map(|p| resolve_tilde(p).display().to_string())
                .collect()
        })
        .unwrap_or_default();

    // If this jump host itself has ProxyJump, resolve those first (outermost first)
    if let Some(nested_jumps) = &jump_params.proxy_jump {
        for nested in nested_jumps {
            resolve_jump_recursive(config, nested, chain, seen);
        }
    }

    chain.push(JumpHostEntry {
        host: resolved_host,
        port: resolved_port,
        user: resolved_user,
        identity_files,
    });
}

/// Resolve ~ to home directory in a path.
fn resolve_tilde(path: &PathBuf) -> PathBuf {
    let s = path.display().to_string();
    if s.starts_with("~/") || s.starts_with("~\\") {
        if let Some(home) = dirs::home_dir() {
            return home.join(&s[2..]);
        }
    }
    path.clone()
}

/// Get the current user's username.
fn whoami() -> Option<String> {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .ok()
}
