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

use crate::ssh::client::{exec_on_connection_streaming, SshManager};
use crate::tofu::binary;
use crate::tofu::types::{TofuCommand, TofuCommandEvent, TofuCommandRequest};
use std::path::Path;

/// Where `plan` writes what it found, and what `apply` reads back.
pub const PLAN_FILE: &str = ".reach-plan";

/// Build CLI argument list for a tofu command.
/// `saved_plan`: apply the plan file the last `plan` wrote, instead of
/// planning again and asking. A saved plan is the approval — OpenTofu
/// applies exactly what was shown — so it needs neither `-auto-approve`
/// nor a var-file (the values are already in it).
pub fn build_command_args(request: &TofuCommandRequest, saved_plan: bool) -> Vec<String> {
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

    // Save plan output for later viewing, and let the exit code say whether
    // there is anything in it: 0 nothing, 2 changes, 1 error.
    if matches!(request.command, TofuCommand::Plan) {
        args.push(format!("-out={}", PLAN_FILE));
        args.push("-detailed-exitcode".to_string());
    }

    // The machine-readable UI: one JSON object per line instead of prose,
    // which the run view turns into a plan tree, apply progress and
    // diagnostics. Only the long-running commands speak it.
    if matches!(
        request.command,
        TofuCommand::Plan | TofuCommand::Apply | TofuCommand::Destroy
    ) {
        args.push("-json".to_string());
    }

    // Input=false to prevent interactive prompts
    match request.command {
        TofuCommand::Apply | TofuCommand::Plan | TofuCommand::Destroy => {
            args.push("-input=false".to_string());
        }
        _ => {}
    }

    // Var file — not with a saved plan, whose values are already fixed.
    if !saved_plan {
        if let Some(ref var_file) = request.var_file {
            args.push(format!("-var-file={}", var_file));
        }
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
        | TofuCommand::Test
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

    // The plan file is positional and must follow every flag, so it goes last.
    if saved_plan && matches!(request.command, TofuCommand::Apply) {
        args.push(PLAN_FILE.to_string());
    }

    args
}

/// Execute a tofu command locally, streaming output via Tauri events.
pub async fn run_local(
    working_dir: &str,
    args: &[String],
    run_id: &str,
    app_handle: &tauri::AppHandle,
    extra_env: &[(String, String)],
) -> Result<i32, String> {
    let event_name = format!("tofu-output-{}", run_id);

    // The binary is the project's decision (see tofu::binary). A pinned
    // version Reach has not downloaded yet is fetched first, in the open,
    // so the run that follows uses exactly what the project asked for.
    let system = |handle: &tauri::AppHandle, line: String| {
        let _ = handle.emit(
            &event_name,
            TofuCommandEvent {
                run_id: run_id.to_string(),
                stream: "system".to_string(),
                line,
                done: false,
                exit_code: None,
            },
        );
    };
    let resolved = match binary::resolve(Some(Path::new(working_dir))) {
        Ok(b) => b,
        Err(binary::ResolveError::PinnedNotInstalled(v)) => {
            system(app_handle, format!("This project pins OpenTofu {}. Installing it…", v));
            let h = app_handle.clone();
            let ev = event_name.clone();
            let rid = run_id.to_string();
            let report = move |m: &str| {
                let _ = h.emit(
                    &ev,
                    TofuCommandEvent {
                        run_id: rid.clone(),
                        stream: "system".to_string(),
                        line: m.to_string(),
                        done: false,
                        exit_code: None,
                    },
                );
            };
            if let Err(e) = binary::install(Some(v), report).await {
                let _ = app_handle.emit(
                    &event_name,
                    TofuCommandEvent {
                        run_id: run_id.to_string(),
                        stream: "stderr".to_string(),
                        line: e.clone(),
                        done: true,
                        exit_code: Some(1),
                    },
                );
                return Err(e);
            }
            binary::resolve(Some(Path::new(working_dir))).map_err(|_| "OpenTofu was installed but could not be resolved".to_string())?
        }
        Err(binary::ResolveError::NotFound) => {
            let _ = app_handle.emit(
                &event_name,
                TofuCommandEvent {
                    run_id: run_id.to_string(),
                    stream: "stderr".to_string(),
                    line: "OpenTofu is not installed. Install it from the OpenTofu tab.".to_string(),
                    done: true,
                    exit_code: Some(1),
                },
            );
            return Err("OpenTofu CLI not found".to_string());
        }
    };
    let binary = resolved.path;

    let mut child = silent_async_command(binary)
        .args(args)
        .current_dir(working_dir)
        .envs(AUTOMATION_ENV)
        .envs(extra_env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
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
            line: done_message(args, exit_code),
            done: true,
            exit_code: Some(exit_code),
        },
    );

    Ok(exit_code)
}

/// Told to a process that must never wait for a human: no prompts, and no
/// "run `tofu apply` next" hints written for someone at a shell.
const AUTOMATION_ENV: [(&str, &str); 2] = [("TF_IN_AUTOMATION", "1"), ("TF_INPUT", "0")];

/// The same two, as a prefix for a remote shell.
fn automation_prefix() -> String {
    AUTOMATION_ENV
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join(" ")
}

/// What the exit code means. `plan -detailed-exitcode` returns 2 for
/// "there are changes", which is the answer, not an error.
fn done_message(args: &[String], exit_code: i32) -> String {
    let detailed = args.iter().any(|a| a == "-detailed-exitcode");
    match (detailed, exit_code) {
        (_, 0) => "Command completed successfully.".to_string(),
        (true, 2) => "Plan complete: changes pending.".to_string(),
        _ => format!("Command exited with code {}.", exit_code),
    }
}

/// Execute a tofu command on a remote SSH connection, streaming output via Tauri events.
pub async fn run_remote(
    connection_id: &str,
    working_dir: &str,
    args: &[String],
    run_id: &str,
    app_handle: &tauri::AppHandle,
    ssh_manager: &mut SshManager,
    extra_env: &[(String, String)],
) -> Result<i32, String> {
    let event_name = format!("tofu-output-{}", run_id);

    // Build the full command string for remote execution
    // Extra variables ride on the command line here — a remote shell has no
    // other channel — so they are single-quoted and, for a moment, visible
    // in that host's process list.
    let mut prefix = automation_prefix();
    for (k, v) in extra_env {
        prefix.push(' ');
        prefix.push_str(k);
        prefix.push('=');
        prefix.push_str(&shell_escape(v));
    }
    let cmd = format!(
        "cd {} && {} tofu {}",
        shell_escape(working_dir),
        prefix,
        args.join(" ")
    );

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
            line: done_message(args, exit_code),
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
    use crate::tofu::types::TofuExecutionTarget;

    fn build_command_args_test(r: &TofuCommandRequest) -> Vec<String> {
        build_command_args(r, false)
    }

    fn req(command: TofuCommand) -> TofuCommandRequest {
        TofuCommandRequest {
            project_id: "p".into(),
            command,
            target: TofuExecutionTarget::Local,
            auto_approve: false,
            var_file: None,
            extra_args: vec![],
        }
    }

    #[test]
    fn plan_speaks_json_and_reports_changes_in_its_exit_code() {
        let args = build_command_args_test(&req(TofuCommand::Plan));
        assert!(args.contains(&"-json".to_string()));
        assert!(args.contains(&"-detailed-exitcode".to_string()));
        assert!(args.contains(&"-out=.reach-plan".to_string()));
        assert!(args.contains(&"-input=false".to_string()));
    }

    #[test]
    fn apply_and_destroy_speak_json() {
        for c in [TofuCommand::Apply, TofuCommand::Destroy] {
            let args = build_command_args_test(&req(c));
            assert!(args.contains(&"-json".to_string()), "{:?}", args);
        }
    }

    #[test]
    fn init_and_validate_speak_prose() {
        // Neither supports -json; passing it would be an error, not silence.
        for c in [TofuCommand::Init, TofuCommand::Validate, TofuCommand::Fmt] {
            let args = build_command_args_test(&req(c));
            assert!(!args.contains(&"-json".to_string()), "{:?}", args);
        }
    }

    #[test]
    fn exit_two_on_a_detailed_plan_is_news_not_failure() {
        let plan = build_command_args_test(&req(TofuCommand::Plan));
        assert_eq!(done_message(&plan, 2), "Plan complete: changes pending.");
        assert_eq!(done_message(&plan, 0), "Command completed successfully.");
        assert!(done_message(&plan, 1).contains("code 1"));
        let apply = build_command_args_test(&req(TofuCommand::Apply));
        assert!(done_message(&apply, 2).contains("code 2"), "apply has no special 2");
    }

    #[test]
    fn a_saved_plan_is_applied_as_is() {
        let mut r = req(TofuCommand::Apply);
        r.var_file = Some("prod.tfvars".into());
        let args = build_command_args(&r, true);
        assert_eq!(args.last().map(String::as_str), Some(PLAN_FILE), "plan file goes last");
        assert!(!args.iter().any(|a| a.starts_with("-var-file")), "values are in the plan");
        assert!(!args.contains(&"-auto-approve".to_string()), "the plan is the approval");
        assert!(args.contains(&"-json".to_string()));
    }

    #[test]
    fn without_a_saved_plan_apply_keeps_its_var_file() {
        let mut r = req(TofuCommand::Apply);
        r.var_file = Some("prod.tfvars".into());
        r.auto_approve = true;
        let args = build_command_args(&r, false);
        assert!(args.contains(&"-var-file=prod.tfvars".to_string()));
        assert!(args.contains(&"-auto-approve".to_string()));
        assert!(!args.contains(&PLAN_FILE.to_string()));
    }

    #[test]
    fn remote_prefix_carries_the_automation_env() {
        assert_eq!(automation_prefix(), "TF_IN_AUTOMATION=1 TF_INPUT=0");
    }
}
