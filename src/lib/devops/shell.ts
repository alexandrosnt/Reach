/**
 * Opening a shell inside a container, as a normal terminal tab.
 *
 * The tab runs `docker exec -it <id> sh` (bash when the image has it) on the
 * host the container is on: through the saved session, with every prompt a
 * normal connect has; through the terminal tab's own connection; or here.
 */

import type { Engine, Route } from '$lib/ipc/containers';
import { ctrShellCommand } from '$lib/ipc/containers';
import { sshConnect } from '$lib/ipc/ssh';
import { createTab, getTabs } from '$lib/state/tabs.svelte';
import { setActivePage } from '$lib/state/navigation.svelte';
import { requestSessionAction } from '$lib/state/palette.svelte';

/** bash if the image has it, sh otherwise. */
const PICK_SHELL = 'if command -v bash >/dev/null 2>&1; then exec bash; else exec sh; fi';

export async function openContainerShell(route: Route, engine: Engine, containerId: string, containerName: string): Promise<void> {
	if (route.kind === 'direct') {
		createTab('local', `${containerName} · ${engine}`).localCommand = {
			program: engine,
			args: ['exec', '-it', containerId, 'sh', '-c', PICK_SHELL]
		};
		setActivePage('terminal');
		return;
	}

	const command = await ctrShellCommand(engine, containerId);
	if (route.kind === 'session') {
		// The session list connects it, and is shown for it if another panel
		// was open (an event to a list not on screen did nothing).
		requestSessionAction({ kind: 'connect', sessionId: route.sessionId, shell: command });
		return;
	}

	// A terminal tab's connection: open another login with the same settings.
	const from = getTabs().find((t) => t.connectionId === route.connectionId);
	if (!from?.sshConnectParams) throw new Error('That terminal tab is no longer connected');
	const id = crypto.randomUUID();
	const params = { ...from.sshConnectParams, id, shell: command, injectColors: false };
	await sshConnect(params);
	const tab = createTab('ssh', `${containerName} · ${from.title}`, id, from.sessionName, from.detectedOs);
	tab.sshConnectParams = params;
	setActivePage('terminal');
}
