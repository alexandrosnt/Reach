use serde::{Deserialize, Serialize};

/// Status of a terraform operation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TerraformRunStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// Execution mode for terraform commands.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TerraformExecMode {
    Local,
    Remote,
}

/// Tracks the state of a terraform operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerraformRun {
    pub id: String,
    pub action: String,
    pub working_dir: String,
    pub exec_mode: TerraformExecMode,
    pub connection_id: Option<String>,
    pub status: TerraformRunStatus,
}

/// Payload emitted for each streaming output chunk.
#[derive(Debug, Clone, Serialize)]
pub struct TerraformOutputEvent {
    pub run_id: String,
    pub stream: String,
    pub data: String,
}

/// Payload emitted when a terraform operation completes.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerraformCompleteEvent {
    pub run_id: String,
    pub status: TerraformRunStatus,
    pub exit_code: Option<i32>,
}

/// A saved workspace configuration (persisted in vault).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedTerraformWorkspace {
    pub id: String,
    pub name: String,
    pub working_dir: String,
    pub exec_mode: TerraformExecMode,
    pub connection_id: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
}
