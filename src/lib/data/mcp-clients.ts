/**
 * How each MCP client is told about a Streamable HTTP server with a bearer
 * token. Verified against official documentation, September 2026.
 *
 * This file exists because the ecosystem has not converged, and the
 * disagreements are silent ones — a wrong key name produces a client that
 * simply never connects, with no error worth reading:
 *
 * - Four top-level keys: `mcpServers` (most), `servers` (VS Code),
 *   `context_servers` (Zed), `[mcp_servers.<name>]` (Codex TOML).
 * - Three URL keys: `url` (most), `serverUrl` (Windsurf), `httpUrl` (Gemini —
 *   where plain `url` means SSE instead).
 * - Three spellings of the transport discriminator: `"http"` (VS Code),
 *   `"streamableHttp"` (Cline), `"streamable-http"` (Continue) — and several
 *   clients have no discriminator at all, inferring it from the URL key.
 * - **Cline falls back to legacy SSE when `type` is omitted**, so it must
 *   always be emitted there even though it looks redundant.
 *
 * `verified` records whether the exact syntax appears in that vendor's own
 * docs. Where it does not, the entry says so in the UI rather than presenting a
 * guess as fact — a copy-paste command that silently fails is worse than an
 * honest "check your client's docs".
 */

export type ClientKind = 'cli' | 'json' | 'toml' | 'yaml';

export interface McpClient {
	id: string;
	name: string;
	/** Key into `clientIcons`; falls back to a neutral glyph when absent. */
	icon: string;
	kind: ClientKind;
	/** One-liner for `kind: 'cli'`. Placeholders: {{url}}, {{token}}. */
	command?: string;
	/** Where the snippet goes, for file-based clients. */
	path?: string;
	/** Config snippet. Same placeholders. */
	snippet?: string;
	/** Shown under the snippet. Use for the traps above. */
	note?: string;
	/** False when the syntax is not in the vendor's own documentation. */
	verified: boolean;
	docs: string;
}

export const MCP_CLIENTS: McpClient[] = [
	{
		id: 'claude-code',
		name: 'Claude Code',
		icon: 'claude',
		kind: 'cli',
		command:
			'claude mcp add --transport http reach {{url}} \\\n  --header "Authorization: Bearer {{token}}"',
		note: 'Add --scope user to make it available in every project.',
		verified: true,
		docs: 'https://code.claude.com/docs/en/mcp'
	},
	{
		id: 'gemini-cli',
		name: 'Gemini CLI',
		icon: 'gemini',
		kind: 'cli',
		command:
			'gemini mcp add --transport http \\\n  --header "Authorization: Bearer {{token}}" \\\n  reach {{url}}',
		verified: true,
		docs: 'https://github.com/google-gemini/gemini-cli/blob/main/docs/tools/mcp-server.md'
	},
	{
		id: 'cursor',
		name: 'Cursor',
		icon: 'cursor',
		kind: 'json',
		path: '~/.cursor/mcp.json  (or .cursor/mcp.json in a project)',
		snippet: `{
  "mcpServers": {
    "reach": {
      "url": "{{url}}",
      "headers": { "Authorization": "Bearer {{token}}" }
    }
  }
}`,
		note: 'Cursor infers the transport from the URL — remote servers take no "type" field.',
		verified: true,
		docs: 'https://cursor.com/docs/mcp'
	},
	{
		id: 'vscode',
		name: 'VS Code (Copilot)',
		// Simple Icons dropped the VS Code mark over trademark policy; Copilot
		// is the feature this actually configures, and its mark exists.
		icon: 'copilot',
		kind: 'json',
		path: '.vscode/mcp.json  (or MCP: Open User Configuration)',
		snippet: `{
  "servers": {
    "reach": {
      "type": "http",
      "url": "{{url}}",
      "headers": { "Authorization": "Bearer {{token}}" }
    }
  }
}`,
		note: 'Top-level key is "servers", not "mcpServers" — VS Code is the odd one out here.',
		verified: true,
		docs: 'https://code.visualstudio.com/docs/agents/reference/mcp-configuration'
	},
	{
		id: 'windsurf',
		name: 'Windsurf',
		icon: 'windsurf',
		kind: 'json',
		path: '~/.codeium/windsurf/mcp_config.json',
		snippet: `{
  "mcpServers": {
    "reach": {
      "serverUrl": "{{url}}",
      "headers": { "Authorization": "Bearer {{token}}" }
    }
  }
}`,
		note: 'The URL key is "serverUrl" here, not "url".',
		verified: true,
		docs: 'https://docs.windsurf.com/windsurf/cascade/mcp'
	},
	{
		id: 'zed',
		name: 'Zed',
		icon: 'zed',
		kind: 'json',
		path: 'settings.json',
		snippet: `{
  "context_servers": {
    "reach": {
      "url": "{{url}}",
      "headers": { "Authorization": "Bearer {{token}}" }
    }
  }
}`,
		note: 'Zed calls them "context_servers". Without an Authorization header it will try OAuth instead.',
		verified: true,
		docs: 'https://zed.dev/docs/ai/mcp'
	},
	{
		id: 'cline',
		name: 'Cline',
		icon: '',
		kind: 'json',
		path: '~/.cline/mcp.json  (or the MCP Servers panel)',
		snippet: `{
  "mcpServers": {
    "reach": {
      "type": "streamableHttp",
      "url": "{{url}}",
      "headers": { "Authorization": "Bearer {{token}}" },
      "disabled": false,
      "autoApprove": []
    }
  }
}`,
		note: 'Keep "type": leaving it out makes Cline fall back to legacy SSE, which this server does not speak.',
		verified: true,
		docs: 'https://docs.cline.bot/mcp/configuring-mcp-servers'
	},
	{
		id: 'continue',
		name: 'Continue',
		icon: '',
		kind: 'yaml',
		path: '~/.continue/config.yaml  (or .continue/mcpServers/reach.yaml)',
		snippet: `mcpServers:
  - name: Reach
    type: streamable-http
    url: {{url}}
    requestOptions:
      headers:
        Authorization: Bearer {{token}}`,
		note: 'mcpServers is a list here, not a map. Continue documents requestOptions.headers by schema but ships no verbatim example — check it connects.',
		verified: false,
		docs: 'https://docs.continue.dev/reference'
	},
	{
		id: 'codex',
		name: 'Codex CLI',
		icon: '',
		kind: 'toml',
		path: '~/.codex/config.toml',
		snippet: `[mcp_servers.reach]
url = "{{url}}"
bearer_token_env_var = "REACH_MCP_TOKEN"`,
		note: 'Codex reads the token from an environment variable rather than inline, so export REACH_MCP_TOKEN={{token}} first. Its docs only show the stdio form of "codex mcp add", so the file is the reliable route.',
		verified: true,
		docs: 'https://developers.openai.com/codex/mcp/'
	},
	{
		id: 'claude-desktop',
		name: 'Claude Desktop',
		icon: 'claude',
		kind: 'json',
		path: 'macOS: ~/Library/Application Support/Claude/claude_desktop_config.json\nWindows: %APPDATA%\\Claude\\claude_desktop_config.json',
		snippet: `{
  "mcpServers": {
    "reach": {
      "command": "npx",
      "args": ["-y", "mcp-remote", "{{url}}",
               "--allow-http",
               "--header", "Authorization: Bearer {{token}}"]
    }
  }
}`,
		note: 'Claude Desktop’s config file is stdio-only, so this bridges through mcp-remote. Its custom-connector UI does OAuth and has no field for a bearer token. The mcp-remote flags come from that package rather than Anthropic’s docs.',
		verified: false,
		docs: 'https://modelcontextprotocol.io/docs/develop/connect-remote-servers'
	},
	{
		id: 'jetbrains',
		name: 'JetBrains AI',
		icon: 'jetbrains',
		kind: 'json',
		path: 'Settings | Tools | AI Assistant | Model Context Protocol (MCP) → Add',
		snippet: `{
  "mcpServers": {
    "reach": {
      "url": "{{url}}"
    }
  }
}`,
		note: 'JetBrains documents no way to send an Authorization header, so this will be refused with 401 by Reach. Listed for completeness — it needs header support before it can work.',
		verified: false,
		docs: 'https://www.jetbrains.com/help/ai-assistant/mcp.html'
	}
];

/** Substitute the live URL and token into a template. */
export function fill(template: string, url: string, token: string): string {
	return template.replaceAll('{{url}}', url).replaceAll('{{token}}', token);
}
