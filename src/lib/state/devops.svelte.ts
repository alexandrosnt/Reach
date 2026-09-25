/**
 * The DevOps workspaces, and which of them are switched on.
 *
 * Everything that shows or reaches a DevOps tool reads this list: the tab bar,
 * the page that mounts it, and Settings → DevOps. Adding a tool is one entry
 * here, a variant in `src-tauri/src/devops.rs`, and a `require` at the top of
 * its commands.
 *
 * Switching a tool off hides it and makes the backend refuse its commands.
 * Nothing is deleted; projects and runs are there again when it comes back.
 */

import { invoke } from '@tauri-apps/api/core';
import { getSettings, updateSetting } from '$lib/state/settings.svelte';
import { getActivePage, setActivePage, type Page } from '$lib/state/navigation.svelte';
import { isCommandRunning as isAnsibleRunning } from '$lib/state/ansible.svelte';
import { isCommandRunning as isTofuRunning } from '$lib/state/tofu.svelte';
import { isAnyBusy as isDbBusy } from '$lib/state/db.svelte';
import { t } from '$lib/state/i18n.svelte';

export type DevopsToolId = 'ansible' | 'tofu' | 'databases';

export interface DevopsTool {
	id: DevopsToolId;
	/** The page it opens. Shares the id, so the tab bar can use either. */
	page: Page & DevopsToolId;
	/** Binary name for `toolchain_check`; empty for a tool Reach carries itself. */
	binary: string;
	label: () => string;
	description: () => string;
	/** A run in progress. The switch waits for it rather than cutting it off. */
	isBusy: () => boolean;
}

export const DEVOPS_TOOLS: DevopsTool[] = [
	{
		id: 'ansible',
		page: 'ansible',
		binary: 'ansible',
		label: () => t('nav.ansible'),
		description: () => t('devops.ansible_desc'),
		isBusy: isAnsibleRunning
	},
	{
		id: 'tofu',
		page: 'tofu',
		binary: 'tofu',
		label: () => t('nav.tofu'),
		description: () => t('devops.tofu_desc'),
		isBusy: isTofuRunning
	},
	{
		id: 'databases',
		page: 'databases',
		binary: '',
		label: () => t('nav.databases'),
		description: () => t('devops.databases_desc'),
		isBusy: isDbBusy
	}
];

export function isDevopsEnabled(id: DevopsToolId): boolean {
	return getSettings().devopsTools.includes(id);
}

export function getEnabledDevopsTools(): DevopsTool[] {
	return DEVOPS_TOOLS.filter((tool) => isDevopsEnabled(tool.id));
}

/**
 * Switch a tool on or off. Refuses to switch off mid-run and returns false,
 * though Settings disables the control first so this is only a backstop.
 */
export async function setDevopsEnabled(id: DevopsToolId, enabled: boolean): Promise<boolean> {
	const tool = DEVOPS_TOOLS.find((d) => d.id === id);
	if (!tool) return false;
	if (!enabled && tool.isBusy()) return false;

	const others = getSettings().devopsTools.filter((d) => d !== id);
	updateSetting('devopsTools', enabled ? [...others, id] : others);
	if (!enabled && getActivePage() === tool.page) setActivePage('terminal');
	await pushToBackend(id, enabled);
	return true;
}

/** Tell the backend what is on. Call once after `loadSettings()`. */
export async function syncDevopsTools(): Promise<void> {
	await Promise.all(DEVOPS_TOOLS.map((tool) => pushToBackend(tool.id, isDevopsEnabled(tool.id))));
}

async function pushToBackend(tool: DevopsToolId, enabled: boolean): Promise<void> {
	try {
		await invoke('devops_set_enabled', { tool, enabled });
	} catch {
		// Backend not ready yet
	}
}
