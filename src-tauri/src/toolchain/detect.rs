use serde::Serialize;
use std::process::Command;

#[derive(Debug, Clone, Serialize)]
pub struct ToolStatus {
    pub installed: bool,
    pub version: Option<String>,
    pub path: Option<String>,
    pub warning: Option<String>,
    /// If true, the tool cannot run locally on this platform (e.g. Ansible on Windows without WSL).
    pub local_unsupported: bool,
    /// If true, the tool runs through WSL on Windows.
    pub wsl: bool,
}

/// Check whether a tool is installed, get its version, and verify it actually works.
pub fn check_tool(tool: &str) -> ToolStatus {
    // Ansible on Windows: ALWAYS check WSL first. Native ansible is broken on Windows
    // (os.get_blocking / OSError) so we never try to run it directly.
    #[cfg(windows)]
    {
        if tool == "ansible" {
            return check_ansible_windows();
        }
    }

    // For ansible on non-Windows, ensure Python scripts directories are in PATH.
    if tool == "ansible" {
        ensure_ansible_in_path();
    }

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
                        warning: Some("ansible is installed but binaries were not found in PATH. Playbook execution may fail.".into()),
                        local_unsupported: false,
                        wsl: false,
                    };
                }
            }
            return ToolStatus {
                installed: false,
                version: None,
                path: None,
                warning: None,
                local_unsupported: false,
                wsl: false,
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
                wsl: false,
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
                wsl: false,
            }
        }
        Err(e) => ToolStatus {
            installed: true,
            version: None,
            path,
            warning: Some(format!("Failed to run {}: {}", tool, e)),
            local_unsupported: false,
            wsl: false,
        },
    }
}

/// Check Ansible on Windows. Native ansible is broken on Windows, so we check WSL first.
/// Only if WSL is not available do we fall back to reporting the native (broken) status.
#[cfg(windows)]
fn check_ansible_windows() -> ToolStatus {
    // Step 1: Check if WSL is available
    let wsl_list = Command::new("wsl.exe")
        .args(["--list", "--quiet"])
        .output();

    let wsl_available = wsl_list
        .as_ref()
        .map(|out| {
            out.status.success()
                && String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .any(|l| !l.trim().trim_matches('\u{feff}').is_empty())
        })
        .unwrap_or(false);

    if wsl_available {
        // Step 2: Check if ansible is installed inside WSL.
        // Use login shell (-l) so ~/.profile / ~/.bashrc PATH additions are loaded
        // (pip --user installs to ~/.local/bin which is often only in interactive PATH).
        let ansible_check = Command::new("wsl.exe")
            .args(["--", "bash", "-lc", "which ansible 2>/dev/null || command -v ansible 2>/dev/null"])
            .output();

        if let Ok(ref out) = ansible_check {
            if out.status.success() {
                let wsl_path = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !wsl_path.is_empty() {
                    // WSL has ansible — get version
                    let version = Command::new("wsl.exe")
                        .args(["--", "bash", "-lc", "ansible --version 2>/dev/null"])
                        .output()
                        .ok()
                        .and_then(|o| {
                            if o.status.success() {
                                let s = String::from_utf8_lossy(&o.stdout).to_string();
                                let first = s.lines().next().unwrap_or("").trim().to_string();
                                if first.is_empty() { None } else { Some(first) }
                            } else {
                                None
                            }
                        });

                    return ToolStatus {
                        installed: true,
                        version,
                        path: Some(format!("WSL: {}", wsl_path)),
                        warning: None,
                        local_unsupported: false,
                        wsl: true,
                    };
                }
            }
        }

        // WSL exists but no ansible installed
        return ToolStatus {
            installed: false,
            version: None,
            path: None,
            warning: None,
            local_unsupported: false,
            wsl: true,
        };
    }

    // No WSL available — report native status (which is always broken on Windows)
    ensure_ansible_in_path();
    let version = ansible_version_from_metadata();
    ToolStatus {
        installed: version.is_some(),
        version,
        path: None,
        warning: Some("Ansible does not support Windows as a control node. Install WSL or use an SSH connection to run Ansible on a remote Linux host.".into()),
        local_unsupported: true,
        wsl: false,
    }
}

/// Check if WSL is available on this Windows system.
#[cfg(windows)]
pub fn is_wsl_available() -> bool {
    Command::new("wsl.exe")
        .args(["--list", "--quiet"])
        .output()
        .map(|out| {
            out.status.success()
                && String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .any(|l| !l.trim().trim_matches('\u{feff}').is_empty())
        })
        .unwrap_or(false)
}

/// Convert a Windows path to a WSL /mnt/ path.
/// e.g. "C:\Users\foo\project" -> "/mnt/c/Users/foo/project"
pub fn windows_to_wsl_path(win_path: &str) -> String {
    let path = win_path.replace('\\', "/");
    // Match drive letter: "C:/..." -> "/mnt/c/..."
    if path.len() >= 2 && path.as_bytes()[1] == b':' {
        let drive = (path.as_bytes()[0] as char).to_lowercase().next().unwrap();
        format!("/mnt/{}{}", drive, &path[2..])
    } else {
        path
    }
}

/// Diagnose why `ansible --version` failed and return (version, warning, local_unsupported).
fn diagnose_ansible_failure(stderr: &str) -> (Option<String>, Option<String>, bool) {
    let version = ansible_version_from_metadata();

    if stderr.contains("os.get_blocking") || stderr.contains("OSError") {
        (
            version,
            Some("Ansible does not support this platform as a control node.".into()),
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
