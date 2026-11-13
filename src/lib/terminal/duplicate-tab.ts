/**
 * Duplicating a tab (#160): the same kind of session again, in a new tab
 * just after the original and in its group.
 *
 * An SSH tab opens another login with the tab's own connect settings, as
 * Reconnect does, so it asks nothing a reconnect would not. Remote desktops
 * are not duplicated: a second RDP login as the same user usually logs the
 * first one off.
 */

import { sshConnect } from '#lib/ipc/ssh.js';
import { createTab, getTabs, placeAfter, type Tab } from '#lib/state/tabs.svelte.js';

export function canDuplicate(tab: Tab | undefined): boolean {
	if (!tab) return false;
	return tab.type === 'local' || (tab.type === 'ssh' && !!tab.sshConnectParams);
}

export async function duplicateTab(id: string): Promise<void> {
	const from = getTabs().find((t) => t.id === id);
	if (!from || !canDuplicate(from)) return;

	if (from.type === 'local') {
		// A WSL tab is known by its connection id; a plain shell has none.
		const wsl = from.connectionId?.startsWith('wsl-');
		const tab = createTab(
			'local',
			// A plain shell takes the next free name ("Local 2"); a shell in a
			// container or a distribution keeps the name that says which.
			from.localCommand ? from.title : undefined,
			wsl ? `wsl-${crypto.randomUUID()}` : undefined,
			from.sessionName,
			from.detectedOs,
			from.localCommand ? { program: from.localCommand.program, args: [...from.localCommand.args] } : undefined,
			from.wslDistro
		);
		placeAfter(tab.id, from.id);
		return;
	}

	const params = { ...from.sshConnectParams!, id: crypto.randomUUID() };
	await sshConnect(params);
	const tab = createTab('ssh', from.title, params.id, from.sessionName, from.detectedOs);
	tab.sshConnectParams = params;
	placeAfter(tab.id, from.id);
}
