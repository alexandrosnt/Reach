use std::path::PathBuf;
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

/// Install OpenTofu CLI.
///
/// - Windows: downloads the binary from GitHub releases into the tools directory.
/// - Linux/macOS: uses the official install script from get.opentofu.org.
pub async fn install_tofu(app_handle: &tauri::AppHandle) -> Result<String, String> {
    let tool = "tofu";
    emit_progress(app_handle, tool, "Installing OpenTofu...");

    #[cfg(windows)]
    {
        install_tofu_windows(app_handle).await
    }

    #[cfg(not(windows))]
    {
        install_tofu_unix(app_handle).await
    }
}

#[cfg(windows)]
async fn install_tofu_windows(app_handle: &tauri::AppHandle) -> Result<String, String> {
    let tool = "tofu";
    let tools_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("com.reach.app")
        .join("tools");
    let _ = std::fs::create_dir_all(&tools_dir);

    emit_progress(app_handle, tool, "Downloading OpenTofu from GitHub...");

    // Use PowerShell to download and extract the latest release
    let ps_script = format!(
        r#"
$ErrorActionPreference = 'Stop'
$toolsDir = '{}'
$arch = if ([System.Environment]::Is64BitOperatingSystem) {{ 'amd64' }} else {{ '386' }}
$apiUrl = 'https://api.github.com/repos/opentofu/opentofu/releases/latest'
$headers = @{{ 'User-Agent' = 'Reach-App' }}
$release = Invoke-RestMethod -Uri $apiUrl -Headers $headers
$tag = $release.tag_name -replace '^v',''
$zipName = "tofu_{0}_windows_$arch.zip" -f $tag
$asset = $release.assets | Where-Object {{ $_.name -eq $zipName }} | Select-Object -First 1
if (-not $asset) {{ throw "Could not find release asset: $zipName" }}
$zipPath = Join-Path $env:TEMP $zipName
Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $zipPath -Headers $headers
Expand-Archive -Path $zipPath -DestinationPath $toolsDir -Force
Remove-Item $zipPath -Force
$tofuPath = Join-Path $toolsDir 'tofu.exe'
if (Test-Path $tofuPath) {{ Write-Output "OK:$tofuPath" }} else {{ throw 'tofu.exe not found after extraction' }}
"#,
        tools_dir.display()
    );

    let mut child = tokio::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &ps_script])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn PowerShell: {}", e))?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    if let Some(stderr) = stderr {
        let handle = app_handle.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                emit_progress(&handle, "tofu", &line);
            }
        });
    }

    let mut output_lines = Vec::new();
    if let Some(stdout) = stdout {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            output_lines.push(line);
        }
    }

    let status = child
        .wait()
        .await
        .map_err(|e| format!("Failed to wait for installer: {}", e))?;

    if !status.success() {
        let msg = "OpenTofu installation failed".to_string();
        emit_done(app_handle, tool, false, &msg);
        return Err(msg);
    }

    emit_progress(app_handle, tool, "Verifying installation...");
    let check = super::detect::check_tool("tofu");
    if check.installed {
        let version = check.version.unwrap_or_else(|| "unknown".to_string());
        emit_done(app_handle, tool, true, &version);
        Ok(version)
    } else {
        let msg = "OpenTofu was downloaded but could not be found. Try restarting the app.".to_string();
        emit_done(app_handle, tool, false, &msg);
        Err(msg)
    }
}

#[cfg(not(windows))]
async fn install_tofu_unix(app_handle: &tauri::AppHandle) -> Result<String, String> {
    let tool = "tofu";

    // Use the official install script, installing to the app's tools dir
    let tools_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("com.reach.app")
        .join("tools");
    let _ = std::fs::create_dir_all(&tools_dir);

    emit_progress(app_handle, tool, "Downloading OpenTofu via install script...");

    // Try the official cosign-verified method first, fall back to direct download
    let install_cmd = format!(
        "curl -fsSL https://get.opentofu.org/install-opentofu.sh | sh -s -- --install-method standalone --install-path {}",
        tools_dir.display()
    );

    let mut child = tokio::process::Command::new("sh")
        .args(["-c", &install_cmd])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to run install script: {}", e))?;

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
                        tool: "tofu".to_string(),
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
                        tool: "tofu".to_string(),
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
        let msg = "OpenTofu install script failed".to_string();
        emit_done(app_handle, tool, false, &msg);
        return Err(msg);
    }

    emit_progress(app_handle, tool, "Verifying installation...");
    let check = super::detect::check_tool("tofu");
    if check.installed {
        let version = check.version.unwrap_or_else(|| "unknown".to_string());
        emit_done(app_handle, tool, true, &version);
        Ok(version)
    } else {
        let msg = "OpenTofu was installed but could not be found in PATH. Try restarting the app.".to_string();
        emit_done(app_handle, tool, false, &msg);
        Err(msg)
    }
}

/// Install Ansible via pipx or pip (or through WSL on Windows).
pub async fn install_ansible(app_handle: &tauri::AppHandle) -> Result<String, String> {
    let tool = "ansible";

    // On Windows, try installing through WSL if available
    #[cfg(windows)]
    {
        if super::detect::is_wsl_available() {
            return install_ansible_wsl(app_handle).await;
        }
    }

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

/// Install Ansible inside WSL (Windows only).
#[cfg(windows)]
async fn install_ansible_wsl(app_handle: &tauri::AppHandle) -> Result<String, String> {
    let tool = "ansible";

    emit_progress(app_handle, tool, "Installing Ansible via WSL...");
    emit_progress(app_handle, tool, "Detecting package manager inside WSL...");

    // Detect which package manager/pip is available in WSL
    let has_pip3 = std::process::Command::new("wsl.exe")
        .args(["--", "which", "pip3"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    let has_apt = std::process::Command::new("wsl.exe")
        .args(["--", "which", "apt-get"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    // Build the install command
    let install_cmd = if has_pip3 {
        emit_progress(app_handle, tool, "Using pip3 to install Ansible in WSL...");
        "pip3 install --user ansible".to_string()
    } else if has_apt {
        emit_progress(app_handle, tool, "Using apt to install Ansible in WSL...");
        "sudo apt-get update -y && sudo apt-get install -y ansible".to_string()
    } else {
        let msg = "WSL has no supported package manager (pip3 or apt). Install Python 3 in WSL first.";
        emit_done(app_handle, tool, false, msg);
        return Err(msg.to_string());
    };

    let mut child = tokio::process::Command::new("wsl.exe")
        .args(["--", "bash", "-c", &install_cmd])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn wsl.exe: {}", e))?;

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
        .map_err(|e| format!("Failed to wait for WSL installer: {}", e))?;

    if !status.success() {
        let msg = format!(
            "WSL install failed with exit code {}",
            status.code().unwrap_or(-1)
        );
        emit_done(app_handle, tool, false, &msg);
        return Err(msg);
    }

    // Verify ansible is now available in WSL
    emit_progress(app_handle, tool, "Verifying Ansible installation in WSL...");
    let check = super::detect::check_tool("ansible");
    if check.installed {
        let version = check.version.unwrap_or_else(|| "installed via WSL".to_string());
        emit_done(app_handle, tool, true, &version);
        Ok(version)
    } else {
        let msg = "Ansible was installed in WSL but could not be verified.";
        emit_done(app_handle, tool, false, msg);
        Err(msg.to_string())
    }
}
