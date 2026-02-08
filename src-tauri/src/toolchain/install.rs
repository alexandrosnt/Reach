use std::path::{Path, PathBuf};
use std::process::Stdio;

use serde::Serialize;
use tauri::Emitter;
use tokio::io::{AsyncBufReadExt, BufReader};

#[derive(Debug, Clone, Serialize)]
pub struct ToolInstallEvent {
    pub tool: String,
    pub message: String,
    pub done: bool,
    pub success: bool,
}

fn emit_progress(app_handle: &tauri::AppHandle, tool: &str, message: &str) {
    let _ = app_handle.emit(
        &format!("toolchain-install-{}", tool),
        ToolInstallEvent {
            tool: tool.to_string(),
            message: message.to_string(),
            done: false,
            success: false,
        },
    );
}

fn emit_done(app_handle: &tauri::AppHandle, tool: &str, success: bool, message: &str) {
    let _ = app_handle.emit(
        &format!("toolchain-install-{}", tool),
        ToolInstallEvent {
            tool: tool.to_string(),
            message: message.to_string(),
            done: true,
            success,
        },
    );
}

/// Get the tools directory inside the app data dir.
pub fn tools_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("tools")
}

/// Determine platform and arch for Terraform download URLs.
fn terraform_platform() -> Result<(&'static str, &'static str), String> {
    let os = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        return Err("Unsupported operating system".to_string());
    };

    let arch = if cfg!(target_arch = "x86_64") {
        "amd64"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        return Err("Unsupported architecture".to_string());
    };

    Ok((os, arch))
}

/// Install Terraform by downloading the official binary.
pub async fn install_terraform(
    app_handle: &tauri::AppHandle,
    data_dir: &Path,
) -> Result<String, String> {
    let tool = "terraform";
    let (os, arch) = terraform_platform()?;

    emit_progress(app_handle, tool, "Fetching latest version...");

    // Get latest version from checkpoint API
    let client = reqwest::Client::new();
    let checkpoint: serde_json::Value = client
        .get("https://checkpoint-api.hashicorp.com/v1/check/terraform")
        .send()
        .await
        .map_err(|e| format!("Failed to check latest version: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse version response: {}", e))?;

    let version = checkpoint["current_version"]
        .as_str()
        .ok_or_else(|| "Could not determine latest Terraform version".to_string())?
        .to_string();

    emit_progress(
        app_handle,
        tool,
        &format!("Downloading Terraform v{}...", version),
    );

    let ext = if cfg!(target_os = "windows") {
        "terraform.exe"
    } else {
        "terraform"
    };

    let zip_url = format!(
        "https://releases.hashicorp.com/terraform/{}/terraform_{}_{}_{}.zip",
        version, version, os, arch
    );

    let response = client
        .get(&zip_url)
        .send()
        .await
        .map_err(|e| format!("Failed to download Terraform: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "Download failed with status: {}",
            response.status()
        ));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read download: {}", e))?;

    emit_progress(app_handle, tool, "Extracting...");

    let dest_dir = tools_dir(data_dir);
    std::fs::create_dir_all(&dest_dir)
        .map_err(|e| format!("Failed to create tools directory: {}", e))?;

    // Extract the zip in a blocking task
    let dest_dir_clone = dest_dir.clone();
    let ext_name = ext.to_string();
    tokio::task::spawn_blocking(move || {
        let cursor = std::io::Cursor::new(bytes);
        let mut archive =
            zip::ZipArchive::new(cursor).map_err(|e| format!("Failed to open zip: {}", e))?;

        for i in 0..archive.len() {
            let mut file = archive
                .by_index(i)
                .map_err(|e| format!("Failed to read zip entry: {}", e))?;
            let name = file.name().to_string();
            if name.ends_with(&ext_name) || name == ext_name {
                let dest_path = dest_dir_clone.join(&ext_name);
                let mut out = std::fs::File::create(&dest_path)
                    .map_err(|e| format!("Failed to create file: {}", e))?;
                std::io::copy(&mut file, &mut out)
                    .map_err(|e| format!("Failed to extract file: {}", e))?;

                // Make executable on Unix
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    std::fs::set_permissions(&dest_path, std::fs::Permissions::from_mode(0o755))
                        .map_err(|e| format!("Failed to set permissions: {}", e))?;
                }
                break;
            }
        }
        Ok::<(), String>(())
    })
    .await
    .map_err(|e| format!("Extract task failed: {}", e))?
    .map_err(|e: String| e)?;

    let version_str = format!("Terraform v{}", version);
    emit_done(app_handle, tool, true, &version_str);

    Ok(version_str)
}

/// Install Ansible via pipx or pip.
pub async fn install_ansible(app_handle: &tauri::AppHandle) -> Result<String, String> {
    let tool = "ansible";

    // Try pipx first, then pip3, then pip
    let (installer, args) = if which::which("pipx").is_ok() {
        emit_progress(app_handle, tool, "Installing Ansible via pipx...");
        ("pipx", vec!["install", "ansible"])
    } else if which::which("pip3").is_ok() {
        emit_progress(app_handle, tool, "Installing Ansible via pip3...");
        ("pip3", vec!["install", "--user", "ansible"])
    } else if which::which("pip").is_ok() {
        emit_progress(app_handle, tool, "Installing Ansible via pip...");
        ("pip", vec!["install", "--user", "ansible"])
    } else {
        emit_done(
            app_handle,
            tool,
            false,
            "Python 3 is required to install Ansible",
        );
        return Err("Python 3 is required to install Ansible. Please install Python 3 and try again.".to_string());
    };

    let mut child = tokio::process::Command::new(installer)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn {}: {}", installer, e))?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let event_name = format!("toolchain-install-{}", tool);

    if let Some(stdout) = stdout {
        let event = event_name.clone();
        let handle = app_handle.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = handle.emit(
                    &event,
                    ToolInstallEvent {
                        tool: "ansible".to_string(),
                        message: line,
                        done: false,
                        success: false,
                    },
                );
            }
        });
    }

    if let Some(stderr) = stderr {
        let event = event_name.clone();
        let handle = app_handle.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = handle.emit(
                    &event,
                    ToolInstallEvent {
                        tool: "ansible".to_string(),
                        message: line,
                        done: false,
                        success: false,
                    },
                );
            }
        });
    }

    let status = child
        .wait()
        .await
        .map_err(|e| format!("Failed to wait for installer: {}", e))?;

    if !status.success() {
        let msg = format!(
            "{} install failed with exit code {}",
            installer,
            status.code().unwrap_or(-1)
        );
        emit_done(app_handle, tool, false, &msg);
        return Err(msg);
    }

    // After successful install, discover and add Python scripts directory to PATH
    emit_progress(app_handle, tool, "Verifying installation...");
    add_python_scripts_to_path(installer);

    // Try 1: check if ansible is now in PATH
    let check = super::detect::check_tool("ansible");
    if check.installed {
        let version = check.version.unwrap_or_else(|| "unknown".to_string());
        emit_done(app_handle, tool, true, &version);
        return Ok(version);
    }

    // Try 2: get version via pip metadata (doesn't import ansible, avoids Python compat issues)
    let pythons: &[&str] = if cfg!(windows) {
        &["python", "python3", "py"]
    } else {
        &["python3", "python"]
    };

    for python in pythons {
        if which::which(python).is_err() {
            continue;
        }
        if let Ok(output) = std::process::Command::new(python)
            .args(["-c", "from importlib.metadata import version; print(version('ansible'))"])
            .output()
        {
            if output.status.success() {
                let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !version.is_empty() {
                    let version_str = format!("ansible {}", version);
                    emit_done(app_handle, tool, true, &version_str);
                    return Ok(version_str);
                }
            }
        }
    }

    // Try 3: search for ansible binary directly in known locations
    if let Some(found) = find_ansible_binary() {
        // Add its parent directory to PATH
        if let Some(parent) = found.parent() {
            let sep = if cfg!(windows) { ";" } else { ":" };
            let current_path = std::env::var("PATH").unwrap_or_default();
            let parent_str = parent.to_string_lossy().to_string();
            if !current_path.contains(&parent_str) {
                std::env::set_var("PATH", format!("{}{}{}", parent_str, sep, current_path));
            }
        }
        // Try to get version from the found binary
        if let Ok(output) = std::process::Command::new(&found).arg("--version").output() {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let first_line = stdout.lines().next().unwrap_or("ansible (installed)").trim().to_string();
            emit_done(app_handle, tool, true, &first_line);
            return Ok(first_line);
        }
        let msg = format!("ansible ({})", found.display());
        emit_done(app_handle, tool, true, &msg);
        return Ok(msg);
    }

    let msg = "Ansible was installed but could not be found in PATH. You may need to restart the application.".to_string();
    emit_done(app_handle, tool, false, &msg);
    Err(msg)
}

/// Search common locations for the ansible binary.
pub fn find_ansible_binary() -> Option<PathBuf> {
    let bin_name = if cfg!(windows) { "ansible.exe" } else { "ansible" };

    // Check pipx location
    if let Some(home) = dirs::home_dir() {
        let pipx_path = home.join(".local").join("bin").join(bin_name);
        if pipx_path.exists() {
            return Some(pipx_path);
        }
    }

    #[cfg(not(windows))]
    {
        if let Some(home) = dirs::home_dir() {
            let local_bin = home.join(".local").join("bin").join(bin_name);
            if local_bin.exists() {
                return Some(local_bin);
            }
        }
        // Check common system paths
        for dir in &["/usr/local/bin", "/usr/bin"] {
            let p = PathBuf::from(dir).join(bin_name);
            if p.exists() {
                return Some(p);
            }
        }
    }

    #[cfg(windows)]
    {
        // Check %APPDATA%\Python\PythonXY\Scripts
        if let Some(appdata) = dirs::data_dir() {
            let python_dir = appdata.join("Python");
            if python_dir.exists() {
                if let Ok(entries) = std::fs::read_dir(&python_dir) {
                    for entry in entries.flatten() {
                        let candidate = entry.path().join("Scripts").join(bin_name);
                        if candidate.exists() {
                            return Some(candidate);
                        }
                    }
                }
            }
        }
        // Check %LOCALAPPDATA%\Programs\Python\PythonXY\Scripts
        if let Some(local_appdata) = dirs::data_local_dir() {
            let python_dir = local_appdata.join("Programs").join("Python");
            if python_dir.exists() {
                if let Ok(entries) = std::fs::read_dir(&python_dir) {
                    for entry in entries.flatten() {
                        let candidate = entry.path().join("Scripts").join(bin_name);
                        if candidate.exists() {
                            return Some(candidate);
                        }
                    }
                }
            }
        }
        // Check %USERPROFILE%\.local\bin (pipx on Windows)
        if let Some(home) = dirs::home_dir() {
            let candidate = home.join(".local").join("bin").join(bin_name);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    None
}

/// Discover and add all known Python scripts directories to process PATH.
/// Called during detection when ansible is found via metadata but not via `which`.
pub fn ensure_python_scripts_in_path() {
    let sep = if cfg!(windows) { ";" } else { ":" };
    let current_path = std::env::var("PATH").unwrap_or_default();
    let mut new_dirs: Vec<PathBuf> = Vec::new();

    // pipx: ~/.local/bin
    if let Some(home) = dirs::home_dir() {
        let pipx_bin = home.join(".local").join("bin");
        if pipx_bin.exists() && !current_path.contains(&pipx_bin.to_string_lossy().to_string()) {
            new_dirs.push(pipx_bin);
        }
    }

    // pip --user: ask Python for the user scripts directory
    let pythons: &[&str] = if cfg!(windows) {
        &["python", "python3", "py"]
    } else {
        &["python3", "python"]
    };
    for python in pythons {
        if which::which(python).is_err() {
            continue;
        }
        if let Ok(output) = std::process::Command::new(python)
            .args([
                "-c",
                "import sysconfig; print(sysconfig.get_path('scripts', '{}_user'.format('nt' if __import__('os').name == 'nt' else 'posix_prefix')))",
            ])
            .output()
        {
            let dir = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !dir.is_empty() {
                let p = PathBuf::from(&dir);
                if p.exists() && !current_path.contains(&dir) {
                    new_dirs.push(p);
                }
            }
        }
        break; // only need one Python interpreter
    }

    #[cfg(not(windows))]
    {
        if let Some(home) = dirs::home_dir() {
            let local_bin = home.join(".local").join("bin");
            if local_bin.exists()
                && !current_path.contains(&local_bin.to_string_lossy().to_string())
            {
                new_dirs.push(local_bin);
            }
        }
    }

    #[cfg(windows)]
    {
        if let Some(appdata) = dirs::data_dir() {
            let python_dir = appdata.join("Python");
            if python_dir.exists() {
                if let Ok(entries) = std::fs::read_dir(&python_dir) {
                    for entry in entries.flatten() {
                        let scripts = entry.path().join("Scripts");
                        if scripts.exists() {
                            let s = scripts.to_string_lossy().to_string();
                            if !current_path.contains(&s) {
                                new_dirs.push(scripts);
                            }
                        }
                    }
                }
            }
        }
    }

    if !new_dirs.is_empty() {
        let current = std::env::var("PATH").unwrap_or_default();
        let additions: Vec<String> = new_dirs
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        let new_path = format!("{}{}{}", additions.join(sep), sep, current);
        std::env::set_var("PATH", new_path);
    }
}

/// After pip/pipx install, discover where scripts were placed and add to process PATH.
fn add_python_scripts_to_path(installer: &str) {
    let sep = if cfg!(windows) { ";" } else { ":" };
    let current_path = std::env::var("PATH").unwrap_or_default();
    let mut new_dirs: Vec<PathBuf> = Vec::new();

    if installer == "pipx" {
        // pipx installs to ~/.local/bin (Unix) or %USERPROFILE%\.local\bin (Windows)
        if let Some(home) = dirs::home_dir() {
            let pipx_bin = home.join(".local").join("bin");
            if pipx_bin.exists() && !current_path.contains(&pipx_bin.to_string_lossy().to_string()) {
                new_dirs.push(pipx_bin);
            }
        }
    } else {
        // pip --user: discover the user scripts directory via Python
        let python = if which::which("python3").is_ok() {
            "python3"
        } else {
            "python"
        };

        if let Ok(output) = std::process::Command::new(python)
            .args([
                "-c",
                "import sysconfig; print(sysconfig.get_path('scripts', '{}_user'.format('nt' if __import__('os').name == 'nt' else 'posix_prefix')))",
            ])
            .output()
        {
            let scripts_dir = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !scripts_dir.is_empty() {
                let p = PathBuf::from(&scripts_dir);
                if p.exists() && !current_path.contains(&scripts_dir) {
                    new_dirs.push(p);
                }
            }
        }

        // Also check the common user scripts directory directly as fallback
        // Unix: ~/.local/bin
        #[cfg(not(windows))]
        {
            if let Some(home) = dirs::home_dir() {
                let local_bin = home.join(".local").join("bin");
                if local_bin.exists() && !current_path.contains(&local_bin.to_string_lossy().to_string()) {
                    new_dirs.push(local_bin);
                }
            }
        }
        // Windows: %APPDATA%\Python\PythonXY\Scripts
        #[cfg(windows)]
        {
            if let Some(appdata) = dirs::data_dir() {
                let python_dir = appdata.join("Python");
                if python_dir.exists() {
                    if let Ok(entries) = std::fs::read_dir(&python_dir) {
                        for entry in entries.flatten() {
                            let scripts = entry.path().join("Scripts");
                            if scripts.exists() {
                                let s = scripts.to_string_lossy().to_string();
                                if !current_path.contains(&s) {
                                    new_dirs.push(scripts);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if !new_dirs.is_empty() {
        let additions: Vec<String> = new_dirs.iter().map(|p| p.to_string_lossy().to_string()).collect();
        let new_path = format!("{}{}{}", additions.join(sep), sep, current_path);
        std::env::set_var("PATH", new_path);
    }
}
