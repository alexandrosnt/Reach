import { invoke } from '@tauri-apps/api/core';

export interface ToolStatus {
	installed: boolean;
	version: string | null;
	path: string | null;
	warning: string | null;
	localUnsupported: boolean;
}

export interface ToolInstallEvent {
	tool: string;
	message: string;
	done: boolean;
	success: boolean;
}

export async function toolchainCheck(tool: string): Promise<ToolStatus> {
	return invoke<ToolStatus>('toolchain_check', { tool });
}

export async function toolchainInstall(tool: string): Promise<ToolStatus> {
	return invoke<ToolStatus>('toolchain_install', { tool });
}
