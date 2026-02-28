use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// A saved OpenTofu project.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TofuProject {
    pub id: String,
    pub name: String,
    pub path: String,
    pub description: String,
    pub created_at: String,
    pub last_opened_at: String,
    #[serde(default)]
    pub providers: Vec<TofuProviderConfig>,
    #[serde(default)]
    pub variables: Vec<TofuVariable>,
    #[serde(default)]
    pub environments: Vec<TofuEnvironment>,
    #[serde(default)]
    pub active_environment: Option<String>,
    #[serde(default)]
    pub resources: Vec<TofuResourceConfig>,
    #[serde(default)]
    pub outputs: Vec<TofuOutput>,
    #[serde(default)]
    pub backend: Option<TofuBackendConfig>,
    #[serde(default)]
    pub data_sources: Vec<TofuDataSource>,
    #[serde(default)]
    pub locals: Vec<TofuLocal>,
    #[serde(default)]
    pub modules: Vec<TofuModuleConfig>,
}

/// A configured provider instance in a project.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TofuProviderConfig {
    pub provider_id: String,
    pub source: String,
    pub version: String,
    pub fields: HashMap<String, serde_json::Value>,
}

/// A user-defined Terraform variable.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TofuVariable {
    pub name: String,
    pub var_type: TofuVarType,
    pub description: String,
    #[serde(default)]
    pub default_value: Option<String>,
    #[serde(default)]
    pub sensitive: bool,
}

/// Supported Terraform variable types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TofuVarType {
    String,
    Number,
    Bool,
    List,
    Map,
}

impl TofuVarType {
    pub fn as_hcl(&self) -> &str {
        match self {
            TofuVarType::String => "string",
            TofuVarType::Number => "number",
            TofuVarType::Bool => "bool",
            TofuVarType::List => "list(string)",
            TofuVarType::Map => "map(string)",
        }
    }
}

/// An environment with per-variable values.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TofuEnvironment {
    pub name: String,
    pub values: HashMap<String, String>,
}

/// A provider from the static catalog.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCatalogEntry {
    pub id: String,
    pub name: String,
    pub source: String,
    pub description: String,
    pub category: String,
    pub icon: String,
    pub fields: Vec<ProviderFieldSchema>,
}

/// Schema for a provider configuration field.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderFieldSchema {
    pub name: String,
    pub label: String,
    pub field_type: ProviderFieldType,
    pub required: bool,
    #[serde(default)]
    pub default_value: Option<String>,
    #[serde(default)]
    pub options: Vec<FieldOption>,
    #[serde(default)]
    pub help_text: Option<String>,
}

/// Type of a provider config field (determines form widget).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProviderFieldType {
    String,
    Number,
    Bool,
    Select,
    Sensitive,
}

/// An option in a Select field.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldOption {
    pub label: String,
    pub value: String,
}

/// Result of HCL generation (a list of files).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HclGenerationResult {
    pub files: Vec<GeneratedFile>,
}

/// A single generated file with filename and content.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedFile {
    pub filename: String,
    pub content: String,
}

/// Streaming event payload emitted during tofu command execution.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TofuCommandEvent {
    pub run_id: String,
    pub stream: String,
    pub line: String,
    pub done: bool,
    pub exit_code: Option<i32>,
}

/// Which tofu CLI command to run.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TofuCommand {
    Init,
    Plan,
    Apply,
    Destroy,
    Validate,
    Output,
    Show,
    ProvidersSchema,
    StateList,
    StateShow,
    StateRm,
    StateMv,
    Import,
    Fmt,
    FmtCheck,
    WorkspaceList,
    WorkspaceNew,
    WorkspaceSelect,
    WorkspaceDelete,
}

impl TofuCommand {
    pub fn as_str(&self) -> &str {
        match self {
            TofuCommand::Init => "init",
            TofuCommand::Plan => "plan",
            TofuCommand::Apply => "apply",
            TofuCommand::Destroy => "destroy",
            TofuCommand::Validate => "validate",
            TofuCommand::Output => "output",
            TofuCommand::Show => "show",
            TofuCommand::ProvidersSchema => "providers",
            TofuCommand::StateList => "state",
            TofuCommand::StateShow => "state",
            TofuCommand::StateRm => "state",
            TofuCommand::StateMv => "state",
            TofuCommand::Import => "import",
            TofuCommand::Fmt => "fmt",
            TofuCommand::FmtCheck => "fmt",
            TofuCommand::WorkspaceList => "workspace",
            TofuCommand::WorkspaceNew => "workspace",
            TofuCommand::WorkspaceSelect => "workspace",
            TofuCommand::WorkspaceDelete => "workspace",
        }
    }
}

/// A configured resource instance in a project.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TofuResourceConfig {
    pub id: String,
    pub resource_type: String,
    pub logical_name: String,
    pub provider_id: String,
    pub fields: HashMap<String, serde_json::Value>,
}

/// A resource from the static catalog.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceCatalogEntry {
    pub id: String,
    pub name: String,
    pub resource_type: String,
    pub provider_id: String,
    pub category: String,
    pub description: String,
    pub fields: Vec<ProviderFieldSchema>,
}

/// A user-defined output block.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TofuOutput {
    pub name: String,
    pub value: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub sensitive: bool,
}

/// Backend configuration block (e.g. S3, azurerm, gcs).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TofuBackendConfig {
    pub backend_type: String,
    pub fields: HashMap<String, serde_json::Value>,
}

/// A data source reference (data "type" "name" {}).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TofuDataSource {
    pub id: String,
    pub data_type: String,
    pub logical_name: String,
    pub provider_id: String,
    pub fields: HashMap<String, serde_json::Value>,
}

/// A local value (locals { name = expression }).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TofuLocal {
    pub name: String,
    pub expression: String,
}

/// A module reference (module "name" { source = "..." }).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TofuModuleConfig {
    pub id: String,
    pub name: String,
    pub source: String,
    pub version: String,
    pub inputs: HashMap<String, serde_json::Value>,
}

/// A data source from the static catalog.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataSourceCatalogEntry {
    pub id: String,
    pub name: String,
    pub data_type: String,
    pub provider_id: String,
    pub category: String,
    pub description: String,
    pub fields: Vec<ProviderFieldSchema>,
}

/// A backend type from the static catalog.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendCatalogEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub fields: Vec<ProviderFieldSchema>,
}

/// A node in the dependency graph visualization.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub resource_type: String,
    pub provider_id: String,
    pub x: f64,
    pub y: f64,
}

/// An edge in the dependency graph visualization.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphEdge {
    pub from_id: String,
    pub to_id: String,
    pub label: String,
}

/// The full dependency graph returned to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyGraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

/// A pre-built project template from the catalog.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub providers: Vec<TofuProviderConfig>,
    pub variables: Vec<TofuVariable>,
    pub resources: Vec<TofuResourceConfig>,
    pub outputs: Vec<TofuOutput>,
}

/// A live output value from `tofu output -json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TofuOutputValue {
    pub name: String,
    pub value: serde_json::Value,
    pub output_type: String,
    pub sensitive: bool,
}

/// Where to execute the tofu command.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TofuExecutionTarget {
    Local,
    Ssh { connection_id: String },
}

/// Full request to execute a tofu command.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TofuCommandRequest {
    pub project_id: String,
    pub command: TofuCommand,
    pub target: TofuExecutionTarget,
    pub auto_approve: bool,
    pub var_file: Option<String>,
    pub extra_args: Vec<String>,
}

/// Workspace listing result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TofuWorkspaceInfo {
    pub workspaces: Vec<String>,
    pub current: String,
}

/// Result of `tofu fmt`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TofuFmtResult {
    pub success: bool,
    pub changed_files: Vec<String>,
    pub error: Option<String>,
}

/// Structured plan summary parsed from `tofu show -json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TofuPlanSummary {
    pub resource_changes: Vec<TofuResourceChange>,
    pub output_changes: Vec<TofuOutputChange>,
    pub has_changes: bool,
}

/// A single resource change in a plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TofuResourceChange {
    pub address: String,
    pub resource_type: String,
    pub name: String,
    pub provider: String,
    pub action: TofuChangeAction,
    pub attribute_changes: Vec<TofuAttributeChange>,
}

/// The type of change for a resource or output.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TofuChangeAction {
    Create,
    Update,
    Delete,
    Replace,
    Read,
    NoOp,
}

/// An attribute-level diff within a resource change.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TofuAttributeChange {
    pub attribute: String,
    pub old_value: Option<serde_json::Value>,
    pub new_value: Option<serde_json::Value>,
    pub sensitive: bool,
}

/// An output-level change in a plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TofuOutputChange {
    pub name: String,
    pub action: TofuChangeAction,
}

/// A parsed provider schema from `tofu providers schema -json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSchema {
    pub source: String,
    pub provider_attributes: Vec<SchemaAttribute>,
    pub resource_schemas: Vec<ResourceSchema>,
    pub data_source_schemas: Vec<ResourceSchema>,
}

/// A resource or data source schema from a provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceSchema {
    pub resource_type: String,
    pub attributes: Vec<SchemaAttribute>,
}

/// A single attribute in a schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaAttribute {
    pub name: String,
    pub attr_type: String,
    pub description: String,
    pub required: bool,
    pub optional: bool,
    pub computed: bool,
    pub sensitive: bool,
}

/// In-memory cache for provider schemas per project.
#[derive(Debug, Default)]
pub struct SchemaCache {
    pub cache: HashMap<String, Vec<ProviderSchema>>,
}
