import type {
	AnsibleProject,
	AnsibleRole,
	AnsibleCollection,
	AnsibleInventoryHost,
	AnsibleInventoryGroup
} from '$lib/ipc/ansible';
import { ansibleEngines, ansibleRemoteEngine, type AnsibleEngineInfo } from '$lib/ipc/ansible';
import { splitLines } from '$lib/tofu/ui-json';
import { parseAnsibleLine, applyAnsibleEvent, emptyAnsibleRun, type AnsibleRunView } from '$lib/ansible/recap';
import {
	ansibleListProjects,
	ansibleCreateProject,
	ansibleDeleteProject,
	ansibleOpenProject,
	ansibleRunCommand,
	ansibleListFiles,
	ansibleUpdateInventory,
	ansibleGenerateInventory,
	ansibleWriteInventory,
	ansibleListRoles,
	ansibleListCollections,
	type AnsibleCommandRequest,
	type AnsibleCommandEvent,
	type AnsibleExecutionTarget
} from '$lib/ipc/ansible';
import { toolchainCheck, type ToolStatus } from '$lib/ipc/toolchain';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

// --- State ---
let projects = $state<AnsibleProject[]>([]);
let activeProjectId = $state<string | null>(null);
let toolStatus = $state<ToolStatus | null>(null);
let toolChecking = $state(false);
let commandRunning = $state(false);
let commandOutput = $state<Array<{ stream: string; line: string }>>([]);
let currentRunId = $state<string | null>(null);
// The run, folded from its output lines as they arrive. See ansible/recap.
let runView = $state<AnsibleRunView>(emptyAnsibleRun());
let runExitCode = $state<number | null>(null);
let projectFiles = $state<string[]>([]);
let roles = $state<AnsibleRole[]>([]);
let collections = $state<AnsibleCollection[]>([]);
let rolesLoading = $state(false);
let collectionsLoading = $state(false);
let workspaceTab = $state<
	'playbooks' | 'inventory' | 'roles' | 'collections' | 'adhoc' | 'vault'
>('playbooks');

// --- Getters ---
export function getProjects(): AnsibleProject[] {
	return projects;
}

export function getActiveProjectId(): string | null {
	return activeProjectId;
}

export function getActiveProject(): AnsibleProject | undefined {
	return projects.find((p) => p.id === activeProjectId);
}

export function getToolStatus(): ToolStatus | null {
	return toolStatus;
}

export function isToolInstalled(): boolean {
	return toolStatus?.installed ?? false;
}

export function isToolChecking(): boolean {
	return toolChecking;
}

export function isLocalUnsupported(): boolean {
	return toolStatus?.localUnsupported ?? false;
}

export function isWsl(): boolean {
	return toolStatus?.wsl ?? false;
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

export function getRunView(): AnsibleRunView {
	return runView;
}

export function getRunExitCode(): number | null {
	return runExitCode;
}

export function getCurrentRunId(): string | null {
	return currentRunId;
}

export function getProjectFiles(): string[] {
	return projectFiles;
}

export function getRoles(): AnsibleRole[] {
	return roles;
}

export function getCollections(): AnsibleCollection[] {
	return collections;
}

export function isRolesLoading(): boolean {
	return rolesLoading;
}

export function isCollectionsLoading(): boolean {
	return collectionsLoading;
}

export function getWorkspaceTab():
	| 'playbooks'
	| 'inventory'
	| 'roles'
	| 'collections'
	| 'adhoc'
	| 'vault' {
	return workspaceTab;
}

// --- Actions ---
export function setWorkspaceTab(
	tab: 'playbooks' | 'inventory' | 'roles' | 'collections' | 'adhoc' | 'vault'
): void {
	workspaceTab = tab;
}

export async function checkTool(): Promise<void> {
	toolChecking = true;
	try {
		toolStatus = await toolchainCheck('ansible');
	} finally {
		toolChecking = false;
	}
}

export async function loadProjects(): Promise<void> {
	try {
		projects = await ansibleListProjects();
	} catch {
		projects = [];
	}
}

export async function createProject(
	name: string,
	path: string,
	description: string
): Promise<AnsibleProject> {
	const project = await ansibleCreateProject(name, path, description);
	projects = [...projects, project];
	return project;
}

export async function deleteProject(projectId: string): Promise<void> {
	await ansibleDeleteProject(projectId);
	projects = projects.filter((p) => p.id !== projectId);
	if (activeProjectId === projectId) {
		activeProjectId = null;
		projectFiles = [];
	}
}

export async function openProject(projectId: string): Promise<void> {
	const project = await ansibleOpenProject(projectId);
	activeProjectId = project.id;
	projects = projects.map((p) => (p.id === project.id ? project : p));
	await refreshFiles();
}

export function closeProject(): void {
	activeProjectId = null;
	projectFiles = [];
	commandOutput = [];
	runView = emptyAnsibleRun();
	runExitCode = null;
	currentRunId = null;
	commandRunning = false;
	workspaceTab = 'playbooks';
	roles = [];
	collections = [];
	rolesLoading = false;
	collectionsLoading = false;
}

export async function refreshFiles(): Promise<void> {
	if (!activeProjectId) return;
	try {
		projectFiles = await ansibleListFiles(activeProjectId);
	} catch {
		projectFiles = [];
	}
}

function updateLocalProject(updated: AnsibleProject): void {
	projects = projects.map((p) => (p.id === updated.id ? updated : p));
}

export async function saveInventory(
	hosts: AnsibleInventoryHost[],
	groups: AnsibleInventoryGroup[]
): Promise<void> {
	const project = getActiveProject();
	if (!project) return;
	const updated = await ansibleUpdateInventory(project.id, hosts, groups);
	updateLocalProject(updated);
}

export async function generateInventory(): Promise<string> {
	const project = getActiveProject();
	if (!project) throw new Error('No active project');
	return ansibleGenerateInventory(project.id);
}

export async function writeInventory(content: string, filename?: string): Promise<void> {
	const project = getActiveProject();
	if (!project) return;
	await ansibleWriteInventory(project.id, content, filename);
	await refreshFiles();
}

export async function runCommand(request: AnsibleCommandRequest): Promise<string> {
	commandRunning = true;
	commandOutput = [];
	runView = emptyAnsibleRun();
	runExitCode = null;

	const runId = await ansibleRunCommand(request);
	currentRunId = runId;

	const eventName = `ansible-output-${runId}`;
	let unlisten: UnlistenFn | null = null;

	// Local runs arrive a line at a time; remote runs arrive as whatever the
	// SSH channel had, which can end mid-line. One splitter serves both.
	let carry = '';
	unlisten = await listen<AnsibleCommandEvent & { data?: string }>(eventName, (event) => {
		const data = event.payload;
		const take = (stream: string, line: string) => {
			commandOutput = [...commandOutput, { stream, line }];
			if (stream !== 'stdout') return;
			const e = parseAnsibleLine(line);
			if (e) runView = applyAnsibleEvent(runView, e);
		};
		if (typeof data.line === 'string') {
			take(data.stream, data.line);
		} else if (typeof data.data === 'string') {
			const split = splitLines(carry, data.data);
			carry = split.carry;
			for (const line of split.lines) take(data.stream, line);
		}

		if (data.done) {
			if (carry) {
				take('stdout', carry);
				carry = '';
			}
			runExitCode = data.exitCode ?? null;
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

export async function loadRoles(): Promise<void> {
	const project = getActiveProject();
	if (!project) return;
	rolesLoading = true;
	try {
		roles = await ansibleListRoles(project.id);
	} catch {
		roles = [];
	} finally {
		rolesLoading = false;
	}
}

export async function loadCollections(): Promise<void> {
	const project = getActiveProject();
	if (!project) return;
	collectionsLoading = true;
	try {
		collections = await ansibleListCollections(project.id);
	} catch {
		collections = [];
	} finally {
		collectionsLoading = false;
	}
}

export function clearOutput(): void {
	commandOutput = [];
	runView = emptyAnsibleRun();
	runExitCode = null;
}

// ==================== ENGINES ====================

let engines = $state<AnsibleEngineInfo[]>([]);
let enginesLoaded = $state(false);
let remoteEngines = $state<Record<string, AnsibleEngineInfo>>({});
// The setup screen stepped past by choice: projects can be written now and
// run later on a remote host or in a container. Session-only, on purpose.
let setupSkipped = $state(false);

export function getEngines(): AnsibleEngineInfo[] {
	return engines;
}

export function getRemoteEngine(connectionId: string): AnsibleEngineInfo | null {
	return remoteEngines[connectionId] ?? null;
}

export function areEnginesLoaded(): boolean {
	return enginesLoaded;
}

/// True when something on this machine can run a playbook right now.
export function hasLocalEngine(): boolean {
	return engines.some((e) => e.available);
}

export function isSetupSkipped(): boolean {
	return setupSkipped;
}

export function skipToolchainSetup(): void {
	setupSkipped = true;
}

export async function loadEngines(): Promise<void> {
	try {
		engines = await ansibleEngines();
	} catch {
		engines = [];
	} finally {
		enginesLoaded = true;
	}
}

export async function checkRemoteEngine(connectionId: string): Promise<void> {
	try {
		const info = await ansibleRemoteEngine(connectionId);
		remoteEngines = { ...remoteEngines, [connectionId]: info };
	} catch (e) {
		remoteEngines = {
			...remoteEngines,
			[connectionId]: { kind: 'remote', available: false, version: null, reason: String(e), unofficial: false, detail: null }
		};
	}
}
