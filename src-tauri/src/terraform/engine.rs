use std::process::Stdio;

use tauri::Emitter;
use tokio::io::{AsyncBufReadExt, BufReader};

use super::types::{TerraformCompleteEvent, TerraformOutputEvent, TerraformRunStatus};
use crate::ssh::client::{exec_on_connection, exec_on_connection_streaming, SharedHandle};

/// Run a terraform command locally, streaming output as Tauri events.
/// Returns the exit code of the process.
pub async fn run_local(
    action_args: &[&str],
    working_dir: &str,
    run_id: &str,
    app_handle: &tauri::AppHandle,
) -> Result<i32, String> {
    let output_event = format!("tf-output-{}", run_id);
    let complete_event = format!("tf-complete-{}", run_id);

    let mut child = tokio::process::Command::new("terraform")
        .args(action_args)
        .current_dir(working_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn terraform: {}", e))?;

    let stdout = child.stdout.take().ok_or("No stdout")?;
    let stderr = child.stderr.take().ok_or("No stderr")?;

    let stdout_event = output_event.clone();
    let stdout_handle = app_handle.clone();
    let stdout_run_id = run_id.to_string();
    let stdout_task = tokio::spawn(async move {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let _ = stdout_handle.emit(
                &stdout_event,
                TerraformOutputEvent {
                    run_id: stdout_run_id.clone(),
                    stream: "stdout".to_string(),
                    data: line,
                },
            );
        }
    });

    let stderr_event = output_event.clone();
    let stderr_handle = app_handle.clone();
    let stderr_run_id = run_id.to_string();
    let stderr_task = tokio::spawn(async move {
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let _ = stderr_handle.emit(
                &stderr_event,
                TerraformOutputEvent {
                    run_id: stderr_run_id.clone(),
                    stream: "stderr".to_string(),
                    data: line,
                },
            );
        }
    });

    let _ = tokio::join!(stdout_task, stderr_task);

    let status = child
        .wait()
        .await
        .map_err(|e| format!("Failed to wait for terraform: {}", e))?;

    let exit_code = status.code().unwrap_or(-1);

    let run_status = if exit_code == 0 {
        TerraformRunStatus::Completed
    } else {
        TerraformRunStatus::Failed
    };

    let _ = app_handle.emit(
        &complete_event,
        TerraformCompleteEvent {
            run_id: run_id.to_string(),
            status: run_status,
            exit_code: Some(exit_code),
        },
    );

    Ok(exit_code)
}

/// Run a terraform command remotely over SSH, streaming output as Tauri events.
/// Returns the exit code.
pub async fn run_remote(
    action_args: &[&str],
    working_dir: &str,
    run_id: &str,
    handle: &SharedHandle,
    app_handle: &tauri::AppHandle,
) -> Result<i32, String> {
    let complete_event = format!("tf-complete-{}", run_id);

    let args_str = action_args.join(" ");
    let command = format!("cd {} && terraform {}", shell_escape(working_dir), args_str);

    let exit_code = exec_on_connection_streaming(handle, &command, run_id, "tf-output", app_handle)
        .await
        .map_err(|e| e.to_string())?;

    let run_status = if exit_code == 0 {
        TerraformRunStatus::Completed
    } else {
        TerraformRunStatus::Failed
    };

    let _ = app_handle.emit(
        &complete_event,
        TerraformCompleteEvent {
            run_id: run_id.to_string(),
            status: run_status,
            exit_code: Some(exit_code),
        },
    );

    Ok(exit_code)
}

/// Run a terraform command synchronously (collects all output) — local mode.
pub async fn run_local_sync(
    action_args: &[&str],
    working_dir: &str,
) -> Result<String, String> {
    let output = tokio::process::Command::new("terraform")
        .args(action_args)
        .current_dir(working_dir)
        .output()
        .await
        .map_err(|e| format!("Failed to run terraform: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        Err(format!("{}{}", stdout, stderr))
    }
}

/// Run a terraform command synchronously over SSH (collects all output).
pub async fn run_remote_sync(
    action_args: &[&str],
    working_dir: &str,
    handle: &SharedHandle,
) -> Result<String, String> {
    let args_str = action_args.join(" ");
    let command = format!("cd {} && terraform {}", shell_escape(working_dir), args_str);

    exec_on_connection(handle, &command)
        .await
        .map_err(|e| e.to_string())
}

/// Build the CLI args for a terraform action.
pub fn build_args(action: &str) -> Vec<&str> {
    match action {
        "init" => vec!["init", "-no-color"],
        "plan" => vec!["plan", "-no-color"],
        "apply" => vec!["apply", "-auto-approve", "-no-color"],
        "destroy" => vec!["destroy", "-auto-approve", "-no-color"],
        _ => vec![action, "-no-color"],
    }
}

/// Minimal shell escaping for paths used in `cd` commands over SSH.
fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}
