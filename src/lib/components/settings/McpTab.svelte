<script lang="ts">
	/**
	 * The MCP wizard: pick a port, start listening, then get the exact
	 * incantation for your client.
	 *
	 * The last step is the point of the whole panel. Every client spells this
	 * differently — four top-level keys, three URL keys, three transport
	 * spellings — and getting one wrong produces a client that silently never
	 * connects. Reach knows its own URL and token, so it can just hand over the
	 * finished thing rather than leaving people to translate documentation.
	 *
	 * The token is regenerated on every start and never persisted, so this
	 * panel is also the only place to read it. That is stated rather than left
	 * to be discovered after a restart.
	 */
	import { onMount } from 'svelte';
	import Button from '$lib/components/shared/Button.svelte';
	import Toggle from '$lib/components/shared/Toggle.svelte';
	import McpClientIcon from '$lib/components/shared/McpClientIcon.svelte';
	import { MCP_CLIENTS, fill } from '$lib/data/mcp-clients';
	import * as mcp from '$lib/state/mcp.svelte';
	import { addToast } from '$lib/state/toasts.svelte';
	import { t } from '$lib/state/i18n.svelte';

	let status = $derived(mcp.getMcpStatus());
	let agents = $derived(mcp.getAgents());
	let busy = $derived(mcp.isBusy());

	/** 0 asks the OS for a free port, which is the right default: a fixed one
	 *  that collides turns into "MCP doesn't work". */
	let portInput = $state('0');
	let selectedClient = $state(MCP_CLIENTS[0].id);

	let client = $derived(MCP_CLIENTS.find((c) => c.id === selectedClient) ?? MCP_CLIENTS[0]);
	let rendered = $derived(
		status.url && status.token
			? fill(client.kind === 'cli' ? (client.command ?? '') : (client.snippet ?? ''), status.url, status.token)
			: ''
	);

	onMount(() => {
		mcp.refresh();
		mcp.loadAgents();
	});

	async function toggle(on: boolean): Promise<void> {
		if (on) {
			const p = Number.parseInt(portInput, 10);
			await mcp.start(Number.isFinite(p) && p > 0 ? p : undefined);
			const err = mcp.getError();
			if (err) addToast(err, 'error');
		} else {
			await mcp.stop();
		}
	}

	async function copy(text: string): Promise<void> {
		try {
			await navigator.clipboard.writeText(text);
			addToast(t('mcp.copied'), 'info');
		} catch {
			addToast(t('mcp.copy_failed'), 'error');
		}
	}
</script>

<div class="tab-content">
	<!-- Step 1: the switch. Off means no socket exists at all. -->
	<div class="setting-row">
		<div class="setting-info">
			<span class="setting-label">{t('mcp.enable')}</span>
			<span class="setting-description">{t('mcp.enable_desc')}</span>
		</div>
		<div class="setting-control">
			<Toggle checked={status.enabled} onchange={toggle} disabled={busy} />
		</div>
	</div>

	{#if !status.enabled}
		<div class="setting-row">
			<div class="setting-info">
				<span class="setting-label">{t('mcp.port')}</span>
				<span class="setting-description">{t('mcp.port_desc')}</span>
			</div>
			<div class="setting-control">
				<input class="port" type="number" min="0" max="65535" bind:value={portInput} />
			</div>
		</div>
	{/if}

	{#if status.enabled}
		<!-- Step 2: what is actually exposed. Sharing is separate from running,
		     and the difference is worth stating plainly. -->
		<div class="panel">
			<div class="live">
				<span class="dot" class:exposing={mcp.isExposing()}></span>
				<span class="live-text">
					{#if mcp.isExposing()}
						{t('mcp.exposing', { count: status.sharedSessionIds.length })}
					{:else}
						{t('mcp.listening_nothing_shared')}
					{/if}
				</span>
			</div>

			<div class="kv">
				<span class="k">{t('mcp.url')}</span>
				<code class="v">{status.url}</code>
				<button class="copy" onclick={() => copy(status.url ?? '')}>{t('mcp.copy')}</button>
			</div>
			<div class="kv">
				<span class="k">{t('mcp.token')}</span>
				<code class="v token">{status.token}</code>
				<button class="copy" onclick={() => copy(status.token ?? '')}>{t('mcp.copy')}</button>
			</div>
			<p class="hint">{t('mcp.token_hint')}</p>
		</div>

		<!-- Step 3: the agent decides the tool surface, not just the tone. -->
		<div class="section">
			<span class="section-label">{t('mcp.agent')}</span>
			<p class="section-hint">{t('mcp.agent_desc')}</p>
			<div class="agents">
				{#each agents as a (a.id)}
					<button
						class="agent"
						class:selected={status.agentId === a.id}
						onclick={() => mcp.setAgent(a.id)}
						disabled={busy}
					>
						<span class="agent-head">
							<span class="agent-name">{a.name}</span>
							{#if a.readOnly}
								<span class="ro">{t('mcp.read_only')}</span>
							{/if}
						</span>
						<span class="agent-desc">{a.description}</span>
						<span class="agent-tools">{a.tools.join(' · ')}</span>
					</button>
				{/each}
			</div>
		</div>

		<!-- Step 4: the payoff. -->
		<div class="section">
			<span class="section-label">{t('mcp.connect')}</span>
			<p class="section-hint">{t('mcp.connect_desc')}</p>

			<div class="clients">
				{#each MCP_CLIENTS as c (c.id)}
					<button
						class="client"
						class:selected={selectedClient === c.id}
						onclick={() => (selectedClient = c.id)}
						title={c.name}
					>
						<McpClientIcon slug={c.icon} size={20} />
						<span class="client-name">{c.name}</span>
					</button>
				{/each}
			</div>

			{#if client.path}
				<div class="path">
					<span class="k">{t('mcp.config_path')}</span>
					<code class="v path-v">{client.path}</code>
				</div>
			{/if}

			<div class="snippet-wrap">
				<div class="snippet-bar">
					<span class="snippet-kind">{client.kind}</span>
					<button class="copy" onclick={() => copy(rendered)}>{t('mcp.copy')}</button>
				</div>
				<pre class="snippet">{rendered}</pre>
			</div>

			{#if client.note}
				<p class="note">{client.note}</p>
			{/if}
			{#if !client.verified}
				<p class="unverified">{t('mcp.unverified')}</p>
			{/if}
			<a class="docs" href={client.docs} target="_blank" rel="noreferrer noopener">
				{t('mcp.client_docs', { name: client.name })}
			</a>
		</div>
	{/if}
</div>

<style>
	.tab-content {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
	}

	.setting-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-4);
	}

	.setting-info {
		display: flex;
		flex-direction: column;
		gap: 2px;
		min-width: 0;
	}

	.setting-label {
		font-size: var(--text-sm);
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.setting-description {
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
	}

	.setting-control {
		flex-shrink: 0;
	}

	.port {
		width: 110px;
		padding: 6px var(--space-2);
		font-family: var(--font-mono);
		font-size: var(--text-sm);
		color: var(--color-text-primary);
		background: var(--color-surface-sunken);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-sm);
		outline: none;
	}

	.panel {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		padding: var(--space-3);
		background: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-card);
	}

	.live {
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}

	.dot {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: var(--color-text-tertiary);
		flex-shrink: 0;
	}

	/* Amber, not green: a session being readable by an AI is a state worth
	   noticing, not a success to celebrate. */
	.dot.exposing {
		background: var(--color-warning);
	}

	.live-text {
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
	}

	.kv,
	.path {
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}

	.k {
		flex-shrink: 0;
		width: 72px;
		font-size: var(--text-2xs);
		text-transform: uppercase;
		letter-spacing: 0.04em;
		color: var(--color-text-tertiary);
	}

	.v {
		flex: 1;
		min-width: 0;
		padding: 4px var(--space-2);
		font-family: var(--font-mono);
		font-size: var(--text-xs);
		color: var(--color-text-primary);
		background: var(--color-surface-sunken);
		border-radius: var(--radius-sm);
		overflow-x: auto;
		white-space: nowrap;
	}

	.path-v {
		white-space: pre-wrap;
	}

	.token {
		/* Long and opaque; let it scroll rather than reflow the panel. */
		letter-spacing: 0.02em;
	}

	.copy {
		flex-shrink: 0;
		padding: 4px 10px;
		font-size: var(--text-2xs);
		font-family: inherit;
		color: var(--color-text-secondary);
		background: none;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-sm);
		cursor: pointer;
	}

	.copy:hover {
		color: var(--color-text-primary);
		background: var(--color-surface-hover);
	}

	.hint,
	.note,
	.section-hint {
		margin: 0;
		font-size: var(--text-xs);
		color: var(--color-text-tertiary);
	}

	.unverified {
		margin: 0;
		font-size: var(--text-xs);
		color: var(--color-warning);
	}

	.section {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}

	.section-label {
		font-size: var(--text-2xs);
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.04em;
		color: var(--color-text-tertiary);
	}

	.agents {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
		gap: var(--space-2);
	}

	.agent {
		display: flex;
		flex-direction: column;
		gap: 3px;
		padding: var(--space-2) var(--space-3);
		text-align: left;
		background: var(--color-bg-elevated);
		border: 2px solid var(--color-border);
		border-radius: var(--radius-card);
		cursor: pointer;
	}

	.agent:hover {
		background: var(--color-surface-hover);
	}

	.agent.selected {
		border-color: var(--color-accent);
	}

	.agent-head {
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}

	.agent-name {
		font-size: var(--text-sm);
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.ro {
		padding: 1px 6px;
		font-size: 0.625rem;
		font-weight: 700;
		text-transform: uppercase;
		color: var(--color-success);
		border: 1px solid var(--color-success);
		border-radius: var(--radius-sm);
	}

	.agent-desc {
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
	}

	.agent-tools {
		font-family: var(--font-mono);
		font-size: 0.625rem;
		color: var(--color-text-tertiary);
	}

	.clients {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
	}

	.client {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		padding: 6px var(--space-3);
		font-family: inherit;
		background: var(--color-bg-elevated);
		border: 2px solid var(--color-border);
		border-radius: var(--radius-btn);
		cursor: pointer;
	}

	.client:hover {
		background: var(--color-surface-hover);
	}

	.client.selected {
		border-color: var(--color-accent);
	}

	.client-name {
		font-size: var(--text-xs);
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.snippet-wrap {
		border: 1px solid var(--color-border);
		border-radius: var(--radius-sm);
		overflow: hidden;
	}

	/* The copy button lives in its own bar rather than floating over the code.
	   Floated, it sat on top of a horizontally scrolling <pre> and hid whatever
	   scrolled underneath it — on a panel whose whole job is handing over an
	   exact string. */
	.snippet-bar {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 4px 4px 4px var(--space-3);
		background: var(--color-bg-elevated);
		border-bottom: 1px solid var(--color-border);
	}

	.snippet-kind {
		font-size: 0.625rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.06em;
		color: var(--color-text-tertiary);
	}

	.snippet {
		margin: 0;
		padding: var(--space-3);
		font-family: var(--font-mono);
		font-size: var(--text-xs);
		line-height: 1.55;
		color: var(--color-text-primary);
		background: var(--color-surface-sunken);
		overflow-x: auto;
	}

	.docs {
		font-size: var(--text-xs);
		color: var(--color-accent);
		text-decoration: none;
	}

	.docs:hover {
		text-decoration: underline;
	}
</style>
