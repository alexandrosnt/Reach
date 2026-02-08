import type { PlaybookRun, SavedPlaybookProject } from '$lib/ipc/playbook';

const MAX_OUTPUT_LINES = 10000;

let playbookContent = $state('');
let extraVars = $state('');
let become = $state(false);
let selectedConnectionId = $state<string | null>(null);
let activeRun = $state<PlaybookRun | null>(null);
let outputBuffer = $state('');
let outputLineCount = $state(0);
let savedProjects = $state<SavedPlaybookProject[]>([]);

let isRunning = $derived(activeRun !== null && activeRun.status === 'Running');

export function getPlaybookContent(): string {
	return playbookContent;
}
export function setPlaybookContent(content: string): void {
	playbookContent = content;
}

export function getExtraVars(): string {
	return extraVars;
}
export function setExtraVars(vars: string): void {
	extraVars = vars;
}

export function getBecome(): boolean {
	return become;
}
export function setBecome(val: boolean): void {
	become = val;
}

export function getSelectedConnectionId(): string | null {
	return selectedConnectionId;
}
export function setSelectedConnectionId(id: string | null): void {
	selectedConnectionId = id;
}

export function getActiveRun(): PlaybookRun | null {
	return activeRun;
}
export function setActiveRun(run: PlaybookRun | null): void {
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

export function getSavedProjects(): SavedPlaybookProject[] {
	return savedProjects;
}
export function setSavedProjects(projects: SavedPlaybookProject[]): void {
	savedProjects = projects;
}
