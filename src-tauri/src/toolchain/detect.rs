use serde::Serialize;
use std::process::Command;

#[derive(Debug, Clone, Serialize)]
pub struct ToolStatus {
    pub installed: bool,
    pub version: Option<String>,
    pub path: Option<String>,
    pub warning: Option<String>,
    /// If true, the tool cannot run locally on this platform (e.g. Ansible on Windows).
    pub local_unsupported: bool,
}

/// Check whether a tool is installed, get its version, and verify it actually works.
pub fn check_tool(tool: &str) -> ToolStatus {
    // For ansible, ensure Python scripts directories are in PATH before checking.
    // This handles the case where ansible was installed via pip but its scripts dir
    // (e.g. %APPDATA%\Python\Python3X\Scripts) isn't in PATH.
    if tool == "ansible" {
        ensure_ansible_in_path();
    }

    let ansible_on_windows = tool == "ansible" && cfg!(windows);

    let bin = match which::which(tool) {
        Ok(p) => p,
        Err(_) => {
            // Binary not in PATH — for ansible, check via pip metadata
            if tool == "ansible" {
                if let Some(version) = ansible_version_from_metadata() {
                    return ToolStatus {
                        installed: true,
                        version: Some(version),
                        path: None,
                        warning: if ansible_on_windows {
                            Some("Ansible does not support Windows as a control node. Use remote mode to run playbooks on a Linux server via SSH.".into())
                        } else {
                            Some("ansible is installed but binaries were not found in PATH. Playbook execution may fail.".into())
                        },
                        local_unsupported: ansible_on_windows,
                    };
                }
            }
            return ToolStatus {
                installed: false,
                version: None,
                path: None,
                warning: None,
                local_unsupported: false,
            };
        }
    };

    let path = Some(bin.to_string_lossy().to_string());

    // Run --version and check if the tool actually works
    match Command::new(&bin).arg("--version").output() {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            let text = if stdout.trim().is_empty() { &stderr } else { &stdout };
            let version = text.lines().next().unwrap_or("").trim().to_string();

            ToolStatus {
                installed: true,
                version: if version.is_empty() { None } else { Some(version) },
                path,
                warning: None,
                local_unsupported: false,
            }
        }
        Ok(out) => {
            // Command failed — tool is installed but broken
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            let (version, warning, local_broken) = if tool == "ansible" {
                diagnose_ansible_failure(&stderr)
            } else {
                (
                    None,
                    Some(format!("{} --version failed (exit {})", tool, out.status.code().unwrap_or(-1))),
                    false,
                )
            };

            ToolStatus {
                installed: true,
                version,
                path,
                warning,
                local_unsupported: local_broken,
            }
        }
        Err(e) => ToolStatus {
            installed: true,
            version: None,
            path,
            warning: Some(format!("Failed to run {}: {}", tool, e)),
            local_unsupported: false,
        },
    }
}

/// Diagnose why `ansible --version` failed and return (version, warning, local_unsupported).
fn diagnose_ansible_failure(stderr: &str) -> (Option<String>, Option<String>, bool) {
    let version = ansible_version_from_metadata();

    if stderr.contains("os.get_blocking") || (cfg!(windows) && stderr.contains("OSError")) {
        (
            version,
            Some("Ansible does not support Windows as a control node. Use remote mode to run playbooks on a Linux server via SSH.".into()),
            true,
        )
    } else if stderr.contains("ModuleNotFoundError") || stderr.contains("ImportError") {
        (
            version,
            Some("Ansible has missing dependencies. Try reinstalling with: pip install --user ansible".into()),
            false,
        )
    } else {
        let first_meaningful = stderr
            .lines()
            .find(|l| !l.trim().is_empty() && !l.contains("Traceback"))
            .unwrap_or("Unknown error")
            .trim();
        (
            version,
            Some(format!("ansible --version failed: {}", first_meaningful)),
            false,
        )
    }
}

/// Get ansible version from pip metadata without importing the package.
fn ansible_version_from_metadata() -> Option<String> {
    let pythons: &[&str] = if cfg!(windows) {
        &["python", "python3", "py"]
    } else {
        &["python3", "python"]
    };

    for python in pythons {
        if which::which(python).is_err() {
            continue;
        }
        if let Ok(out) = Command::new(python)
            .args(["-c", "from importlib.metadata import version; print(version('ansible'))"])
            .output()
        {
            if out.status.success() {
                let ver = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !ver.is_empty() {
                    return Some(format!("ansible {}", ver));
                }
            }
        }
    }
    None
}

/// Try to locate ansible binaries and add their directory to process PATH.
fn ensure_ansible_in_path() {
    if which::which("ansible").is_ok() {
        return; // already found
    }

    // Try direct binary search first (O(1) known locations)
    if let Some(found) = super::install::find_ansible_binary() {
        if let Some(parent) = found.parent() {
            let sep = if cfg!(windows) { ";" } else { ":" };
            let current = std::env::var("PATH").unwrap_or_default();
            let dir = parent.to_string_lossy().to_string();
            if !current.contains(&dir) {
                std::env::set_var("PATH", format!("{}{}{}", dir, sep, current));
                tracing::info!("Added ansible scripts dir to PATH: {}", dir);
            }
        }
        return;
    }

    // Fallback: discover all Python scripts directories
    super::install::ensure_python_scripts_in_path();
}
