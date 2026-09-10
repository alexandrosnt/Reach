<script lang="ts">
	/**
	 * MCP settings.
	 *
	 * Ordered as the decisions actually happen: turn it on, decide what the AI
	 * *is*, decide how much you want to be asked, see what is currently exposed,
	 * then connect a client. Each section states the consequence rather than
	 * just naming a knob — this panel is where someone decides how much of their
	 * infrastructure an AI can touch, and a bare toggle would undersell that.
	 *
	 * The connect step is the payoff: every client spells this differently, so
	 * Reach fills in its own URL and token and hands over the finished thing.
	 */
	import { onMount } from 'svelte';
	import Toggle from '$lib/components/shared/Toggle.svelte';
	import McpClientIcon from '$lib/components/shared/McpClientIcon.svelte';
	import { MCP_CLIENTS, fill } from '$lib/data/mcp-clients';
	import type { McpMode } from '$lib/ipc/mcp';
	import * as mcp from '$lib/state/mcp.svelte';
	import { addToast } from '$lib/state/toasts.svelte';
	import { t } from '$lib/state/i18n.svelte';

	let status = $derived(mcp.getMcpStatus());
	let agents = $derived(mcp.getAgents());
	let busy = $derived(mcp.isBusy());

	/** 0 asks the OS for a free port — the right default, because a fixed one
	 *  that collides turns into "MCP doesn't work". */
	let portInput = $state('0');
	let selectedClient = $state(MCP_CLIENTS[0].id);
	let revealToken = $state(false);

	const MODES: { id: McpMode; label: () => string; hint: () => string }[] = [
		{ id: 'ask', label: () => t('mcp.mode_ask'), hint: () => t('mcp.mode_ask_hint') },
		{ id: 'auto_safe', label: () => t('mcp.mode_auto_safe'), hint: () => t('mcp.mode_auto_safe_hint') },
		{ id: 'auto', label: () => t('mcp.mode_auto'), hint: () => t('mcp.mode_auto_hint') },
		{ id: 'dangerous', label: () => t('mcp.mode_dangerous'), hint: () => t('mcp.mode_dangerous_hint') }
	];

	/** Dangerous is the only mode that asks before it stops asking. Not to
	 *  discourage it — it is a legitimate choice on a machine you are willing
	 *  to lose — but because "an AI may now wipe this disk unprompted" should
	 *  never be one stray click away. */
	let confirmingDangerous = $state(false);

	async function pickMode(id: McpMode): Promise<void> {
		if (id === 'dangerous' && status.mode !== 'dangerous') {
			confirmingDangerous = true;
			return;
		}
		confirmingDangerous = false;
		await mcp.setMode(id);
	}

	let client = $derived(MCP_CLIENTS.find((c) => c.id === selectedClient) ?? MCP_CLIENTS[0]);
	let rendered = $derived(
		status.url && status.token
			? fill(
					client.kind === 'cli' ? (client.command ?? '') : (client.snippet ?? ''),
					status.url,
					status.token
				)
			: ''
	);

	/** Masked unless deliberately revealed. It opens a shell; it should not sit
	 *  in plain text on a screen someone might be sharing. */
	/** 24 bullets regardless of the real length: the token is 43 characters and
	 *  a faithful mask overflowed the field, leaving a scrollbar under a row of
	 *  dots. The count carries no information worth a scrollbar. */
	let shownToken = $derived(
		!status.token ? '' : revealToken ? status.token : '•'.repeat(24)
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
			revealToken = false;
		}
	}

	/** Regenerating locks out every client already configured with the old
	 *  token, which is the whole point of having it — but it is also silent and
	 *  irreversible, so it asks first. */
	let confirmingRegen = $state(false);

	async function regenerate(): Promise<void> {
		confirmingRegen = false;
		await mcp.regenerateToken();
		const err = mcp.getError();
		addToast(err ?? t('mcp.token_regenerated'), err ? 'error' : 'warning');
	}

	/** `root@10.144.144.2`, falling back to the id when a session predates the
	 *  descriptor carrying host and user. */
	function label(sess: { host: string; username: string; sessionId: string }): string {
		if (!sess.host) return sess.sessionId;
		return sess.username ? `${sess.username}@${sess.host}` : sess.host;
	}

	async function copy(text: string, label: string): Promise<void> {
		try {
			await navigator.clipboard.writeText(text);
			addToast(label, 'info');
		} catch {
			addToast(t('mcp.copy_failed'), 'error');
		}
	}
</script>

<div class="tab-content">
	<!-- 1 · The switch. Off means no socket exists at all. -->
	<section class="block">
		<div class="row">
			<div class="row-info">
				<span class="row-label">{t('mcp.enable')}</span>
				<span class="row-desc">{t('mcp.enable_desc')}</span>
			</div>
			<Toggle checked={status.enabled} onchange={toggle} disabled={busy} />
		</div>

		{#if !status.enabled}
			<div class="row">
				<div class="row-info">
					<span class="row-label">{t('mcp.port')}</span>
					<span class="row-desc">{t('mcp.port_desc')}</span>
				</div>
				<input class="port" type="number" min="0" max="65535" bind:value={portInput} />
			</div>
		{:else}
			<div class="live">
				<span class="dot" class:exposing={mcp.isExposing()}></span>
				<span class="live-text">
					{#if mcp.isExposing()}
						{t('mcp.exposing', { count: status.sessions.length })}
					{:else}
						{t('mcp.listening_nothing_shared')}
					{/if}
				</span>
				<span class="port-note">{t('mcp.port_locked')}</span>
			</div>

			<div class="kv">
				<span class="k">{t('mcp.url')}</span>
				<code class="v">{status.url}</code>
				<button class="mini" onclick={() => copy(status.url ?? '', t('mcp.copied'))}>
					{t('mcp.copy')}
				</button>
			</div>

			<div class="kv">
				<span class="k">{t('mcp.token')}</span>
				<code class="v">{shownToken}</code>
				<button class="mini" onclick={() => (revealToken = !revealToken)}>
					{revealToken ? t('mcp.hide') : t('mcp.reveal')}
				</button>
				<button class="mini" onclick={() => copy(status.token ?? '', t('mcp.copied'))}>
					{t('mcp.copy')}
				</button>
				<button class="mini danger" onclick={() => (confirmingRegen = true)} disabled={busy}>
					{t('mcp.regenerate')}
				</button>
			</div>
			{#if confirmingRegen}
				<div class="confirm" role="alertdialog">
					<p class="confirm-msg">{t('mcp.regenerate_confirm')}</p>
					<div class="confirm-actions">
						<button class="mini" onclick={() => (confirmingRegen = false)}>
							{t('mcp.confirm_reject')}
						</button>
						<button class="mini danger-solid" onclick={regenerate}>
							{t('mcp.regenerate')}
						</button>
					</div>
				</div>
			{/if}

			<p class="hint">{t('mcp.token_hint')}</p>
		{/if}
	</section>

	{#if status.enabled}
		<!-- 2 · The agent decides the tool surface, not just the tone. -->
		<section class="block">
			<div class="head">
				<span class="head-label">{t('mcp.agent')}</span>
				<span class="head-hint">{t('mcp.agent_desc')}</span>
			</div>
			<div class="agents">
				{#each agents as a (a.id)}
					<button
						class="agent"
						class:selected={status.agentId === a.id}
						onclick={() => mcp.setAgent(a.id)}
						disabled={busy}
						aria-pressed={status.agentId === a.id}
					>
						<span class="agent-head">
							<span class="agent-name">{a.name}</span>
							{#if a.readOnly}
								<span class="tag ok">{t('mcp.read_only')}</span>
							{/if}
						</span>
						<span class="agent-desc">{a.description}</span>
						<span class="agent-tools">{a.tools.join(' · ')}</span>
					</button>
				{/each}
			</div>
		</section>

		<!-- 3 · How much you want to be asked. Mirrors the status bar. -->
		<section class="block">
			<div class="head">
				<span class="head-label">{t('mcp.mode')}</span>
				<span class="head-hint">{t('mcp.mode_desc')}</span>
			</div>
			<div class="modes">
				{#each MODES as m (m.id)}
					<button
						class="mode"
						class:selected={status.mode === m.id}
						class:warn={status.mode === m.id && (m.id === 'auto_safe' || m.id === 'auto')}
						class:danger={m.id === 'dangerous'}
						class:danger-active={status.mode === 'dangerous' && m.id === 'dangerous'}
						onclick={() => pickMode(m.id)}
						disabled={busy || status.readOnly}
						aria-pressed={status.mode === m.id}
					>
						<span class="mode-name">{m.label()}</span>
						<span class="mode-hint">{m.hint()}</span>
					</button>
				{/each}
			</div>

			{#if confirmingDangerous}
				<div class="confirm" role="alertdialog" aria-labelledby="mcp-danger-msg">
					<p id="mcp-danger-msg" class="confirm-msg">{t('mcp.mode_dangerous_confirm')}</p>
					<div class="confirm-actions">
						<button class="mini" onclick={() => (confirmingDangerous = false)}>
							{t('mcp.confirm_reject')}
						</button>
						<button
							class="mini danger-solid"
							onclick={async () => {
								confirmingDangerous = false;
								await mcp.setMode('dangerous');
							}}
						>
							{t('mcp.mode_dangerous')}
						</button>
					</div>
				</div>
			{/if}

			{#if status.mode === 'dangerous'}
				<p class="danger-banner">{t('mcp.mode_dangerous_active')}</p>
			{/if}
			{#if status.readOnly}
				<p class="hint">{t('mcp.mode_moot')}</p>
			{/if}
		</section>

		<!-- 4 · What is exposed right now, and how to stop it. -->
		<section class="block">
			<div class="head">
				<span class="head-label">{t('mcp.shared')}</span>
				<span class="head-hint">{t('mcp.shared_desc')}</span>
			</div>
			{#if status.sessions.length === 0}
				<p class="empty">{t('mcp.shared_none')}</p>
			{:else}
				<ul class="shared">
					{#each status.sessions as sess (sess.sessionId)}
						<li class="shared-row">
							<!-- Two lines rather than one: the host and the two dropdowns
							     together do not fit the panel's width, and squeezing them
							     onto one row truncated the hostname to `root@10.14…` —
							     which defeats the point of showing it instead of a UUID. -->
							<div class="shared-head">
								<span class="dot exposing"></span>
								<span class="shared-name" title={sess.sessionId}>{label(sess)}</span>
								<button class="mini" onclick={() => mcp.unshare(sess.sessionId)}>
									{t('mcp.stop_sharing')}
								</button>
							</div>

							<div class="shared-controls">
							<!-- Per-session overrides, matching the chips in the bottom bar.
							     Having them only there meant configuring a session you were
							     not currently looking at was impossible. -->
							<select
								class="mini-select"
								class:overridden={!!sess.agentId}
								value={sess.agentId ?? ''}
								onchange={(e) =>
									mcp.setSessionAgent(sess.sessionId, e.currentTarget.value || null)}
							>
								<option value="">{t('mcp.follow_global')}</option>
								{#each agents as a (a.id)}
									<option value={a.id}>{a.name}</option>
								{/each}
							</select>

							<select
								class="mini-select"
								class:overridden={!!sess.mode}
								value={sess.mode ?? ''}
								onchange={(e) =>
									mcp.setSessionMode(
										sess.sessionId,
										(e.currentTarget.value || null) as McpMode | null
									)}
							>
								<option value="">{t('mcp.follow_global')}</option>
								{#each MODES as m (m.id)}
									<option value={m.id}>{m.label()}</option>
								{/each}
							</select>

							</div>
						</li>
					{/each}
				</ul>
			{/if}
		</section>

		<!-- 5 · The payoff. -->
		<section class="block">
			<div class="head">
				<span class="head-label">{t('mcp.connect')}</span>
				<span class="head-hint">{t('mcp.connect_desc')}</span>
			</div>

			<div class="clients">
				{#each MCP_CLIENTS as c (c.id)}
					<button
						class="client"
						class:selected={selectedClient === c.id}
						onclick={() => (selectedClient = c.id)}
						title={c.name}
					>
						<McpClientIcon slug={c.icon} size={18} />
						<span class="client-name">{c.name}</span>
					</button>
				{/each}
			</div>

			{#if client.path}
				<div class="kv">
					<span class="k">{t('mcp.config_path')}</span>
					<code class="v wrap">{client.path}</code>
				</div>
			{/if}

			<div class="snippet-wrap">
				<div class="snippet-bar">
					<span class="snippet-kind">{client.kind}</span>
					<button class="mini" onclick={() => copy(rendered, t('mcp.copied'))}>
						{t('mcp.copy')}
					</button>
				</div>
				<pre class="snippet">{rendered}</pre>
			</div>

			{#if client.note}<p class="hint">{client.note}</p>{/if}
			{#if !client.verified}<p class="warn-text">{t('mcp.unverified')}</p>{/if}
			<a class="docs" href={client.docs} target="_blank" rel="noreferrer noopener">
				{t('mcp.client_docs', { name: client.name })}
			</a>
		</section>
	{/if}
</div>

<style>
	.tab-content {
		display: flex;
		flex-direction: column;
		gap: var(--space-5);
	}

	.block {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}

	.row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-4);
	}

	.row-info {
		display: flex;
		flex-direction: column;
		gap: 2px;
		min-width: 0;
	}

	.row-label {
		font-size: var(--text-sm);
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.row-desc,
	.head-hint,
	.hint,
	.empty {
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
		margin: 0;
	}

	.hint,
	.empty {
		color: var(--color-text-tertiary);
	}

	.head {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.head-label {
		font-size: var(--text-2xs);
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.04em;
		color: var(--color-text-tertiary);
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

	/* Amber, not green: a session an AI can read is a state to notice, not a
	   success to celebrate. Same colour as the share eye on the tab. */
	.dot.exposing {
		background: var(--color-warning);
	}

	.live-text {
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
	}

	.port-note {
		margin-left: auto;
		font-size: var(--text-2xs);
		color: var(--color-text-tertiary);
	}

	.kv {
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}

	.k {
		flex-shrink: 0;
		width: 64px;
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

	.v.wrap {
		white-space: pre-wrap;
	}

	.mini {
		flex-shrink: 0;
		padding: 4px 9px;
		font-size: var(--text-2xs);
		font-family: inherit;
		color: var(--color-text-secondary);
		background: none;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-sm);
		cursor: pointer;
	}

	.mini:hover:not(:disabled) {
		color: var(--color-text-primary);
		background: var(--color-surface-hover);
	}

	.mini:disabled {
		opacity: 0.5;
		cursor: default;
	}

	.mini.danger:hover:not(:disabled) {
		color: var(--color-danger);
		border-color: var(--color-danger);
		background: none;
	}

	/* Dangerous is drawn as a hazard, not another option in the row: dashed
	   until chosen, solid red once active, with a standing banner underneath.
	   The person enabling it should not be able to forget they did. */
	.mode.danger {
		border-style: dashed;
		border-color: color-mix(in srgb, var(--color-danger) 55%, transparent);
	}

	.mode.danger .mode-name {
		color: var(--color-danger);
	}

	.mode.danger-active {
		border-style: solid;
		border-color: var(--color-danger);
		background: color-mix(in srgb, var(--color-danger) 12%, transparent);
	}

	.confirm {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		padding: var(--space-3);
		border: 1px solid var(--color-danger);
		border-radius: var(--radius-card);
		background: color-mix(in srgb, var(--color-danger) 10%, transparent);
	}

	.confirm-msg {
		margin: 0;
		font-size: var(--text-sm);
		color: var(--color-text-primary);
	}

	.confirm-actions {
		display: flex;
		justify-content: flex-end;
		gap: var(--space-2);
	}

	.mini.danger-solid {
		color: #fff;
		background: var(--color-danger);
		border-color: var(--color-danger);
	}

	.danger-banner {
		margin: 0;
		padding: var(--space-2) var(--space-3);
		font-size: var(--text-xs);
		font-weight: 600;
		color: var(--color-danger);
		border-left: 3px solid var(--color-danger);
		background: color-mix(in srgb, var(--color-danger) 10%, transparent);
		border-radius: var(--radius-sm);
	}

	.agents,
	.modes {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
		gap: var(--space-2);
	}

	.agent,
	.mode {
		display: flex;
		flex-direction: column;
		gap: 3px;
		padding: var(--space-2) var(--space-3);
		text-align: left;
		font-family: inherit;
		background: var(--color-bg-elevated);
		border: 2px solid var(--color-border);
		border-radius: var(--radius-card);
		cursor: pointer;
	}

	.agent:hover:not(:disabled),
	.mode:hover:not(:disabled) {
		background: var(--color-surface-hover);
	}

	.agent:disabled,
	.mode:disabled {
		opacity: 0.5;
		cursor: default;
	}

	.agent.selected,
	.mode.selected {
		border-color: var(--color-accent);
	}

	/* An active auto mode is bordered amber, so the panel says the same thing
	   the status bar does without needing to be read. */
	.mode.warn {
		border-color: var(--color-warning);
	}

	.agent-head {
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}

	.agent-name,
	.mode-name {
		font-size: var(--text-sm);
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.tag {
		padding: 1px 6px;
		font-size: 0.625rem;
		font-weight: 700;
		text-transform: uppercase;
		border-radius: var(--radius-sm);
	}

	.tag.ok {
		color: var(--color-success);
		border: 1px solid var(--color-success);
	}

	.agent-desc,
	.mode-hint {
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
	}

	.agent-tools {
		font-family: var(--font-mono);
		font-size: 0.625rem;
		color: var(--color-text-tertiary);
	}

	.shared-head {
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}

	.shared-controls {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		padding-left: 16px;
	}

	.shared-name {
		flex: 1;
		min-width: 0;
		font-family: var(--font-mono);
		font-size: var(--text-xs);
		color: var(--color-text-primary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.mini-select {
		flex: 1;
		min-width: 0;
		max-width: 220px;
		padding: 3px 6px;
		font-family: inherit;
		font-size: var(--text-2xs);
		color: var(--color-text-secondary);
		background: var(--color-surface-sunken);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-sm);
		cursor: pointer;
	}

	/* Dotted edge marks a session that differs from the global default, the
	   same signal the bottom-bar chips use. */
	.mini-select.overridden {
		border-style: dotted;
		border-color: var(--color-accent);
		color: var(--color-text-primary);
	}

	.confirm {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		padding: var(--space-3);
		border: 1px solid var(--color-danger);
		border-radius: var(--radius-card);
		background: color-mix(in srgb, var(--color-danger) 10%, transparent);
	}

	.confirm-msg {
		margin: 0;
		font-size: var(--text-sm);
		color: var(--color-text-primary);
	}

	.confirm-actions {
		display: flex;
		justify-content: flex-end;
		gap: var(--space-2);
	}

	.mini.danger-solid {
		color: #fff;
		background: var(--color-danger);
		border-color: var(--color-danger);
	}

	.shared {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.shared-row {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-2);
		background: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-sm);
	}

	.clients {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
	}

	.client {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 5px var(--space-2);
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

	/* The copy button sits in its own bar rather than floating over the code:
	   floated, it covered whatever scrolled underneath, on the one panel whose
	   entire job is handing over an exact string. */
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

	.warn-text {
		margin: 0;
		font-size: var(--text-xs);
		color: var(--color-warning);
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
