import { invoke } from '@tauri-apps/api/core';

export type TerraformExecMode = 'local' | 'remote';
export type TerraformAction = 'init' | 'plan' | 'apply' | 'destroy';

export interface TerraformRun {
	id: string;
	action: string;
	workingDir: string;
	execMode: TerraformExecMode;
	connectionId: string | null;
	status: 'Running' | 'Completed' | 'Failed' | 'Cancelled';
}

export interface TerraformOutputEvent {
	run_id: string;
	stream: 'stdout' | 'stderr';
	data: string;
}

export interface TerraformCompleteEvent {
	runId: string;
	status: 'Running' | 'Completed' | 'Failed' | 'Cancelled';
	exitCode: number | null;
}

export interface SavedTerraformWorkspace {
	id: string;
	name: string;
	workingDir: string;
	execMode: TerraformExecMode;
	connectionId: string | null;
	createdAt: number;
	updatedAt: number;
}

export async function terraformRun(
	action: TerraformAction,
	workingDir: string,
	execMode: TerraformExecMode,
	connectionId?: string
): Promise<TerraformRun> {
	return invoke<TerraformRun>('terraform_run', {
		action,
		workingDir,
		execMode,
		connectionId: connectionId ?? null
	});
}

export async function terraformCancel(runId: string): Promise<void> {
	return invoke('terraform_cancel', { runId });
}

export async function terraformGetRun(runId: string): Promise<TerraformRun> {
	return invoke<TerraformRun>('terraform_get_run', { runId });
}

export async function terraformStateList(
	workingDir: string,
	execMode: TerraformExecMode,
	connectionId?: string
): Promise<string[]> {
	return invoke<string[]>('terraform_state_list', {
		workingDir,
		execMode,
		connectionId: connectionId ?? null
	});
}

export async function terraformStateShow(
	workingDir: string,
	resource: string,
	execMode: TerraformExecMode,
	connectionId?: string
): Promise<string> {
	return invoke<string>('terraform_state_show', {
		workingDir,
		resource,
		execMode,
		connectionId: connectionId ?? null
	});
}

export async function terraformOutput(
	workingDir: string,
	execMode: TerraformExecMode,
	connectionId?: string
): Promise<Record<string, unknown>> {
	return invoke<Record<string, unknown>>('terraform_output', {
		workingDir,
		execMode,
		connectionId: connectionId ?? null
	});
}

export async function terraformCheck(
	execMode: TerraformExecMode,
	connectionId?: string
): Promise<string> {
	return invoke<string>('terraform_check', {
		execMode,
		connectionId: connectionId ?? null
	});
}

export async function terraformSaveWorkspace(
	name: string,
	workingDir: string,
	execMode: TerraformExecMode,
	connectionId?: string,
	id?: string
): Promise<SavedTerraformWorkspace> {
	return invoke<SavedTerraformWorkspace>('terraform_save_workspace', {
		id: id ?? null,
		name,
		workingDir,
		execMode,
		connectionId: connectionId ?? null
	});
}

export async function terraformListWorkspaces(): Promise<SavedTerraformWorkspace[]> {
	return invoke<SavedTerraformWorkspace[]>('terraform_list_workspaces');
}

export async function terraformDeleteWorkspace(id: string): Promise<void> {
	return invoke('terraform_delete_workspace', { id });
}
