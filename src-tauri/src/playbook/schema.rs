use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// A playbook is a sequence of steps to execute on a remote host via SSH.
///
/// The YAML format uses flat fields per step rather than nested enums.
/// Hosts are selected at runtime in the frontend, not in the YAML.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playbook {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub version: Option<u32>,
    #[serde(default)]
    pub variables: HashMap<String, String>,
    pub steps: Vec<PlaybookStep>,
}

/// A single step within a playbook.
///
/// Uses flat fields for simplicity in YAML authoring:
/// ```yaml
/// - name: Check disk space
///   command: df -h
///   timeout: 30
///   retries: 2
///   retry_delay: 5
///   on_failure: continue
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybookStep {
    pub name: String,
    pub command: String,
    /// Timeout in seconds for this step. None means no timeout.
    #[serde(default)]
    pub timeout: Option<u64>,
    /// Expected exit code. None means any exit code is accepted.
    #[serde(default)]
    pub expect_exit_code: Option<i32>,
    /// Regex pattern to match against command output.
    #[serde(default)]
    pub expect_output: Option<String>,
    /// Number of times to retry on failure.
    #[serde(default)]
    pub retries: Option<u32>,
    /// Delay in seconds between retries.
    #[serde(default)]
    pub retry_delay: Option<u64>,
    /// What to do on failure: "stop" (default) or "continue".
    #[serde(default)]
    pub on_failure: Option<String>,
}
