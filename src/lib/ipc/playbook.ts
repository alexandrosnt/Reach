import { invoke } from '@tauri-apps/api/core';

export interface PlaybookRun {
	id: string;
	name: string | null;
	connectionId: string;
	status: 'Running' | 'Completed' | 'Failed' | 'Cancelled';
}

export interface PlaybookOutputEvent {
	run_id: string;
	stream: 'stdout' | 'stderr';
	data: string;
}

export interface PlaybookCompleteEvent {
	run_id: string;
	status: 'Running' | 'Completed' | 'Failed' | 'Cancelled';
	exit_code: number | null;
	tasks_ok: number;
	tasks_failed: number;
}

export interface PlaybookValidation {
	valid: boolean;
	tasks: string[];
	error: string | null;
}

export interface SavedPlaybookProject {
	id: string;
	name: string;
	playbookContent: string;
	connectionId: string | null;
	become: boolean;
	createdAt: number;
	updatedAt: number;
}

export async function playbookRun(
	playbookContent: string,
	connectionId: string,
	options?: {
		runId?: string;
		useBecome?: boolean;
		extraVars?: string;
	}
): Promise<PlaybookRun> {
	return invoke<PlaybookRun>('playbook_run', {
		runId: options?.runId ?? null,
		playbookContent,
		connectionId,
		useBecome: options?.useBecome ?? null,
		extraVars: options?.extraVars ?? null
	});
}

export async function playbookCancel(runId: string): Promise<void> {
	return invoke('playbook_cancel', { runId });
}

export async function playbookGetRun(runId: string): Promise<PlaybookRun> {
	return invoke<PlaybookRun>('playbook_get_run', { runId });
}

export async function playbookValidate(playbookContent: string): Promise<PlaybookValidation> {
	return invoke<PlaybookValidation>('playbook_validate', { playbookContent });
}

export async function playbookSaveProject(
	name: string,
	playbookContent: string,
	options?: {
		id?: string;
		connectionId?: string;
		become?: boolean;
	}
): Promise<SavedPlaybookProject> {
	return invoke<SavedPlaybookProject>('playbook_save_project', {
		id: options?.id ?? null,
		name,
		playbookContent,
		connectionId: options?.connectionId ?? null,
		useBecome: options?.become ?? null
	});
}

export async function playbookListProjects(): Promise<SavedPlaybookProject[]> {
	return invoke<SavedPlaybookProject[]>('playbook_list_projects');
}

export async function playbookDeleteProject(id: string): Promise<void> {
	return invoke('playbook_delete_project', { id });
}
