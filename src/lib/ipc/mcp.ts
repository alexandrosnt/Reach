import { invoke } from '@tauri-apps/api/core';

/** State of the MCP server. Everything defaults to off/empty. */
export interface McpStatus {
	enabled: boolean;
	/** Bound port. Absent when not listening. */
	port?: number;
	/** Regenerated on every start and never persisted, so it changes across restarts. */
	token?: string;
	agentId: string;
	agentName: string;
	/** True when the active agent has no `send_input` tool at all. */
	readOnly: boolean;
	sharedSessionIds: string[];
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

/** Resolve a pending confirmation. Not answering is a rejection. */
export async function mcpConfirmResponse(promptId: string, approved: boolean): Promise<void> {
	return invoke('mcp_confirm_response', { prompt_id: promptId, approved });
}
