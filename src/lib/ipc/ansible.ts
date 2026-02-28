import { invoke } from '@tauri-apps/api/core';

export interface AnsibleProject {
	id: string;
	name: string;
	path: string;
	description: string;
	createdAt: string;
	lastOpenedAt: string;
	inventoryHosts: AnsibleInventoryHost[];
	inventoryGroups: AnsibleInventoryGroup[];
	vaultPassword: string | null;
}

export interface AnsibleInventoryHost {
	name: string;
	ansibleHost: string | null;
	ansiblePort: number | null;
	ansibleUser: string | null;
	groups: string[];
	variables: Record<string, string>;
}

export interface AnsibleInventoryGroup {
	name: string;
	variables: Record<string, string>;
	children: string[];
}

export interface AnsibleCommandEvent {
	runId: string;
	stream: 'stdout' | 'stderr' | 'system';
	line: string;
	done: boolean;
	exitCode: number | null;
}

export type AnsibleCommand =
	| 'playbook'
	| 'adHoc'
	| 'galaxyRoleInstall'
	| 'galaxyRoleList'
	| 'galaxyRoleRemove'
	| 'galaxyCollectionInstall'
	| 'galaxyCollectionList'
	| 'vaultEncrypt'
	| 'vaultDecrypt'
	| 'vaultView'
	| 'inventory'
	| 'syntaxCheck';

export interface AnsibleExecutionTarget {
	type: 'local' | 'ssh';
	connectionId?: string;
}

export interface AnsibleCommandRequest {
	projectId: string;
	command: AnsibleCommand;
	target: AnsibleExecutionTarget;
	playbook?: string | null;
	inventoryFile?: string | null;
	moduleName?: string | null;
	moduleArgs?: string | null;
	hostPattern?: string | null;
	roleName?: string | null;
	collectionName?: string | null;
	vaultFile?: string | null;
	extraArgs?: string[];
}

export interface AnsibleRole {
	name: string;
	version: string | null;
}

export interface AnsibleCollection {
	name: string;
	version: string | null;
}

export async function ansibleListProjects(): Promise<AnsibleProject[]> {
	return invoke<AnsibleProject[]>('ansible_list_projects');
}

export async function ansibleCreateProject(
	name: string,
	path: string,
	description: string
): Promise<AnsibleProject> {
	return invoke<AnsibleProject>('ansible_create_project', { name, path, description });
}

export async function ansibleDeleteProject(projectId: string): Promise<void> {
	return invoke<void>('ansible_delete_project', { projectId });
}

export async function ansibleOpenProject(projectId: string): Promise<AnsibleProject> {
	return invoke<AnsibleProject>('ansible_open_project', { projectId });
}

export async function ansibleUpdateInventory(
	projectId: string,
	hosts: AnsibleInventoryHost[],
	groups: AnsibleInventoryGroup[]
): Promise<AnsibleProject> {
	return invoke<AnsibleProject>('ansible_update_inventory', { projectId, hosts, groups });
}

export async function ansibleListFiles(projectId: string): Promise<string[]> {
	return invoke<string[]>('ansible_list_files', { projectId });
}

export async function ansibleReadFile(projectId: string, filename: string): Promise<string> {
	return invoke<string>('ansible_read_file', { projectId, filename });
}

export async function ansibleWriteFile(
	projectId: string,
	filename: string,
	content: string
): Promise<void> {
	return invoke<void>('ansible_write_file', { projectId, filename, content });
}

export async function ansibleRunCommand(request: AnsibleCommandRequest): Promise<string> {
	return invoke<string>('ansible_run_command', { request });
}

export async function ansibleGenerateInventory(projectId: string): Promise<string> {
	return invoke<string>('ansible_generate_inventory', { projectId });
}

export async function ansibleWriteInventory(
	projectId: string,
	content: string,
	filename?: string
): Promise<void> {
	return invoke<void>('ansible_write_inventory', { projectId, content, filename });
}

export async function ansibleListRoles(projectId: string): Promise<AnsibleRole[]> {
	return invoke<AnsibleRole[]>('ansible_list_roles', { projectId });
}

export async function ansibleListCollections(projectId: string): Promise<AnsibleCollection[]> {
	return invoke<AnsibleCollection[]>('ansible_list_collections', { projectId });
}

export async function ansibleVaultView(projectId: string, vaultFile: string): Promise<string> {
	return invoke<string>('ansible_vault_view', { projectId, vaultFile });
}
