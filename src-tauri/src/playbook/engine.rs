use crate::playbook::modules::{translate_module, wrap_become};
use crate::playbook::parser::{evaluate_when, extract_tasks, interpolate_vars, parse_playbook};
use crate::playbook::schema::{PlaybookCompleteEvent, PlaybookRunStatus};
use crate::ssh::client::{exec_on_connection_with_exit_code, SharedHandle, StreamingOutputEvent};
use std::collections::HashMap;
use tauri::Emitter;
use tokio_util::sync::CancellationToken;

/// Run a playbook over SSH.
/// Emits streaming output events and a final complete event.
pub async fn run_playbook(
    playbook_content: &str,
    run_id: &str,
    handle: &SharedHandle,
    use_become: bool,
    extra_vars: Option<&str>,
    cancel_token: &CancellationToken,
    app_handle: &tauri::AppHandle,
) -> Result<(u32, u32), String> {
    let output_event = format!("playbook-output-{}", run_id);
    let complete_event = format!("playbook-complete-{}", run_id);

    let plays = parse_playbook(playbook_content)?;

    // Parse extra vars (key=value format, space-separated)
    let mut global_vars: HashMap<String, String> = HashMap::new();
    if let Some(ev) = extra_vars {
        for pair in ev.split_whitespace() {
            if let Some(eq_pos) = pair.find('=') {
                let key = pair[..eq_pos].to_string();
                let val = pair[eq_pos + 1..].to_string();
                global_vars.insert(key, val);
            }
        }
    }

    let mut total_ok: u32 = 0;
    let mut total_failed: u32 = 0;

    for play in &plays {
        // Merge play vars with global vars (play vars take precedence)
        let mut vars = global_vars.clone();
        for (key, val) in &play.vars {
            if let Some(s) = val.as_str() {
                vars.insert(key.clone(), s.to_string());
            } else {
                vars.insert(key.clone(), serde_yaml::to_string(val).unwrap_or_default());
            }
        }

        let play_become = play.use_become.unwrap_or(false) || use_become;

        if let Some(name) = &play.name {
            emit_output(app_handle, &output_event, run_id, &format!("\nPLAY [{}] ***", name));
        }

        let tasks = extract_tasks(&play.tasks)?;

        for task in &tasks {
            // Check cancellation between tasks
            if cancel_token.is_cancelled() {
                emit_output(app_handle, &output_event, run_id, "\n*** CANCELLED ***");
                let _ = app_handle.emit(
                    &complete_event,
                    PlaybookCompleteEvent {
                        run_id: run_id.to_string(),
                        status: PlaybookRunStatus::Cancelled,
                        exit_code: None,
                        tasks_ok: total_ok,
                        tasks_failed: total_failed,
                    },
                );
                return Ok((total_ok, total_failed));
            }

            let task_name = task
                .name
                .as_deref()
                .unwrap_or(&task.module);
            emit_output(
                app_handle,
                &output_event,
                run_id,
                &format!("\nTASK [{}] ***", task_name),
            );

            // Evaluate `when` condition
            if let Some(ref condition) = task.when {
                let resolved = interpolate_vars(condition, &vars);
                if !evaluate_when(&resolved, &vars) {
                    emit_output(
                        app_handle,
                        &output_event,
                        run_id,
                        &format!("skipping: [{}]", task_name),
                    );
                    continue;
                }
            }

            // Translate module to shell commands
            let task_become = task.use_become.unwrap_or(play_become);
            let commands = match translate_module(&task.module, &task.args, &vars) {
                Ok(cmds) => wrap_become(cmds, task_become),
                Err(e) => {
                    emit_output(
                        app_handle,
                        &output_event,
                        run_id,
                        &format!("fatal: [{}]: MODULE ERROR: {}", task_name, e),
                    );
                    total_failed += 1;
                    if !task.ignore_errors {
                        let _ = app_handle.emit(
                            &complete_event,
                            PlaybookCompleteEvent {
                                run_id: run_id.to_string(),
                                status: PlaybookRunStatus::Failed,
                                exit_code: None,
                                tasks_ok: total_ok,
                                tasks_failed: total_failed,
                            },
                        );
                        return Ok((total_ok, total_failed));
                    }
                    continue;
                }
            };

            // Execute each command
            let mut task_ok = true;
            for cmd in &commands {
                let (stdout, stderr, exit_code) =
                    exec_on_connection_with_exit_code(handle, cmd).await.map_err(|e| {
                        format!("SSH execution error: {}", e)
                    })?;

                // Emit stdout/stderr
                if !stdout.is_empty() {
                    emit_output(app_handle, &output_event, run_id, &stdout);
                }
                if !stderr.is_empty() {
                    emit_output(app_handle, &output_event, run_id, &stderr);
                }

                if exit_code != 0 {
                    emit_output(
                        app_handle,
                        &output_event,
                        run_id,
                        &format!("fatal: [{}]: non-zero return code (rc={})", task_name, exit_code),
                    );
                    task_ok = false;

                    // Store rc for registered vars
                    if let Some(ref reg) = task.register {
                        vars.insert(format!("{}.rc", reg), exit_code.to_string());
                        vars.insert(format!("{}.stdout", reg), stdout.clone());
                        vars.insert(format!("{}.stderr", reg), stderr.clone());
                        vars.insert(format!("{}.failed", reg), "true".to_string());
                    }

                    break;
                } else if let Some(ref reg) = task.register {
                    vars.insert(format!("{}.rc", reg), "0".to_string());
                    vars.insert(format!("{}.stdout", reg), stdout.trim().to_string());
                    vars.insert(format!("{}.stderr", reg), stderr.trim().to_string());
                    vars.insert(format!("{}.failed", reg), "false".to_string());
                    // Also store stdout as the register var directly (common pattern)
                    vars.insert(reg.clone(), stdout.trim().to_string());
                }
            }

            if task_ok {
                emit_output(
                    app_handle,
                    &output_event,
                    run_id,
                    &format!("ok: [{}]", task_name),
                );
                total_ok += 1;
            } else {
                total_failed += 1;
                if !task.ignore_errors {
                    let _ = app_handle.emit(
                        &complete_event,
                        PlaybookCompleteEvent {
                            run_id: run_id.to_string(),
                            status: PlaybookRunStatus::Failed,
                            exit_code: Some(1),
                            tasks_ok: total_ok,
                            tasks_failed: total_failed,
                        },
                    );
                    return Ok((total_ok, total_failed));
                }
                emit_output(
                    app_handle,
                    &output_event,
                    run_id,
                    "...ignoring",
                );
            }
        }
    }

    // Summary
    emit_output(
        app_handle,
        &output_event,
        run_id,
        &format!(
            "\nPLAY RECAP ***\nok={}\tfailed={}",
            total_ok, total_failed
        ),
    );

    let final_status = if total_failed > 0 {
        PlaybookRunStatus::Failed
    } else {
        PlaybookRunStatus::Completed
    };

    let _ = app_handle.emit(
        &complete_event,
        PlaybookCompleteEvent {
            run_id: run_id.to_string(),
            status: final_status,
            exit_code: Some(if total_failed > 0 { 1 } else { 0 }),
            tasks_ok: total_ok,
            tasks_failed: total_failed,
        },
    );

    Ok((total_ok, total_failed))
}

fn emit_output(
    app_handle: &tauri::AppHandle,
    event: &str,
    run_id: &str,
    data: &str,
) {
    let _ = app_handle.emit(
        event,
        StreamingOutputEvent {
            run_id: run_id.to_string(),
            stream: "stdout".to_string(),
            data: data.to_string(),
        },
    );
}
