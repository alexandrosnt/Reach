use std::collections::HashMap;
use std::time::Duration;

use serde::Serialize;
use tauri::Emitter;
use thiserror::Error;

use super::schema::Playbook;
use crate::ssh::client::{exec_on_connection, SharedHandle};
use crate::state::{PlaybookRun, PlaybookStatus};

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("Execution failed at step {step}: {message}")]
    StepFailed { step: usize, message: String },
    #[error("Playbook not found: {0}")]
    NotFound(String),
    #[error("Already running: {0}")]
    AlreadyRunning(String),
    #[error("Connection error: {0}")]
    ConnectionError(String),
    #[error("Timeout at step {step}: exceeded {timeout_secs}s")]
    Timeout { step: usize, timeout_secs: u64 },
}

/// Payload emitted for each step event.
#[derive(Debug, Clone, Serialize)]
struct StepEvent {
    step_index: usize,
    step_name: String,
    status: String,
    output: String,
}

/// Payload emitted when the playbook completes.
#[derive(Debug, Clone, Serialize)]
struct CompleteEvent {
    run_id: String,
    playbook_name: String,
    status: String,
    total_steps: usize,
    completed_steps: usize,
    failed_steps: usize,
}

/// Replace `{{ varname }}` patterns in a command string with variable values.
fn interpolate_variables(command: &str, variables: &HashMap<String, String>) -> String {
    let mut result = command.to_string();
    for (key, value) in variables {
        // Match {{ varname }} with optional whitespace inside braces
        let patterns = [
            format!("{{{{ {} }}}}", key),   // {{ key }}
            format!("{{{{{}}}}}", key),      // {{key}}
        ];
        for pattern in &patterns {
            result = result.replace(pattern, value);
        }
    }
    result
}

/// Execute a playbook against an SSH connection, emitting Tauri events for progress.
///
/// Steps are executed sequentially. Each step supports:
/// - Variable interpolation in commands
/// - Timeout via `tokio::time::timeout`
/// - Retry with configurable delay
/// - Error strategies: "stop" (default) aborts on failure, "continue" moves on
pub async fn execute(
    playbook: &Playbook,
    handle: &SharedHandle,
    run_id: &str,
    app_handle: &tauri::AppHandle,
) -> Result<PlaybookRun, EngineError> {
    let total_steps = playbook.steps.len();
    tracing::info!(
        "Starting playbook '{}' (run {}) with {} steps",
        playbook.name,
        run_id,
        total_steps
    );

    let step_event_name = format!("playbook-step-{}", run_id);
    let complete_event_name = format!("playbook-complete-{}", run_id);

    let mut run = PlaybookRun {
        id: run_id.to_string(),
        playbook_name: playbook.name.clone(),
        status: PlaybookStatus::Running,
        current_step: 0,
        total_steps,
    };

    let mut completed_steps = 0usize;
    let mut failed_steps = 0usize;
    let mut aborted = false;

    for (i, step) in playbook.steps.iter().enumerate() {
        run.current_step = i;

        // Interpolate variables into the command
        let command = interpolate_variables(&step.command, &playbook.variables);

        tracing::info!("Run {}: step {}/{} '{}': {}", run_id, i + 1, total_steps, step.name, command);

        // Emit "running" event
        let _ = app_handle.emit(
            &step_event_name,
            StepEvent {
                step_index: i,
                step_name: step.name.clone(),
                status: "running".to_string(),
                output: String::new(),
            },
        );

        let max_attempts = step.retries.unwrap_or(0) + 1;
        let retry_delay = Duration::from_secs(step.retry_delay.unwrap_or(1));
        let on_failure = step
            .on_failure
            .as_deref()
            .unwrap_or("stop");

        let mut last_error: Option<String> = None;
        let mut step_succeeded = false;
        let mut step_output = String::new();

        for attempt in 1..=max_attempts {
            if attempt > 1 {
                tracing::info!(
                    "Run {}: retrying step '{}' (attempt {}/{})",
                    run_id,
                    step.name,
                    attempt,
                    max_attempts
                );
                tokio::time::sleep(retry_delay).await;
            }

            // Execute with optional timeout
            let exec_result = if let Some(timeout_secs) = step.timeout {
                match tokio::time::timeout(
                    Duration::from_secs(timeout_secs),
                    exec_on_connection(handle, &command),
                )
                .await
                {
                    Ok(inner) => inner,
                    Err(_) => {
                        last_error = Some(format!("Timeout after {}s", timeout_secs));
                        continue;
                    }
                }
            } else {
                exec_on_connection(handle, &command).await
            };

            match exec_result {
                Ok(output) => {
                    step_output = output;
                    step_succeeded = true;
                    last_error = None;
                    break;
                }
                Err(e) => {
                    last_error = Some(e.to_string());
                }
            }
        }

        if step_succeeded {
            completed_steps += 1;
            tracing::info!("Run {}: step '{}' completed", run_id, step.name);

            let _ = app_handle.emit(
                &step_event_name,
                StepEvent {
                    step_index: i,
                    step_name: step.name.clone(),
                    status: "completed".to_string(),
                    output: step_output,
                },
            );
        } else {
            failed_steps += 1;
            let err_msg = last_error.unwrap_or_else(|| "Unknown error".to_string());
            tracing::error!("Run {}: step '{}' failed: {}", run_id, step.name, err_msg);

            let _ = app_handle.emit(
                &step_event_name,
                StepEvent {
                    step_index: i,
                    step_name: step.name.clone(),
                    status: "failed".to_string(),
                    output: err_msg.clone(),
                },
            );

            match on_failure {
                "continue" => {
                    tracing::info!(
                        "Run {}: on_failure=continue, moving to next step",
                        run_id
                    );
                }
                _ => {
                    // "stop" or default
                    tracing::info!("Run {}: on_failure=stop, aborting playbook", run_id);
                    aborted = true;
                    break;
                }
            }
        }
    }

    // Determine final status
    if aborted {
        run.status = PlaybookStatus::Failed;
    } else if failed_steps > 0 {
        // Some steps failed but we continued through them
        run.status = PlaybookStatus::Completed;
    } else {
        run.status = PlaybookStatus::Completed;
    }
    run.current_step = total_steps;

    let status_str = match &run.status {
        PlaybookStatus::Completed => "completed",
        PlaybookStatus::Failed => "failed",
        PlaybookStatus::Stopped => "stopped",
        PlaybookStatus::Running => "running",
    };

    tracing::info!(
        "Run {}: playbook '{}' finished with status={}, completed={}, failed={}",
        run_id,
        playbook.name,
        status_str,
        completed_steps,
        failed_steps
    );

    let _ = app_handle.emit(
        &complete_event_name,
        CompleteEvent {
            run_id: run_id.to_string(),
            playbook_name: playbook.name.clone(),
            status: status_str.to_string(),
            total_steps,
            completed_steps,
            failed_steps,
        },
    );

    Ok(run)
}

/// Stop a running playbook (placeholder for cancellation support).
pub async fn stop(run_id: &str) -> Result<(), EngineError> {
    tracing::info!("Stopping playbook run {}", run_id);
    // Future: use a CancellationToken per run to signal abort
    Ok(())
}
