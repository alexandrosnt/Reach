use std::process::Stdio;

use tauri::Emitter;
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::ssh::client::{exec_on_connection_streaming, SshManager};
use crate::ansible::types::{AnsibleCommand, AnsibleCommandEvent, AnsibleCommandRequest};
use crate::toolchain::detect::windows_to_wsl_path;

/// Build CLI binary name and argument list for an Ansible command.
pub fn build_command_args(request: &AnsibleCommandRequest) -> (String, Vec<String>) {
    let mut args = Vec::new();

    let binary = match request.command {
        AnsibleCommand::Playbook | AnsibleCommand::SyntaxCheck => {
            if let Some(ref playbook) = request.playbook {
                args.push(playbook.clone());
            }
            if let Some(ref inv) = request.inventory_file {
                args.push("-i".to_string());
                args.push(inv.clone());
            }
            if matches!(request.command, AnsibleCommand::SyntaxCheck) {
                args.push("--syntax-check".to_string());
            }
            "ansible-playbook".to_string()
        }
        AnsibleCommand::AdHoc => {
            if let Some(ref pattern) = request.host_pattern {
                args.push(pattern.clone());
            } else {
                args.push("all".to_string());
            }
            if let Some(ref module) = request.module_name {
                args.push("-m".to_string());
                args.push(module.clone());
            }
            if let Some(ref margs) = request.module_args {
                args.push("-a".to_string());
                args.push(margs.clone());
            }
            if let Some(ref inv) = request.inventory_file {
                args.push("-i".to_string());
                args.push(inv.clone());
            }
            "ansible".to_string()
        }
        AnsibleCommand::GalaxyRoleInstall => {
            args.push("role".to_string());
            args.push("install".to_string());
            if let Some(ref name) = request.role_name {
                args.push(name.clone());
            }
            "ansible-galaxy".to_string()
        }
        AnsibleCommand::GalaxyRoleList => {
            args.push("role".to_string());
            args.push("list".to_string());
            "ansible-galaxy".to_string()
        }
        AnsibleCommand::GalaxyRoleRemove => {
            args.push("role".to_string());
            args.push("remove".to_string());
            if let Some(ref name) = request.role_name {
                args.push(name.clone());
            }
            "ansible-galaxy".to_string()
        }
        AnsibleCommand::GalaxyCollectionInstall => {
            args.push("collection".to_string());
            args.push("install".to_string());
            if let Some(ref name) = request.collection_name {
                args.push(name.clone());
            }
            "ansible-galaxy".to_string()
        }
        AnsibleCommand::GalaxyCollectionList => {
            args.push("collection".to_string());
            args.push("list".to_string());
            "ansible-galaxy".to_string()
        }
        AnsibleCommand::VaultEncrypt => {
            args.push("encrypt".to_string());
            if let Some(ref file) = request.vault_file {
                args.push(file.clone());
            }
            "ansible-vault".to_string()
        }
        AnsibleCommand::VaultDecrypt => {
            args.push("decrypt".to_string());
            if let Some(ref file) = request.vault_file {
                args.push(file.clone());
            }
            "ansible-vault".to_string()
        }
        AnsibleCommand::VaultView => {
            args.push("view".to_string());
            if let Some(ref file) = request.vault_file {
                args.push(file.clone());
            }
            "ansible-vault".to_string()
        }
        AnsibleCommand::Inventory => {
            if let Some(ref inv) = request.inventory_file {
                args.push("-i".to_string());
                args.push(inv.clone());
            }
            args.push("--list".to_string());
            "ansible-inventory".to_string()
        }
    };

    // Extra args
    args.extend(request.extra_args.clone());

    (binary, args)
}

/// Execute an Ansible command locally, streaming output via Tauri events.
/// On Windows, automatically routes through WSL if the binary isn't available natively.
pub async fn run_local(
    working_dir: &str,
    binary: &str,
    args: &[String],
    run_id: &str,
    app_handle: &tauri::AppHandle,
) -> Result<i32, String> {
    let event_name = format!("ansible-output-{}", run_id);

    // Determine if we should run through WSL
    let use_wsl = should_use_wsl(binary);

    let mut child = if use_wsl {
        // Run through WSL: wsl.exe -- <binary> <args...>
        let wsl_dir = windows_to_wsl_path(working_dir);
        let mut wsl_args = vec![
            "--".to_string(),
            "bash".to_string(),
            "-c".to_string(),
        ];
        // Build single command string: cd <dir> && ANSIBLE_FORCE_COLOR=0 ANSIBLE_NOCOLOR=1 <binary> <args>
        let escaped_args: Vec<String> = args.iter().map(|a| shell_escape(a)).collect();
        let cmd_str = format!(
            "cd {} && ANSIBLE_FORCE_COLOR=0 ANSIBLE_NOCOLOR=1 {} {}",
            shell_escape(&wsl_dir),
            binary,
            escaped_args.join(" ")
        );
        wsl_args.push(cmd_str);

        let _ = app_handle.emit(
            &event_name,
            AnsibleCommandEvent {
                run_id: run_id.to_string(),
                stream: "system".to_string(),
                line: format!("Running via WSL: {} {}", binary, args.join(" ")),
                done: false,
                exit_code: None,
            },
        );

        tokio::process::Command::new("wsl.exe")
            .args(&wsl_args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to spawn wsl.exe: {}", e))?
    } else {
        // Check if the binary exists natively
        if which::which(binary).is_err() {
            let _ = app_handle.emit(
                &event_name,
                AnsibleCommandEvent {
                    run_id: run_id.to_string(),
                    stream: "stderr".to_string(),
                    line: format!("{} not found. Please install Ansible first.", binary),
                    done: true,
                    exit_code: Some(1),
                },
            );
            return Err(format!("{} not found", binary));
        }

        tokio::process::Command::new(binary)
            .args(args)
            .current_dir(working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null())
            .env("ANSIBLE_FORCE_COLOR", "0")
            .env("ANSIBLE_NOCOLOR", "1")
            .spawn()
            .map_err(|e| format!("Failed to spawn {}: {}", binary, e))?
    };

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    // Stream stdout
    if let Some(stdout) = stdout {
        let event = event_name.clone();
        let handle = app_handle.clone();
        let rid = run_id.to_string();
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = handle.emit(
                    &event,
                    AnsibleCommandEvent {
                        run_id: rid.clone(),
                        stream: "stdout".to_string(),
                        line,
                        done: false,
                        exit_code: None,
                    },
                );
            }
        });
    }

    // Stream stderr
    if let Some(stderr) = stderr {
        let event = event_name.clone();
        let handle = app_handle.clone();
        let rid = run_id.to_string();
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = handle.emit(
                    &event,
                    AnsibleCommandEvent {
                        run_id: rid.clone(),
                        stream: "stderr".to_string(),
                        line,
                        done: false,
                        exit_code: None,
                    },
                );
            }
        });
    }

    let status = child
        .wait()
        .await
        .map_err(|e| format!("Failed to wait for process: {}", e))?;

    let exit_code = status.code().unwrap_or(-1);

    // Emit done event
    let _ = app_handle.emit(
        &event_name,
        AnsibleCommandEvent {
            run_id: run_id.to_string(),
            stream: "system".to_string(),
            line: if exit_code == 0 {
                "Command completed successfully.".to_string()
            } else {
                format!("Command exited with code {}.", exit_code)
            },
            done: true,
            exit_code: Some(exit_code),
        },
    );

    Ok(exit_code)
}

/// Determine if we should run ansible through WSL.
/// On Windows, always prefer WSL since native Ansible is broken on Windows
/// (os.get_blocking / OSError). Only runs natively on non-Windows platforms.
fn should_use_wsl(binary: &str) -> bool {
    #[cfg(not(windows))]
    { let _ = binary; return false; }

    #[cfg(windows)]
    {
        // On Windows, always prefer WSL for ansible commands since
        // native ansible doesn't work (os.get_blocking error).
        // Use login shell so ~/.local/bin is in PATH.
        std::process::Command::new("wsl.exe")
            .args(["--", "bash", "-lc", &format!("which {} 2>/dev/null || command -v {} 2>/dev/null", binary, binary)])
            .output()
            .map(|out| out.status.success() && !String::from_utf8_lossy(&out.stdout).trim().is_empty())
            .unwrap_or(false)
    }
}

/// Execute an Ansible command on a remote SSH connection, streaming output via Tauri events.
pub async fn run_remote(
    connection_id: &str,
    working_dir: &str,
    binary: &str,
    args: &[String],
    run_id: &str,
    app_handle: &tauri::AppHandle,
    ssh_manager: &mut SshManager,
) -> Result<i32, String> {
    let event_name = format!("ansible-output-{}", run_id);

    // Build the full command string for remote execution
    let cmd = format!(
        "cd {} && {} {}",
        shell_escape(working_dir),
        binary,
        args.join(" ")
    );

    let handle = ssh_manager
        .get_handle(connection_id)
        .map_err(|e| e.to_string())?;

    let exit_code =
        exec_on_connection_streaming(&handle, &cmd, run_id, "ansible-output", app_handle)
            .await
            .map_err(|e| e.to_string())?;

    // Emit done event
    let _ = app_handle.emit(
        &event_name,
        AnsibleCommandEvent {
            run_id: run_id.to_string(),
            stream: "system".to_string(),
            line: if exit_code == 0 {
                "Command completed successfully.".to_string()
            } else {
                format!("Command exited with code {}.", exit_code)
            },
            done: true,
            exit_code: Some(exit_code),
        },
    );

    Ok(exit_code)
}

/// Basic shell escaping for paths in remote commands.
pub fn shell_escape(s: &str) -> String {
    if s.contains(' ') || s.contains('\'') || s.contains('"') {
        format!("'{}'", s.replace('\'', "'\\''"))
    } else {
        s.to_string()
    }
}
