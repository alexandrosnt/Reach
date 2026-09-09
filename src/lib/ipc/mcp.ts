import { invoke } from '@tauri-apps/api/core';

/**
 * How much the user wants to be asked before a command runs.
 *
 * Only the human prompt is affected. Every guard — read-before-write, the
 * echo-off lockout, the secret check, the rate limit, every deny rule — applies
 * in all three modes. Auto removes a question, never a protection.
 */
export type McpMode = 'ask' | 'auto_safe' | 'auto' | 'dangerous';

/** Per-session overrides. `null` on a field means "follow the global value". */
export interface SessionSettings {
	sessionId: string;
	agentId: string | null;
	mode: McpMode | null;
}

/** State of the MCP server. Everything defaults to off/empty. */
export interface McpStatus {
	enabled: boolean;
	/** Bound port. Absent when not listening. */
	port?: number;
	/** Stored encrypted in the settings vault and reused across restarts. Use
	 *  mcpRegenerateToken to revoke every client configured with the old one. */
	token?: string;
	agentId: string;
	agentName: string;
	/** True when the active agent has no `send_input` tool at all. */
	readOnly: boolean;
	sharedSessionIds: string[];
	mode: McpMode;
	sessions: SessionSettings[];
	/** Ready to paste into a client config. Absent when not listening. */
	url?: string;
}

export interface AgentSummary {
	id: string;
	name: string;
	description: string;
	readOnly: boolean;
	tools: string[];
}

/** Payload of the `mcp-confirm-request` event. */
export interface McpConfirmRequest {
	promptId: string;
	sessionId: string;
	host: string;
	command: string;
	rationale: string;
	references: string[];
	rollback?: string;
	agentName: string;
	/** "Benign" | "Mutating" | "Sensitive" | "Destructive" */
	danger: string;
	dangerReason?: string;
}

/** Start listening. `port` omitted or 0 lets the OS pick a free one. */
export async function mcpStart(port?: number): Promise<McpStatus> {
	return invoke<McpStatus>('mcp_start', { port: port ?? null });
}

/** Stop and revoke: the token dies, sharing is cleared, preconditions are dropped. */
export async function mcpStop(): Promise<McpStatus> {
	return invoke<McpStatus>('mcp_stop');
}

/**
 * Reload saved settings and, if the server was enabled before, start it again.
 *
 * Called once the vault is available rather than at process start: these live
 * in the vault, which may still be locked when the app boots. Restores agent,
 * mode and the port; never restores sharing, which is always a fresh decision.
 */
export async function mcpRestore(): Promise<McpStatus> {
	return invoke<McpStatus>('mcp_restore');
}

export async function mcpStatus(): Promise<McpStatus> {
	return invoke<McpStatus>('mcp_status');
}

/** Offer one session. Enabling the server does not do this. */
export async function mcpShareSession(
	sessionId: string,
	kind: 'ssh' | 'local',
	host: string,
	username: string
): Promise<string[]> {
	return invoke<string[]>('mcp_share_session', {
		session_id: sessionId,
		kind,
		host,
		username
	});
}

export async function mcpUnshareSession(sessionId: string): Promise<string[]> {
	return invoke<string[]>('mcp_unshare_session', { session_id: sessionId });
}

export async function mcpListAgents(): Promise<AgentSummary[]> {
	return invoke<AgentSummary[]>('mcp_list_agents');
}

/** Only reachable from here — there is no MCP method that sets the agent. */
export async function mcpSetAgent(agentId: string): Promise<McpStatus> {
	return invoke<McpStatus>('mcp_set_agent', { agent_id: agentId });
}

/** Choose how much to be asked. Reach-only — no MCP method reaches this. */
export async function mcpSetMode(mode: McpMode): Promise<McpStatus> {
	return invoke<McpStatus>('mcp_set_mode', { mode });
}

/**
 * Point one shared session at a different agent. `null` follows the global one.
 *
 * A per-session agent can only *narrow* the tool surface: MCP advertises one
 * tool list per connection, so the global agent stays the ceiling.
 */
export async function mcpSetSessionAgent(
	sessionId: string,
	agentId: string | null
): Promise<McpStatus> {
	return invoke<McpStatus>('mcp_set_session_agent', {
		session_id: sessionId,
		agent_id: agentId
	});
}

/** Point one shared session at a different mode. `null` follows the global one. */
export async function mcpSetSessionMode(
	sessionId: string,
	mode: McpMode | null
): Promise<McpStatus> {
	return invoke<McpStatus>('mcp_set_session_mode', { session_id: sessionId, mode });
}

/** Replace the token, revoking every client configured with the old one. */
export async function mcpRegenerateToken(): Promise<McpStatus> {
	return invoke<McpStatus>('mcp_regenerate_token');
}

/** Resolve a pending confirmation. Not answering is a rejection. */
export async function mcpConfirmResponse(promptId: string, approved: boolean): Promise<void> {
	return invoke('mcp_confirm_response', { prompt_id: promptId, approved });
}
