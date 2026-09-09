/**
 * MCP server state.
 *
 * Mirrors the backend rather than owning anything: the backend is the authority
 * on whether a listener exists, what the token is, and which sessions are
 * shared. Every mutation goes through IPC and the reply replaces this.
 */
import * as mcpIpc from '$lib/ipc/mcp';
import type { AgentSummary, McpMode, McpStatus } from '$lib/ipc/mcp';

const EMPTY: McpStatus = {
	enabled: false,
	agentId: 'architect',
	agentName: 'Architect',
	readOnly: true,
	sharedSessionIds: [],
	mode: 'ask',
	sessions: []
};

let status = $state<McpStatus>({ ...EMPTY });
let agents = $state<AgentSummary[]>([]);
let busy = $state(false);
let error = $state<string | null>(null);

export function getMcpStatus(): McpStatus {
	return status;
}

export function getAgents(): AgentSummary[] {
	return agents;
}

export function isBusy(): boolean {
	return busy;
}

export function getError(): string | null {
	return error;
}

/** True when a session is exposed right now. Drives the status-bar indicator. */
export function isExposing(): boolean {
	return status.enabled && status.sharedSessionIds.length > 0;
}

export function isShared(sessionId: string): boolean {
	return status.sharedSessionIds.includes(sessionId);
}

async function run<T>(fn: () => Promise<T>): Promise<T | null> {
	busy = true;
	error = null;
	try {
		return await fn();
	} catch (err) {
		error = String(err);
		return null;
	} finally {
		busy = false;
	}
}

export async function refresh(): Promise<void> {
	const s = await run(() => mcpIpc.mcpStatus());
	if (s) status = s;
}

export async function loadAgents(): Promise<void> {
	const list = await run(() => mcpIpc.mcpListAgents());
	if (list) agents = list;
}

export async function start(port?: number): Promise<void> {
	const s = await run(() => mcpIpc.mcpStart(port));
	if (s) status = s;
}

export async function stop(): Promise<void> {
	const s = await run(() => mcpIpc.mcpStop());
	if (s) status = s;
}

export async function setAgent(agentId: string): Promise<void> {
	const s = await run(() => mcpIpc.mcpSetAgent(agentId));
	if (s) status = s;
}

export async function setMode(mode: McpMode): Promise<void> {
	const s = await run(() => mcpIpc.mcpSetMode(mode));
	if (s) status = s;
}

export async function regenerateToken(): Promise<void> {
	const s = await run(() => mcpIpc.mcpRegenerateToken());
	if (s) status = s;
}

export async function setSessionAgent(sessionId: string, agentId: string | null): Promise<void> {
	const s = await run(() => mcpIpc.mcpSetSessionAgent(sessionId, agentId));
	if (s) status = s;
}

export async function setSessionMode(sessionId: string, mode: McpMode | null): Promise<void> {
	const s = await run(() => mcpIpc.mcpSetSessionMode(sessionId, mode));
	if (s) status = s;
}

export async function share(
	sessionId: string,
	kind: 'ssh' | 'local',
	host: string,
	username: string
): Promise<void> {
	const ids = await run(() => mcpIpc.mcpShareSession(sessionId, kind, host, username));
	if (ids) status = { ...status, sharedSessionIds: ids };
}

export async function unshare(sessionId: string): Promise<void> {
	const ids = await run(() => mcpIpc.mcpUnshareSession(sessionId));
	if (ids) status = { ...status, sharedSessionIds: ids };
}
