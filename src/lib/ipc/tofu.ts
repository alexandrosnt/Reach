import { invoke } from '@tauri-apps/api/core';

export interface TofuProject {
	id: string;
	name: string;
	path: string;
	description: string;
	createdAt: string;
	lastOpenedAt: string;
	providers: TofuProviderConfig[];
	variables: TofuVariable[];
	environments: TofuEnvironment[];
	activeEnvironment: string | null;
	resources: TofuResourceConfig[];
	outputs: TofuOutput[];
	backend: TofuBackendConfig | null;
	dataSources: TofuDataSource[];
	locals: TofuLocal[];
	modules: TofuModuleConfig[];
}

export interface TofuProviderConfig {
	providerId: string;
	source: string;
	version: string;
	fields: Record<string, unknown>;
}

export interface TofuVariable {
	name: string;
	varType: TofuVarType;
	description: string;
	defaultValue: string | null;
	sensitive: boolean;
}

export type TofuVarType = 'string' | 'number' | 'bool' | 'list' | 'map';

export interface TofuEnvironment {
	name: string;
	values: Record<string, string>;
}

export interface ProviderCatalogEntry {
	id: string;
	name: string;
	source: string;
	description: string;
	category: string;
	icon: string;
	fields: ProviderFieldSchema[];
}

export interface ProviderFieldSchema {
	name: string;
	label: string;
	fieldType: ProviderFieldType;
	required: boolean;
	defaultValue: string | null;
	options: FieldOption[];
	helpText: string | null;
}

export type ProviderFieldType = 'string' | 'number' | 'bool' | 'select' | 'sensitive';

export interface FieldOption {
	label: string;
	value: string;
}

export interface TofuResourceConfig {
	id: string;
	resourceType: string;
	logicalName: string;
	providerId: string;
	fields: Record<string, unknown>;
}

export interface ResourceCatalogEntry {
	id: string;
	name: string;
	resourceType: string;
	providerId: string;
	category: string;
	description: string;
	fields: ProviderFieldSchema[];
}

export interface TofuOutput {
	name: string;
	value: string;
	description: string;
	sensitive: boolean;
}

export interface GraphNode {
	id: string;
	label: string;
	resourceType: string;
	providerId: string;
	x: number;
	y: number;
}

export interface GraphEdge {
	fromId: string;
	toId: string;
	label: string;
}

export interface DependencyGraph {
	nodes: GraphNode[];
	edges: GraphEdge[];
}

export interface ProjectTemplate {
	id: string;
	name: string;
	description: string;
	category: string;
	providers: TofuProviderConfig[];
	variables: TofuVariable[];
	resources: TofuResourceConfig[];
	outputs: TofuOutput[];
}

export interface TofuOutputValue {
	name: string;
	value: unknown;
	outputType: string;
	sensitive: boolean;
}

export interface TofuBackendConfig {
	backendType: string;
	fields: Record<string, unknown>;
}

export interface TofuDataSource {
	id: string;
	dataType: string;
	logicalName: string;
	providerId: string;
	fields: Record<string, unknown>;
}

export interface TofuLocal {
	name: string;
	expression: string;
}

export interface TofuModuleConfig {
	id: string;
	name: string;
	source: string;
	version: string;
	inputs: Record<string, unknown>;
}

export interface DataSourceCatalogEntry {
	id: string;
	name: string;
	dataType: string;
	providerId: string;
	category: string;
	description: string;
	fields: ProviderFieldSchema[];
}

export interface BackendCatalogEntry {
	id: string;
	name: string;
	description: string;
	fields: ProviderFieldSchema[];
}

export interface GeneratedFile {
	filename: string;
	content: string;
}

export interface HclGenerationResult {
	files: GeneratedFile[];
}

export interface TofuCommandEvent {
	runId: string;
	stream: 'stdout' | 'stderr' | 'system';
	line: string;
	done: boolean;
	exitCode: number | null;
}

export interface TofuWorkspaceInfo {
	workspaces: string[];
	current: string;
}

export interface TofuFmtResult {
	success: boolean;
	changedFiles: string[];
	error: string | null;
}

export interface TofuPlanSummary {
	resourceChanges: TofuResourceChange[];
	outputChanges: TofuOutputChange[];
	hasChanges: boolean;
}

export interface TofuResourceChange {
	address: string;
	resourceType: string;
	name: string;
	provider: string;
	action: TofuChangeAction;
	attributeChanges: TofuAttributeChange[];
}

export type TofuChangeAction = 'create' | 'update' | 'delete' | 'replace' | 'read' | 'noOp';

export interface TofuAttributeChange {
	attribute: string;
	oldValue: unknown | null;
	newValue: unknown | null;
	sensitive: boolean;
}

export interface TofuOutputChange {
	name: string;
	action: TofuChangeAction;
}

export interface ProviderSchema {
	source: string;
	providerAttributes: SchemaAttribute[];
	resourceSchemas: ResourceSchema[];
	dataSourceSchemas: ResourceSchema[];
}

export interface ResourceSchema {
	resourceType: string;
	attributes: SchemaAttribute[];
}

export interface SchemaAttribute {
	name: string;
	attrType: string;
	description: string;
	required: boolean;
	optional: boolean;
	computed: boolean;
	sensitive: boolean;
}

export type TofuCommand =
	| 'init'
	| 'plan'
	| 'apply'
	| 'destroy'
	| 'validate'
	| 'output'
	| 'show'
	| 'providersSchema'
	| 'stateList'
	| 'stateShow'
	| 'stateRm'
	| 'stateMv'
	| 'import'
	| 'fmt'
	| 'fmtCheck'
	| 'workspaceList'
	| 'workspaceNew'
	| 'workspaceSelect'
	| 'workspaceDelete';

export interface TofuExecutionTarget {
	type: 'local' | 'ssh';
	connectionId?: string;
}

export interface TofuCommandRequest {
	projectId: string;
	command: TofuCommand;
	target: TofuExecutionTarget;
	autoApprove: boolean;
	varFile: string | null;
	extraArgs: string[];
}

export async function tofuListProjects(): Promise<TofuProject[]> {
	return invoke<TofuProject[]>('tofu_list_projects');
}

export async function tofuCreateProject(
	name: string,
	path: string,
	description: string
): Promise<TofuProject> {
	return invoke<TofuProject>('tofu_create_project', { name, path, description });
}

export async function tofuDeleteProject(projectId: string): Promise<void> {
	return invoke<void>('tofu_delete_project', { projectId });
}

export async function tofuOpenProject(projectId: string): Promise<TofuProject> {
	return invoke<TofuProject>('tofu_open_project', { projectId });
}

export async function tofuRunCommand(request: TofuCommandRequest): Promise<string> {
	return invoke<string>('tofu_run_command', { request });
}

export async function tofuReadFile(projectId: string, filename: string): Promise<string> {
	return invoke<string>('tofu_read_file', { projectId, filename });
}

export async function tofuWriteFile(
	projectId: string,
	filename: string,
	content: string
): Promise<void> {
	return invoke<void>('tofu_write_file', { projectId, filename, content });
}

export async function tofuListFiles(projectId: string): Promise<string[]> {
	return invoke<string[]>('tofu_list_files', { projectId });
}

export async function tofuGetProviderCatalog(): Promise<ProviderCatalogEntry[]> {
	return invoke<ProviderCatalogEntry[]>('tofu_get_provider_catalog');
}

export async function tofuUpdateProviders(
	projectId: string,
	providers: TofuProviderConfig[]
): Promise<TofuProject> {
	return invoke<TofuProject>('tofu_update_providers', { projectId, providers });
}

export async function tofuUpdateVariables(
	projectId: string,
	variables: TofuVariable[]
): Promise<TofuProject> {
	return invoke<TofuProject>('tofu_update_variables', { projectId, variables });
}

export async function tofuUpdateEnvironments(
	projectId: string,
	environments: TofuEnvironment[],
	activeEnvironment: string | null
): Promise<TofuProject> {
	return invoke<TofuProject>('tofu_update_environments', {
		projectId,
		environments,
		activeEnvironment
	});
}

export async function tofuGenerateHcl(projectId: string): Promise<HclGenerationResult> {
	return invoke<HclGenerationResult>('tofu_generate_hcl', { projectId });
}

export async function tofuWriteGeneratedFiles(
	projectId: string,
	files: GeneratedFile[]
): Promise<void> {
	return invoke<void>('tofu_write_generated_files', { projectId, files });
}

export async function tofuGetResourceCatalog(): Promise<ResourceCatalogEntry[]> {
	return invoke<ResourceCatalogEntry[]>('tofu_get_resource_catalog');
}

export async function tofuUpdateResources(
	projectId: string,
	resources: TofuResourceConfig[]
): Promise<TofuProject> {
	return invoke<TofuProject>('tofu_update_resources', { projectId, resources });
}

export async function tofuStateListResources(
	projectId: string,
	target: TofuExecutionTarget
): Promise<string[]> {
	return invoke<string[]>('tofu_state_list_resources', { projectId, target });
}

export async function tofuUpdateOutputs(
	projectId: string,
	outputs: TofuOutput[]
): Promise<TofuProject> {
	return invoke<TofuProject>('tofu_update_outputs', { projectId, outputs });
}

export async function tofuGetOutputValues(
	projectId: string,
	target: TofuExecutionTarget
): Promise<TofuOutputValue[]> {
	return invoke<TofuOutputValue[]>('tofu_get_output_values', { projectId, target });
}

export async function tofuGetDependencyGraph(projectId: string): Promise<DependencyGraph> {
	return invoke<DependencyGraph>('tofu_get_dependency_graph', { projectId });
}

export async function tofuGetTemplates(): Promise<ProjectTemplate[]> {
	return invoke<ProjectTemplate[]>('tofu_get_templates');
}

export async function tofuApplyTemplate(
	projectId: string,
	templateId: string
): Promise<TofuProject> {
	return invoke<TofuProject>('tofu_apply_template', { projectId, templateId });
}

export async function tofuUpdateBackend(
	projectId: string,
	backend: TofuBackendConfig | null
): Promise<TofuProject> {
	return invoke<TofuProject>('tofu_update_backend', { projectId, backend });
}

export async function tofuUpdateDataSources(
	projectId: string,
	dataSources: TofuDataSource[]
): Promise<TofuProject> {
	return invoke<TofuProject>('tofu_update_data_sources', { projectId, dataSources });
}

export async function tofuGetDataSourceCatalog(): Promise<DataSourceCatalogEntry[]> {
	return invoke<DataSourceCatalogEntry[]>('tofu_get_data_source_catalog');
}

export async function tofuUpdateLocals(
	projectId: string,
	locals: TofuLocal[]
): Promise<TofuProject> {
	return invoke<TofuProject>('tofu_update_locals', { projectId, locals });
}

export async function tofuUpdateModules(
	projectId: string,
	modules: TofuModuleConfig[]
): Promise<TofuProject> {
	return invoke<TofuProject>('tofu_update_modules', { projectId, modules });
}

export async function tofuGetBackendCatalog(): Promise<BackendCatalogEntry[]> {
	return invoke<BackendCatalogEntry[]>('tofu_get_backend_catalog');
}

export async function tofuWorkspaceList(
	projectId: string,
	target: TofuExecutionTarget
): Promise<TofuWorkspaceInfo> {
	return invoke<TofuWorkspaceInfo>('tofu_workspace_list', { projectId, target });
}

export async function tofuWorkspaceNew(
	projectId: string,
	target: TofuExecutionTarget,
	name: string
): Promise<void> {
	return invoke<void>('tofu_workspace_new', { projectId, target, name });
}

export async function tofuWorkspaceSelect(
	projectId: string,
	target: TofuExecutionTarget,
	name: string
): Promise<void> {
	return invoke<void>('tofu_workspace_select', { projectId, target, name });
}

export async function tofuWorkspaceDelete(
	projectId: string,
	target: TofuExecutionTarget,
	name: string
): Promise<void> {
	return invoke<void>('tofu_workspace_delete', { projectId, target, name });
}

export async function tofuFmt(
	projectId: string,
	target: TofuExecutionTarget,
	checkOnly: boolean
): Promise<TofuFmtResult> {
	return invoke<TofuFmtResult>('tofu_fmt', { projectId, target, checkOnly });
}

export async function tofuShowPlanJson(
	projectId: string,
	target: TofuExecutionTarget
): Promise<TofuPlanSummary> {
	return invoke<TofuPlanSummary>('tofu_show_plan_json', { projectId, target });
}

export async function tofuFetchSchema(
	projectId: string,
	target: TofuExecutionTarget
): Promise<ProviderSchema[]> {
	return invoke<ProviderSchema[]>('tofu_fetch_schema', { projectId, target });
}

export async function tofuGetCachedSchema(
	projectId: string
): Promise<ProviderSchema[] | null> {
	return invoke<ProviderSchema[] | null>('tofu_get_cached_schema', { projectId });
}

export async function tofuGetSchemaResourceFields(
	projectId: string,
	resourceType: string
): Promise<ProviderFieldSchema[]> {
	return invoke<ProviderFieldSchema[]>('tofu_get_schema_resource_fields', {
		projectId,
		resourceType
	});
}
