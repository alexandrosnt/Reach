use std::process::Stdio;

use tauri::Emitter;
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::ssh::client::{exec_on_connection_streaming, SshManager};
use crate::tofu::types::{TofuCommand, TofuCommandEvent, TofuCommandRequest};

/// Build CLI argument list for a tofu command.
pub fn build_command_args(request: &TofuCommandRequest) -> Vec<String> {
    let mut args = vec![request.command.as_str().to_string()];

    // Sub-commands
    match request.command {
        TofuCommand::ProvidersSchema => {
            args.push("schema".to_string());
            args.push("-json".to_string());
        }
        TofuCommand::StateList => {
            args.push("list".to_string());
        }
        TofuCommand::StateShow => {
            args.push("show".to_string());
        }
        TofuCommand::StateRm => {
            args.push("rm".to_string());
        }
        TofuCommand::StateMv => {
            args.push("mv".to_string());
        }
        TofuCommand::Import => {}
        TofuCommand::FmtCheck => {
            args.push("-check".to_string());
        }
        TofuCommand::WorkspaceList => {
            args.push("list".to_string());
        }
        TofuCommand::WorkspaceNew => {
            args.push("new".to_string());
        }
        TofuCommand::WorkspaceSelect => {
            args.push("select".to_string());
        }
        TofuCommand::WorkspaceDelete => {
            args.push("delete".to_string());
        }
        _ => {}
    }

    // Auto-approve for apply/destroy
    if request.auto_approve {
        match request.command {
            TofuCommand::Apply | TofuCommand::Destroy => {
                args.push("-auto-approve".to_string());
            }
            _ => {}
        }
    }

    // Save plan output for later viewing
    if matches!(request.command, TofuCommand::Plan) {
        args.push("-out=.reach-plan".to_string());
    }

    // Input=false to prevent interactive prompts
    match request.command {
        TofuCommand::Apply | TofuCommand::Plan | TofuCommand::Destroy => {
            args.push("-input=false".to_string());
        }
        _ => {}
    }

    // Var file
    if let Some(ref var_file) = request.var_file {
        args.push(format!("-var-file={}", var_file));
    }

    // Extra args
    args.extend(request.extra_args.clone());

    // No color for easier parsing
    match request.command {
        TofuCommand::Init
        | TofuCommand::Plan
        | TofuCommand::Apply
        | TofuCommand::Destroy
        | TofuCommand::Validate
        | TofuCommand::StateShow
        | TofuCommand::StateRm
        | TofuCommand::StateMv
        | TofuCommand::Import
        | TofuCommand::Fmt
        | TofuCommand::FmtCheck
        | TofuCommand::WorkspaceList
        | TofuCommand::WorkspaceNew
        | TofuCommand::WorkspaceSelect
        | TofuCommand::WorkspaceDelete => {
            args.push("-no-color".to_string());
        }
        _ => {}
    }

    args
}

/// Execute a tofu command locally, streaming output via Tauri events.
pub async fn run_local(
    working_dir: &str,
    args: &[String],
    run_id: &str,
    app_handle: &tauri::AppHandle,
) -> Result<i32, String> {
    let event_name = format!("tofu-output-{}", run_id);

    // Find tofu binary (try tofu first, then terraform as fallback)
    let binary = if which::which("tofu").is_ok() {
        "tofu"
    } else if which::which("terraform").is_ok() {
        "terraform"
    } else {
        let _ = app_handle.emit(
            &event_name,
            TofuCommandEvent {
                run_id: run_id.to_string(),
                stream: "stderr".to_string(),
                line: "OpenTofu/Terraform CLI not found. Please install OpenTofu first."
                    .to_string(),
                done: true,
                exit_code: Some(1),
            },
        );
        return Err("OpenTofu CLI not found".to_string());
    };

    let mut child = tokio::process::Command::new(binary)
        .args(args)
        .current_dir(working_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to spawn tofu: {}", e))?;

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
                    TofuCommandEvent {
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
                    TofuCommandEvent {
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
        .map_err(|e| format!("Failed to wait for tofu: {}", e))?;

    let exit_code = status.code().unwrap_or(-1);

    // Emit done event
    let _ = app_handle.emit(
        &event_name,
        TofuCommandEvent {
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

/// Execute a tofu command on a remote SSH connection, streaming output via Tauri events.
pub async fn run_remote(
    connection_id: &str,
    working_dir: &str,
    args: &[String],
    run_id: &str,
    app_handle: &tauri::AppHandle,
    ssh_manager: &mut SshManager,
) -> Result<i32, String> {
    let event_name = format!("tofu-output-{}", run_id);

    // Build the full command string for remote execution
    let cmd = format!("cd {} && tofu {}", shell_escape(working_dir), args.join(" "));

    let handle = ssh_manager
        .get_handle(connection_id)
        .map_err(|e| e.to_string())?;

    let exit_code =
        exec_on_connection_streaming(&handle, &cmd, run_id, "tofu-output", app_handle)
            .await
            .map_err(|e| e.to_string())?;

    // Emit done event
    let _ = app_handle.emit(
        &event_name,
        TofuCommandEvent {
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
