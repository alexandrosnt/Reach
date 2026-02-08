use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single play in a playbook.
#[derive(Debug, Clone, Deserialize)]
pub struct Play {
    pub name: Option<String>,
    #[serde(default, rename = "become")]
    pub use_become: Option<bool>,
    #[serde(default)]
    pub vars: HashMap<String, serde_yaml::Value>,
    #[serde(default)]
    pub tasks: Vec<serde_yaml::Value>,
}

/// A parsed task ready for execution.
#[derive(Debug, Clone)]
pub struct Task {
    pub name: Option<String>,
    pub module: String,
    pub args: serde_yaml::Value,
    pub when: Option<String>,
    pub register: Option<String>,
    pub use_become: Option<bool>,
    pub ignore_errors: bool,
}

/// Status of a playbook run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PlaybookRunStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// Tracks the state of a playbook run.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybookRun {
    pub id: String,
    pub name: Option<String>,
    pub connection_id: String,
    pub status: PlaybookRunStatus,
}

/// Payload emitted when a playbook run completes.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybookCompleteEvent {
    pub run_id: String,
    pub status: PlaybookRunStatus,
    pub exit_code: Option<i32>,
    pub tasks_ok: u32,
    pub tasks_failed: u32,
}

/// Validation result returned by playbook_validate.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybookValidation {
    pub valid: bool,
    pub tasks: Vec<String>,
    pub error: Option<String>,
}

/// A saved playbook project configuration (persisted in vault).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedPlaybookProject {
    pub id: String,
    pub name: String,
    pub playbook_content: String,
    pub connection_id: Option<String>,
    #[serde(rename = "become")]
    pub use_become: bool,
    pub created_at: u64,
    pub updated_at: u64,
}
