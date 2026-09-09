<script lang="ts">
	/**
	 * Per-session MCP controls, sitting beside the CPU/RAM gauges.
	 *
	 * This is the right home for them because it is already scoped to the tab in
	 * front of you. Production and a dev box on the same machine should not
	 * inherit the same trust, and choosing that from a global settings panel
	 * means holding "which session am I configuring" in your head. Here the
	 * answer is whatever you are looking at.
	 *
	 * Renders nothing at all unless the MCP server is running *and* this session
	 * is shared — an inert control for a feature that is switched off is worse
	 * than no control.
	 *
	 * Both dropdowns offer a "follow global" option, so a per-session override
	 * is something you opt into rather than a value you are forced to pin.
	 */
	import * as mcp from '$lib/state/mcp.svelte';
	import type { McpMode } from '$lib/ipc/mcp';
	import { t } from '$lib/state/i18n.svelte';

	let { sessionId }: { sessionId: string | undefined } = $props();

	let status = $derived(mcp.getMcpStatus());
	let agents = $derived(mcp.getAgents());
	let shared = $derived(!!sessionId && status.enabled && mcp.isShared(sessionId));
	let settings = $derived(status.sessions?.find((s) => s.sessionId === sessionId));

	/** What actually applies here: the override if set, otherwise the global. */
	let effectiveMode = $derived<McpMode>(settings?.mode ?? status.mode);
	let effectiveAgentId = $derived(settings?.agentId ?? status.agentId);
	let effectiveAgent = $derived(agents.find((a) => a.id === effectiveAgentId));

	const MODE_LABEL: Record<McpMode, () => string> = {
		ask: () => t('mcp.mode_ask'),
		auto_safe: () => t('mcp.mode_auto_safe'),
		auto: () => t('mcp.mode_auto'),
		dangerous: () => t('mcp.mode_dangerous')
	};

	let openMenu = $state<'agent' | 'mode' | null>(null);
	let rootEl: HTMLDivElement | undefined = $state();

	async function pickAgent(id: string | null): Promise<void> {
		openMenu = null;
		if (sessionId) await mcp.setSessionAgent(sessionId, id);
	}

	async function pickMode(m: McpMode | null): Promise<void> {
		openMenu = null;
		if (sessionId) await mcp.setSessionMode(sessionId, m);
	}

	$effect(() => {
		if (!openMenu) return;
		function outside(e: MouseEvent) {
			if (rootEl && !rootEl.contains(e.target as Node)) openMenu = null;
		}
		document.addEventListener('click', outside, true);
		return () => document.removeEventListener('click', outside, true);
	});
</script>

{#if shared}
	<div class="mcp-bar" bind:this={rootEl}>
		<!-- Amber eye, matching the tab and the settings dot: this session is
		     readable by an AI right now. -->
		<span class="eye" title={t('mcp.exposing', { count: 1 })}>
			<svg width="13" height="13" viewBox="0 0 24 24" fill="none" aria-hidden="true">
				<path d="M2 12s3.6-6.5 10-6.5S22 12 22 12s-3.6 6.5-10 6.5S2 12 2 12z" fill="currentColor" opacity="0.9" />
				<circle cx="12" cy="12" r="2.6" fill="var(--color-bg-secondary)" />
			</svg>
		</span>

		<div class="pick">
			<button
				class="chip"
				class:overridden={!!settings?.agentId}
				onclick={() => (openMenu = openMenu === 'agent' ? null : 'agent')}
				title={t('mcp.agent')}
			>
				{effectiveAgent?.name ?? status.agentName}
				{#if effectiveAgent?.readOnly}<span class="ro">·</span>{/if}
			</button>
			{#if openMenu === 'agent'}
				<ul class="menu" role="listbox">
					<li>
						<button class="opt" class:sel={!settings?.agentId} onclick={() => pickAgent(null)}>
							{t('mcp.follow_global')}
						</button>
					</li>
					{#each agents as a (a.id)}
						<li>
							<button class="opt" class:sel={settings?.agentId === a.id} onclick={() => pickAgent(a.id)}>
								{a.name}{a.readOnly ? ' · ' + t('mcp.read_only') : ''}
							</button>
						</li>
					{/each}
				</ul>
			{/if}
		</div>

		<div class="pick">
			<button
				class="chip"
				class:overridden={!!settings?.mode}
				class:warn={effectiveMode === 'auto_safe' || effectiveMode === 'auto'}
				class:danger={effectiveMode === 'dangerous'}
				onclick={() => (openMenu = openMenu === 'mode' ? null : 'mode')}
				title={t('mcp.mode')}
			>
				{MODE_LABEL[effectiveMode]()}
			</button>
			{#if openMenu === 'mode'}
				<ul class="menu" role="listbox">
					<li>
						<button class="opt" class:sel={!settings?.mode} onclick={() => pickMode(null)}>
							{t('mcp.follow_global')}
						</button>
					</li>
					{#each ['ask', 'auto_safe', 'auto', 'dangerous'] as m (m)}
						<li>
							<button
								class="opt"
								class:sel={settings?.mode === m}
								class:danger-opt={m === 'dangerous'}
								onclick={() => pickMode(m as McpMode)}
							>
								{MODE_LABEL[m as McpMode]()}
							</button>
						</li>
					{/each}
				</ul>
			{/if}
		</div>
	</div>
{/if}

<style>
	.mcp-bar {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		padding-left: var(--space-3);
		margin-left: var(--space-2);
		border-left: 1px solid var(--color-border);
	}

	.eye {
		display: flex;
		color: var(--color-warning);
		flex-shrink: 0;
	}

	.pick {
		position: relative;
	}

	.chip {
		padding: 2px 8px;
		font-family: inherit;
		font-size: var(--text-2xs);
		font-weight: 600;
		color: var(--color-text-secondary);
		background: none;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-sm);
		cursor: pointer;
		white-space: nowrap;
	}

	.chip:hover {
		color: var(--color-text-primary);
		background: var(--color-surface-hover);
	}

	/* A dotted edge means "this session differs from the global default", so a
	   deviation is visible without opening anything. */
	.chip.overridden {
		border-style: dotted;
		border-color: var(--color-accent);
	}

	.chip.warn {
		color: var(--color-warning);
	}

	.chip.danger {
		color: var(--color-danger);
		border-color: var(--color-danger);
	}

	.ro {
		color: var(--color-success);
	}

	.menu {
		position: absolute;
		z-index: 40;
		bottom: calc(100% + 6px);
		left: 0;
		min-width: 190px;
		margin: 0;
		padding: var(--space-1);
		list-style: none;
		background: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		box-shadow: var(--shadow-elevated);
	}

	.opt {
		width: 100%;
		padding: 5px var(--space-2);
		text-align: left;
		font-family: inherit;
		font-size: var(--text-xs);
		color: var(--color-text-primary);
		background: none;
		border: none;
		border-radius: var(--radius-sm);
		cursor: pointer;
		white-space: nowrap;
	}

	.opt:hover {
		background: var(--color-surface-hover);
	}

	.opt.sel {
		color: var(--color-accent);
		font-weight: 600;
	}

	.opt.danger-opt {
		color: var(--color-danger);
	}
</style>
