use serde::{Deserialize, Serialize};

/// A saved Ansible project.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnsibleProject {
    pub id: String,
    pub name: String,
    pub path: String,
    pub description: String,
    pub created_at: String,
    pub last_opened_at: String,
    #[serde(default)]
    pub inventory_hosts: Vec<AnsibleInventoryHost>,
    #[serde(default)]
    pub inventory_groups: Vec<AnsibleInventoryGroup>,
    #[serde(default)]
    pub vault_password: Option<String>,
}

/// A host entry in an Ansible inventory.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnsibleInventoryHost {
    pub name: String,
    #[serde(default)]
    pub ansible_host: Option<String>,
    #[serde(default)]
    pub ansible_port: Option<u16>,
    #[serde(default)]
    pub ansible_user: Option<String>,
    #[serde(default)]
    pub groups: Vec<String>,
    #[serde(default)]
    pub variables: std::collections::HashMap<String, String>,
}

/// A group entry in an Ansible inventory.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnsibleInventoryGroup {
    pub name: String,
    #[serde(default)]
    pub variables: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub children: Vec<String>,
}

/// Streaming event payload emitted during Ansible command execution.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnsibleCommandEvent {
    pub run_id: String,
    pub stream: String,
    pub line: String,
    pub done: bool,
    pub exit_code: Option<i32>,
}

/// Which Ansible CLI command/tool to invoke.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AnsibleCommand {
    Playbook,
    AdHoc,
    GalaxyRoleInstall,
    GalaxyRoleList,
    GalaxyRoleRemove,
    GalaxyCollectionInstall,
    GalaxyCollectionList,
    VaultEncrypt,
    VaultDecrypt,
    VaultView,
    Inventory,
    SyntaxCheck,
}

/// Where to execute the Ansible command.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AnsibleExecutionTarget {
    Local,
    Ssh { connection_id: String },
}

/// Full request to execute an Ansible command.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnsibleCommandRequest {
    pub project_id: String,
    pub command: AnsibleCommand,
    pub target: AnsibleExecutionTarget,
    #[serde(default)]
    pub playbook: Option<String>,
    #[serde(default)]
    pub inventory_file: Option<String>,
    #[serde(default)]
    pub module_name: Option<String>,
    #[serde(default)]
    pub module_args: Option<String>,
    #[serde(default)]
    pub host_pattern: Option<String>,
    #[serde(default)]
    pub role_name: Option<String>,
    #[serde(default)]
    pub collection_name: Option<String>,
    #[serde(default)]
    pub vault_file: Option<String>,
    #[serde(default)]
    pub extra_args: Vec<String>,
}

/// An installed Ansible role.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnsibleRole {
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
}

/// An installed Ansible collection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnsibleCollection {
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
}
