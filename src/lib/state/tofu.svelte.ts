import type {
	TofuProject,
	TofuProviderConfig,
	TofuVariable,
	TofuEnvironment,
	TofuResourceConfig,
	TofuOutput,
	TofuBackendConfig,
	TofuDataSource,
	TofuLocal,
	TofuModuleConfig,
	ProviderCatalogEntry,
	ResourceCatalogEntry,
	DataSourceCatalogEntry,
	BackendCatalogEntry,
	GeneratedFile,
	HclGenerationResult,
	DependencyGraph,
	ProjectTemplate,
	TofuOutputValue,
	TofuWorkspaceInfo,
	TofuFmtResult,
	TofuPlanSummary,
	ProviderSchema
} from '$lib/ipc/tofu';
import {
	tofuListProjects,
	tofuCreateProject,
	tofuDeleteProject,
	tofuOpenProject,
	tofuRunCommand,
	tofuListFiles,
	tofuGetProviderCatalog,
	tofuGetResourceCatalog,
	tofuGetDataSourceCatalog,
	tofuGetBackendCatalog,
	tofuUpdateProviders,
	tofuUpdateVariables,
	tofuUpdateEnvironments,
	tofuUpdateResources,
	tofuUpdateBackend,
	tofuUpdateDataSources,
	tofuUpdateLocals,
	tofuUpdateModules,
	tofuWorkspaceList,
	tofuWorkspaceNew,
	tofuWorkspaceSelect,
	tofuWorkspaceDelete,
	tofuFmt,
	tofuShowPlanJson,
	tofuFetchSchema,
	tofuGetCachedSchema,
	tofuGenerateHcl,
	tofuWriteGeneratedFiles,
	tofuStateListResources,
	tofuUpdateOutputs,
	tofuGetOutputValues,
	tofuGetDependencyGraph,
	tofuGetTemplates,
	tofuApplyTemplate,
	type TofuCommandRequest,
	type TofuCommandEvent,
	type TofuExecutionTarget
} from '$lib/ipc/tofu';
import { toolchainCheck, type ToolStatus } from '$lib/ipc/toolchain';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

// --- State ---
let projects = $state<TofuProject[]>([]);
let activeProjectId = $state<string | null>(null);
let toolStatus = $state<ToolStatus | null>(null);
let commandRunning = $state(false);
let commandOutput = $state<Array<{ stream: string; line: string }>>([]);
let currentRunId = $state<string | null>(null);
let projectFiles = $state<string[]>([]);
let providerCatalog = $state<ProviderCatalogEntry[]>([]);
let resourceCatalog = $state<ResourceCatalogEntry[]>([]);
let workspaceTab = $state<'actions' | 'providers' | 'variables' | 'resources' | 'environments' | 'state' | 'graph' | 'outputs' | 'backend' | 'data_sources' | 'locals' | 'modules' | 'workspaces'>('actions');
let stateResources = $state<string[]>([]);
let stateLoading = $state(false);
let dependencyGraph = $state<DependencyGraph | null>(null);
let graphLoading = $state(false);
let templateCatalog = $state<ProjectTemplate[]>([]);
let outputValues = $state<TofuOutputValue[]>([]);
let outputsLoading = $state(false);
let dataSourceCatalog = $state<DataSourceCatalogEntry[]>([]);
let backendCatalog = $state<BackendCatalogEntry[]>([]);
let workspaceInfo = $state<TofuWorkspaceInfo | null>(null);
let workspacesLoading = $state(false);
let planSummary = $state<TofuPlanSummary | null>(null);
let planSummaryLoading = $state(false);
let fmtRunning = $state(false);
let dynamicSchema = $state<ProviderSchema[] | null>(null);
let schemaLoading = $state(false);
let schemaError = $state<string | null>(null);

// --- Getters ---
export function getProjects(): TofuProject[] {
	return projects;
}

export function getActiveProjectId(): string | null {
	return activeProjectId;
}

export function getActiveProject(): TofuProject | undefined {
	return projects.find((p) => p.id === activeProjectId);
}

export function getToolStatus(): ToolStatus | null {
	return toolStatus;
}

export function isToolInstalled(): boolean {
	return toolStatus?.installed ?? false;
}

export function getToolVersion(): string | null {
	return toolStatus?.version ?? null;
}

export function isCommandRunning(): boolean {
	return commandRunning;
}

export function getCommandOutput(): Array<{ stream: string; line: string }> {
	return commandOutput;
}

export function getCurrentRunId(): string | null {
	return currentRunId;
}

export function getProjectFiles(): string[] {
	return projectFiles;
}

export function getProviderCatalog(): ProviderCatalogEntry[] {
	return providerCatalog;
}

export function getResourceCatalog(): ResourceCatalogEntry[] {
	return resourceCatalog;
}

export function getWorkspaceTab(): 'actions' | 'providers' | 'variables' | 'resources' | 'environments' | 'state' | 'graph' | 'outputs' | 'backend' | 'data_sources' | 'locals' | 'modules' | 'workspaces' {
	return workspaceTab;
}

export function getStateResources(): string[] {
	return stateResources;
}

export function isStateLoading(): boolean {
	return stateLoading;
}

export function getActiveProviders(): TofuProviderConfig[] {
	const project = getActiveProject();
	return project?.providers ?? [];
}

export function getActiveVariables(): TofuVariable[] {
	const project = getActiveProject();
	return project?.variables ?? [];
}

export function getActiveEnvironments(): TofuEnvironment[] {
	const project = getActiveProject();
	return project?.environments ?? [];
}

export function getActiveEnvironmentName(): string | null {
	const project = getActiveProject();
	return project?.activeEnvironment ?? null;
}

export function getActiveResources(): TofuResourceConfig[] {
	const project = getActiveProject();
	return project?.resources ?? [];
}

export function getActiveOutputs(): TofuOutput[] {
	const project = getActiveProject();
	return project?.outputs ?? [];
}

export function getActiveBackend(): TofuBackendConfig | null {
	const project = getActiveProject();
	return project?.backend ?? null;
}

export function getActiveDataSources(): TofuDataSource[] {
	const project = getActiveProject();
	return project?.dataSources ?? [];
}

export function getActiveLocals(): TofuLocal[] {
	const project = getActiveProject();
	return project?.locals ?? [];
}

export function getActiveModules(): TofuModuleConfig[] {
	const project = getActiveProject();
	return project?.modules ?? [];
}

export function getDataSourceCatalog(): DataSourceCatalogEntry[] {
	return dataSourceCatalog;
}

export function getBackendCatalog(): BackendCatalogEntry[] {
	return backendCatalog;
}

export function getWorkspaceInfo(): TofuWorkspaceInfo | null {
	return workspaceInfo;
}

export function isWorkspacesLoading(): boolean {
	return workspacesLoading;
}

export function getPlanSummary(): TofuPlanSummary | null {
	return planSummary;
}

export function isPlanSummaryLoading(): boolean {
	return planSummaryLoading;
}

export function isFmtRunning(): boolean {
	return fmtRunning;
}

export function getDynamicSchema(): ProviderSchema[] | null {
	return dynamicSchema;
}

export function isSchemaLoading(): boolean {
	return schemaLoading;
}

export function getSchemaError(): string | null {
	return schemaError;
}

export function getDependencyGraph(): DependencyGraph | null {
	return dependencyGraph;
}

export function isGraphLoading(): boolean {
	return graphLoading;
}

export function getTemplateCatalog(): ProjectTemplate[] {
	return templateCatalog;
}

export function getOutputValues(): TofuOutputValue[] {
	return outputValues;
}

export function isOutputsLoading(): boolean {
	return outputsLoading;
}

// --- Actions ---
export function setWorkspaceTab(
	tab: 'actions' | 'providers' | 'variables' | 'resources' | 'environments' | 'state' | 'graph' | 'outputs' | 'backend' | 'data_sources' | 'locals' | 'modules' | 'workspaces'
): void {
	workspaceTab = tab;
}

export async function checkTool(): Promise<void> {
	toolStatus = await toolchainCheck('tofu');
}

export async function loadProjects(): Promise<void> {
	try {
		projects = await tofuListProjects();
	} catch {
		projects = [];
	}
}

export async function loadCatalog(): Promise<void> {
	if (providerCatalog.length > 0) return;
	try {
		providerCatalog = await tofuGetProviderCatalog();
	} catch {
		providerCatalog = [];
	}
}

export async function loadResourceCatalog(): Promise<void> {
	if (resourceCatalog.length > 0) return;
	try {
		resourceCatalog = await tofuGetResourceCatalog();
	} catch {
		resourceCatalog = [];
	}
}

export async function createProject(
	name: string,
	path: string,
	description: string
): Promise<TofuProject> {
	const project = await tofuCreateProject(name, path, description);
	projects = [...projects, project];
	return project;
}

export async function deleteProject(projectId: string): Promise<void> {
	await tofuDeleteProject(projectId);
	projects = projects.filter((p) => p.id !== projectId);
	if (activeProjectId === projectId) {
		activeProjectId = null;
		projectFiles = [];
	}
}

export async function openProject(projectId: string): Promise<void> {
	const project = await tofuOpenProject(projectId);
	activeProjectId = project.id;
	projects = projects.map((p) => (p.id === project.id ? project : p));
	await refreshFiles();
}

export function closeProject(): void {
	activeProjectId = null;
	projectFiles = [];
	commandOutput = [];
	currentRunId = null;
	commandRunning = false;
	workspaceTab = 'actions';
	dependencyGraph = null;
	graphLoading = false;
	outputValues = [];
	outputsLoading = false;
	dataSourceCatalog = [];
	backendCatalog = [];
	workspaceInfo = null;
	workspacesLoading = false;
	planSummary = null;
	planSummaryLoading = false;
	fmtRunning = false;
	dynamicSchema = null;
	schemaLoading = false;
	schemaError = null;
}

export async function refreshFiles(): Promise<void> {
	if (!activeProjectId) return;
	try {
		projectFiles = await tofuListFiles(activeProjectId);
	} catch {
		projectFiles = [];
	}
}

function updateLocalProject(updated: TofuProject): void {
	projects = projects.map((p) => (p.id === updated.id ? updated : p));
}

export async function addProvider(provider: TofuProviderConfig): Promise<void> {
	const project = getActiveProject();
	if (!project) return;
	const newProviders = [...project.providers, provider];
	const updated = await tofuUpdateProviders(project.id, newProviders);
	updateLocalProject(updated);
}

export async function removeProvider(providerId: string): Promise<void> {
	const project = getActiveProject();
	if (!project) return;
	const newProviders = project.providers.filter((p) => p.providerId !== providerId);
	const updated = await tofuUpdateProviders(project.id, newProviders);
	updateLocalProject(updated);
}

export async function updateProviderConfig(provider: TofuProviderConfig): Promise<void> {
	const project = getActiveProject();
	if (!project) return;
	const newProviders = project.providers.map((p) =>
		p.providerId === provider.providerId ? provider : p
	);
	const updated = await tofuUpdateProviders(project.id, newProviders);
	updateLocalProject(updated);
}

export async function saveVariables(variables: TofuVariable[]): Promise<void> {
	const project = getActiveProject();
	if (!project) return;
	const updated = await tofuUpdateVariables(project.id, variables);
	updateLocalProject(updated);
}

export async function saveEnvironments(
	environments: TofuEnvironment[],
	activeEnv: string | null
): Promise<void> {
	const project = getActiveProject();
	if (!project) return;
	const updated = await tofuUpdateEnvironments(project.id, environments, activeEnv);
	updateLocalProject(updated);
}

export async function addResource(resource: TofuResourceConfig): Promise<void> {
	const project = getActiveProject();
	if (!project) return;
	const newResources = [...project.resources, resource];
	const updated = await tofuUpdateResources(project.id, newResources);
	updateLocalProject(updated);
}

export async function removeResource(resourceId: string): Promise<void> {
	const project = getActiveProject();
	if (!project) return;
	const newResources = project.resources.filter((r) => r.id !== resourceId);
	const updated = await tofuUpdateResources(project.id, newResources);
	updateLocalProject(updated);
}

export async function updateResourceConfig(resource: TofuResourceConfig): Promise<void> {
	const project = getActiveProject();
	if (!project) return;
	const newResources = project.resources.map((r) => (r.id === resource.id ? resource : r));
	const updated = await tofuUpdateResources(project.id, newResources);
	updateLocalProject(updated);
}

export async function generateHcl(): Promise<HclGenerationResult> {
	const project = getActiveProject();
	if (!project) throw new Error('No active project');
	return tofuGenerateHcl(project.id);
}

export async function writeGeneratedFiles(files: GeneratedFile[]): Promise<void> {
	const project = getActiveProject();
	if (!project) return;
	await tofuWriteGeneratedFiles(project.id, files);
	await refreshFiles();
}

export async function runCommand(request: TofuCommandRequest): Promise<string> {
	commandRunning = true;
	commandOutput = [];

	const runId = await tofuRunCommand(request);
	currentRunId = runId;

	const eventName = `tofu-output-${runId}`;
	let unlisten: UnlistenFn | null = null;

	unlisten = await listen<TofuCommandEvent>(eventName, (event) => {
		const data = event.payload;
		commandOutput = [...commandOutput, { stream: data.stream, line: data.line }];

		if (data.done) {
			commandRunning = false;
			if (unlisten) {
				unlisten();
				unlisten = null;
			}
			refreshFiles();
		}
	});

	return runId;
}

export async function loadStateResources(target: TofuExecutionTarget): Promise<void> {
	const project = getActiveProject();
	if (!project) return;
	stateLoading = true;
	try {
		stateResources = await tofuStateListResources(project.id, target);
	} catch {
		stateResources = [];
	} finally {
		stateLoading = false;
	}
}

export async function stateShowResource(target: TofuExecutionTarget, address: string): Promise<void> {
	const project = getActiveProject();
	if (!project) return;
	setWorkspaceTab('actions');
	await runCommand({
		projectId: project.id,
		command: 'stateShow',
		target,
		autoApprove: false,
		varFile: null,
		extraArgs: [address]
	});
}

export async function stateRemoveResource(target: TofuExecutionTarget, address: string): Promise<void> {
	const project = getActiveProject();
	if (!project) return;
	setWorkspaceTab('actions');
	await runCommand({
		projectId: project.id,
		command: 'stateRm',
		target,
		autoApprove: false,
		varFile: null,
		extraArgs: [address]
	});
	await loadStateResources(target);
}

export async function stateMoveResource(target: TofuExecutionTarget, source: string, dest: string): Promise<void> {
	const project = getActiveProject();
	if (!project) return;
	setWorkspaceTab('actions');
	await runCommand({
		projectId: project.id,
		command: 'stateMv',
		target,
		autoApprove: false,
		varFile: null,
		extraArgs: [source, dest]
	});
	await loadStateResources(target);
}

export async function stateImportResource(target: TofuExecutionTarget, address: string, id: string): Promise<void> {
	const project = getActiveProject();
	if (!project) return;
	setWorkspaceTab('actions');
	await runCommand({
		projectId: project.id,
		command: 'import',
		target,
		autoApprove: false,
		varFile: null,
		extraArgs: [address, id]
	});
	await loadStateResources(target);
}

export async function saveOutputs(outputs: TofuOutput[]): Promise<void> {
	const project = getActiveProject();
	if (!project) return;
	const updated = await tofuUpdateOutputs(project.id, outputs);
	updateLocalProject(updated);
}

export async function loadDependencyGraph(): Promise<void> {
	const project = getActiveProject();
	if (!project) return;
	graphLoading = true;
	try {
		dependencyGraph = await tofuGetDependencyGraph(project.id);
	} catch {
		dependencyGraph = null;
	} finally {
		graphLoading = false;
	}
}

export async function loadTemplates(): Promise<void> {
	if (templateCatalog.length > 0) return;
	try {
		templateCatalog = await tofuGetTemplates();
	} catch {
		templateCatalog = [];
	}
}

export async function applyTemplate(projectId: string, templateId: string): Promise<TofuProject> {
	const updated = await tofuApplyTemplate(projectId, templateId);
	updateLocalProject(updated);
	return updated;
}

export async function loadOutputValues(target: TofuExecutionTarget): Promise<void> {
	const project = getActiveProject();
	if (!project) return;
	outputsLoading = true;
	try {
		outputValues = await tofuGetOutputValues(project.id, target);
	} catch {
		outputValues = [];
	} finally {
		outputsLoading = false;
	}
}

export async function loadWorkspaces(target: TofuExecutionTarget): Promise<void> {
	const project = getActiveProject();
	if (!project) return;
	workspacesLoading = true;
	try {
		workspaceInfo = await tofuWorkspaceList(project.id, target);
	} catch {
		workspaceInfo = null;
	} finally {
		workspacesLoading = false;
	}
}

export async function createWorkspace(target: TofuExecutionTarget, name: string): Promise<void> {
	const project = getActiveProject();
	if (!project) return;
	await tofuWorkspaceNew(project.id, target, name);
	await loadWorkspaces(target);
}

export async function selectWorkspace(target: TofuExecutionTarget, name: string): Promise<void> {
	const project = getActiveProject();
	if (!project) return;
	await tofuWorkspaceSelect(project.id, target, name);
	await loadWorkspaces(target);
}

export async function deleteWorkspace(target: TofuExecutionTarget, name: string): Promise<void> {
	const project = getActiveProject();
	if (!project) return;
	await tofuWorkspaceDelete(project.id, target, name);
	await loadWorkspaces(target);
}

export async function formatFiles(target: TofuExecutionTarget, checkOnly: boolean = false): Promise<TofuFmtResult | null> {
	const project = getActiveProject();
	if (!project) return null;
	fmtRunning = true;
	try {
		const result = await tofuFmt(project.id, target, checkOnly);
		if (!checkOnly) {
			await refreshFiles();
		}
		return result;
	} finally {
		fmtRunning = false;
	}
}

export async function loadPlanSummary(target: TofuExecutionTarget): Promise<void> {
	const project = getActiveProject();
	if (!project) return;
	planSummaryLoading = true;
	try {
		planSummary = await tofuShowPlanJson(project.id, target);
	} catch {
		planSummary = null;
	} finally {
		planSummaryLoading = false;
	}
}

export async function loadDataSourceCatalog(): Promise<void> {
	if (dataSourceCatalog.length > 0) return;
	try {
		dataSourceCatalog = await tofuGetDataSourceCatalog();
	} catch {
		dataSourceCatalog = [];
	}
}

export async function loadBackendCatalog(): Promise<void> {
	if (backendCatalog.length > 0) return;
	try {
		backendCatalog = await tofuGetBackendCatalog();
	} catch {
		backendCatalog = [];
	}
}

export async function saveBackend(backend: TofuBackendConfig | null): Promise<void> {
	const project = getActiveProject();
	if (!project) return;
	const updated = await tofuUpdateBackend(project.id, backend);
	updateLocalProject(updated);
}

export async function saveDataSources(dataSources: TofuDataSource[]): Promise<void> {
	const project = getActiveProject();
	if (!project) return;
	const updated = await tofuUpdateDataSources(project.id, dataSources);
	updateLocalProject(updated);
}

export async function saveLocals(locals: TofuLocal[]): Promise<void> {
	const project = getActiveProject();
	if (!project) return;
	const updated = await tofuUpdateLocals(project.id, locals);
	updateLocalProject(updated);
}

export async function saveModules(modules: TofuModuleConfig[]): Promise<void> {
	const project = getActiveProject();
	if (!project) return;
	const updated = await tofuUpdateModules(project.id, modules);
	updateLocalProject(updated);
}

export async function fetchSchema(target: TofuExecutionTarget): Promise<void> {
	const project = getActiveProject();
	if (!project) return;
	schemaLoading = true;
	schemaError = null;
	try {
		dynamicSchema = await tofuFetchSchema(project.id, target);
	} catch (e) {
		schemaError = e instanceof Error ? e.message : String(e);
		dynamicSchema = null;
	} finally {
		schemaLoading = false;
	}
}

export async function loadCachedSchema(): Promise<void> {
	const project = getActiveProject();
	if (!project) return;
	try {
		dynamicSchema = await tofuGetCachedSchema(project.id);
	} catch {
		dynamicSchema = null;
	}
}

export function clearOutput(): void {
	commandOutput = [];
}
