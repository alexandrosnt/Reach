/**
 * The step each SSH connection being made has reached (ConnectStage in
 * ssh/client.rs). One listener from the start, not one per dialog: a
 * listener set up when a connect begins can miss its first step, and a
 * closed port sends only that one.
 */
import { listen } from '@tauri-apps/api/event';

export const CONNECT_STEPS = ['connect', 'kex', 'hostkey', 'auth', 'shell'] as const;
export type ConnectStep = (typeof CONNECT_STEPS)[number];

const reached = $state<Record<string, ConnectStep>>({});

void listen<{ id: string; stage: ConnectStep }>('ssh-connect-stage', (e) => {
	if (CONNECT_STEPS.includes(e.payload.stage)) reached[e.payload.id] = e.payload.stage;
}).catch(() => {
	// No backend (a test): no steps, and the dialogs show "Connecting…".
});

/** The last step this connection began, if it reported any. */
export function connectStep(id: string | undefined): ConnectStep | null {
	return id ? (reached[id] ?? null) : null;
}
