import type {
	TerraformRun,
	TerraformExecMode,
	SavedTerraformWorkspace
} from '$lib/ipc/terraform';

export type ToolSetupStatus = 'checking' | 'installed' | 'not_installed' | 'installing' | 'install_failed';

const MAX_OUTPUT_LINES = 10000;

let toolStatus = $state<ToolSetupStatus>('checking');
let toolVersion = $state<string | null>(null);
let installLog = $state('');

let workingDir = $state('');
let execMode = $state<TerraformExecMode>('local');
let selectedConnectionId = $state<string | null>(null);
let activeRun = $state<TerraformRun | null>(null);
let outputBuffer = $state('');
let outputLineCount = $state(0);
let stateResources = $state<string[]>([]);
let selectedResource = $state<string | null>(null);
let resourceDetail = $state('');
let outputs = $state<Record<string, unknown>>({});
let savedWorkspaces = $state<SavedTerraformWorkspace[]>([]);

let isRunning = $derived(activeRun !== null && activeRun.status === 'Running');

export function getWorkingDir(): string {
	return workingDir;
}
export function setWorkingDir(dir: string): void {
	workingDir = dir;
}

export function getExecMode(): TerraformExecMode {
	return execMode;
}
export function setExecMode(mode: TerraformExecMode): void {
	execMode = mode;
}

export function getSelectedConnectionId(): string | null {
	return selectedConnectionId;
}
export function setSelectedConnectionId(id: string | null): void {
	selectedConnectionId = id;
}

export function getActiveRun(): TerraformRun | null {
	return activeRun;
}
export function setActiveRun(run: TerraformRun | null): void {
	activeRun = run;
}

export function getOutputBuffer(): string {
	return outputBuffer;
}
export function appendOutput(data: string): void {
	outputBuffer += data + '\n';
	outputLineCount++;
	if (outputLineCount > MAX_OUTPUT_LINES) {
		const lines = outputBuffer.split('\n');
		const trimmed = lines.slice(lines.length - MAX_OUTPUT_LINES);
		outputBuffer = trimmed.join('\n');
		outputLineCount = MAX_OUTPUT_LINES;
	}
}
export function clearOutput(): void {
	outputBuffer = '';
	outputLineCount = 0;
}

export function getIsRunning(): boolean {
	return isRunning;
}

export function getStateResources(): string[] {
	return stateResources;
}
export function setStateResources(resources: string[]): void {
	stateResources = resources;
}

export function getSelectedResource(): string | null {
	return selectedResource;
}
export function setSelectedResource(resource: string | null): void {
	selectedResource = resource;
}

export function getResourceDetail(): string {
	return resourceDetail;
}
export function setResourceDetail(detail: string): void {
	resourceDetail = detail;
}

export function getOutputs(): Record<string, unknown> {
	return outputs;
}
export function setOutputs(data: Record<string, unknown>): void {
	outputs = data;
}

export function getSavedWorkspaces(): SavedTerraformWorkspace[] {
	return savedWorkspaces;
}
export function setSavedWorkspaces(workspaces: SavedTerraformWorkspace[]): void {
	savedWorkspaces = workspaces;
}

export function getToolStatus(): ToolSetupStatus {
	return toolStatus;
}
export function setToolStatus(status: ToolSetupStatus): void {
	toolStatus = status;
}

export function getToolVersion(): string | null {
	return toolVersion;
}
export function setToolVersion(version: string | null): void {
	toolVersion = version;
}

export function getInstallLog(): string {
	return installLog;
}
export function appendInstallLog(line: string): void {
	installLog += line + '\n';
}
export function clearInstallLog(): void {
	installLog = '';
}
