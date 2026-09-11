use std::process::Stdio;

use tauri::Emitter;
use tokio::io::{AsyncBufReadExt, BufReader};

fn silent_async_command(program: impl AsRef<std::ffi::OsStr>) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    cmd
}

fn silent_command(program: impl AsRef<std::ffi::OsStr>) -> std::process::Command {
    let mut cmd = std::process::Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    cmd
}

use crate::ssh::client::{exec_on_connection_streaming, SshManager};
use crate::ansible::types::{AnsibleExecutionTarget, AnsibleCommand, AnsibleCommandEvent, AnsibleCommandRequest};
use crate::toolchain::detect::windows_to_wsl_path;

/// Build CLI binary name and argument list for an Ansible command.
/// `vault_pass_file`: a path — local or remote, the caller knows which —
/// holding the vault password, handed to Ansible as a file rather than an
/// argument so it never shows in a process list.
pub fn build_command_args(
    request: &AnsibleCommandRequest,
    vault_pass_file: Option<&str>,
) -> (String, Vec<String>) {
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

    // The vault password, for the commands that read vaulted content.
    if let Some(f) = vault_pass_file {
        if matches!(
            request.command,
            AnsibleCommand::Playbook
                | AnsibleCommand::SyntaxCheck
                | AnsibleCommand::AdHoc
                | AnsibleCommand::VaultEncrypt
                | AnsibleCommand::VaultDecrypt
                | AnsibleCommand::VaultView
                | AnsibleCommand::Inventory
        ) {
            args.push("--vault-password-file".to_string());
            args.push(f.to_string());
        }
    }

    // Extra args
    args.extend(request.extra_args.clone());

    (binary, args)
}

/// The engine-neutral tail of a local run: stream both pipes, wait, report.
async fn pump(
    mut child: tokio::process::Child,
    run_id: &str,
    app_handle: &tauri::AppHandle,
) -> Result<i32, String> {
    let event_name = format!("ansible-output-{}", run_id);
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

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
                    AnsibleCommandEvent { run_id: rid.clone(), stream: "stdout".to_string(), line, done: false, exit_code: None },
                );
            }
        });
    }
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
                    AnsibleCommandEvent { run_id: rid.clone(), stream: "stderr".to_string(), line, done: false, exit_code: None },
                );
            }
        });
    }

    let status = child.wait().await.map_err(|e| format!("Failed to wait for process: {}", e))?;
    let exit_code = status.code().unwrap_or(-1);
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

pub fn fail_run(run_id: &str, app_handle: &tauri::AppHandle, msg: &str) {
    let _ = app_handle.emit(
        &format!("ansible-output-{}", run_id),
        AnsibleCommandEvent {
            run_id: run_id.to_string(),
            stream: "stderr".to_string(),
            line: msg.to_string(),
            done: true,
            exit_code: Some(1),
        },
    );
}

/// The native engine: Ansible installed on this machine.
pub async fn run_local(
    working_dir: &str,
    binary: &str,
    args: &[String],
    run_id: &str,
    app_handle: &tauri::AppHandle,
) -> Result<i32, String> {
    crate::toolchain::detect::ensure_ansible_in_path();
    if which::which(binary).is_err() {
        fail_run(run_id, app_handle, &format!("{} is not installed on this machine.", binary));
        return Err(format!("{} not found", binary));
    }
    let child = silent_async_command(binary)
        .args(args)
        .current_dir(working_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .env("ANSIBLE_FORCE_COLOR", "0")
        .env("ANSIBLE_NOCOLOR", "1")
        .spawn()
        .map_err(|e| format!("Failed to spawn {}: {}", binary, e))?;
    pump(child, run_id, app_handle).await
}

/// The WSL engine: the project stays on the Windows filesystem and is
/// reached through /mnt, so nothing is copied.
pub async fn run_wsl(
    working_dir: &str,
    binary: &str,
    args: &[String],
    run_id: &str,
    app_handle: &tauri::AppHandle,
) -> Result<i32, String> {
    let wsl_dir = windows_to_wsl_path(working_dir);
    let escaped_args: Vec<String> = args.iter().map(|a| shell_escape(a)).collect();
    let cmd_str = format!(
        "cd {} && ANSIBLE_FORCE_COLOR=0 ANSIBLE_NOCOLOR=1 {} {}",
        shell_escape(&wsl_dir),
        binary,
        escaped_args.join(" ")
    );
    // A login shell, so ~/.local/bin (where pipx puts ansible) is on PATH.
    let child = silent_async_command("wsl.exe")
        .args(["--", "bash", "-lc", &cmd_str])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to spawn wsl.exe: {}", e))?;
    pump(child, run_id, app_handle).await
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

    // The project was synced to `working_dir` on the far side already.
    let escaped_args: Vec<String> = args.iter().map(|a| shell_escape(a)).collect();
    let cmd = format!(
        "cd {} && ANSIBLE_FORCE_COLOR=0 ANSIBLE_NOCOLOR=1 {} {}",
        working_dir,
        binary,
        escaped_args.join(" ")
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

#[cfg(test)]
mod tests {
    use super::*;

    fn req(command: AnsibleCommand) -> AnsibleCommandRequest {
        AnsibleCommandRequest {
            project_id: "p".into(),
            command,
            target: AnsibleExecutionTarget::Local,
            playbook: Some("site.yml".into()),
            inventory_file: Some("inventory.ini".into()),
            module_name: None,
            module_args: None,
            host_pattern: None,
            role_name: None,
            collection_name: None,
            vault_file: None,
            extra_args: vec![],
        }
    }

    #[test]
    fn the_vault_password_travels_as_a_file() {
        let (bin, args) = build_command_args(&req(AnsibleCommand::Playbook), Some("/tmp/v.pass"));
        assert_eq!(bin, "ansible-playbook");
        let i = args.iter().position(|a| a == "--vault-password-file").expect("flag");
        assert_eq!(args[i + 1], "/tmp/v.pass");
        assert!(!args.iter().any(|a| a.contains("hunter")), "never the secret itself");
    }

    #[test]
    fn galaxy_never_sees_the_vault_password() {
        let (_, args) = build_command_args(&req(AnsibleCommand::GalaxyCollectionList), Some("/tmp/v.pass"));
        assert!(!args.iter().any(|a| a == "--vault-password-file"));
    }

    #[test]
    fn without_a_password_nothing_is_added() {
        let (_, args) = build_command_args(&req(AnsibleCommand::Playbook), None);
        assert_eq!(args, vec!["site.yml", "-i", "inventory.ini"]);
    }
}
