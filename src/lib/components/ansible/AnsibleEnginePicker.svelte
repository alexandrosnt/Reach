<!--
	Which engine runs Ansible, with every option's status beside it.

	Native, WSL, container and each open SSH connection are rows in one
	select. A row that cannot run says why in the chip next to it, so nobody
	has to know that Windows cannot be a control node — the picker knows.
	Choosing a remote host checks it once and remembers. When nothing here
	can run, the first open SSH session is picked; when there is none, the
	chip says what to open.
-->
<script lang="ts">
	import { onMount } from 'svelte';
	import { t } from '$lib/state/i18n.svelte';
	import {
		getEngines,
		loadEngines,
		getRemoteEngine,
		checkRemoteEngine
	} from '$lib/state/ansible.svelte';
	import type { AnsibleExecutionTarget } from '$lib/ipc/ansible';
	import { sshListConnections, type ConnectionInfo } from '$lib/ipc/ssh';

	interface Props {
		onchange: (target: AnsibleExecutionTarget | null) => void;
	}
	let { onchange }: Props = $props();

	let engines = $derived(getEngines());
	let connections = $state<ConnectionInfo[]>([]);
	let choice = $state('');

	async function refreshConnections() {
		try {
			connections = await sshListConnections();
		} catch {
			connections = [];
		}
	}

	onMount(async () => {
		await loadEngines();
		await refreshConnections();
		// Pick the first engine that works, in the order they were offered;
		// failing that, the first open SSH session.
		const first = engines.find((e) => e.available);
		if (first) {
			choice = first.kind;
		} else if (connections.length > 0) {
			choice = `ssh:${connections[0].id}`;
			await checkRemoteEngine(connections[0].id);
		}
		onchange(choice ? toTarget(choice) : null);
	});

	let nothingRuns = $derived(!engines.some((e) => e.available) && connections.length === 0);

	function toTarget(c: string): AnsibleExecutionTarget {
		if (c === 'native') return { type: 'local' };
		if (c === 'wsl') return { type: 'wsl' };
		if (c === 'container') return { type: 'container' };
		if (c.startsWith('ssh:')) return { type: 'ssh', connectionId: c.slice(4) };
		return { type: 'local' };
	}

	async function onSelect(e: Event) {
		choice = (e.currentTarget as HTMLSelectElement).value;
		if (choice.startsWith('ssh:')) await checkRemoteEngine(choice.slice(4));
		onchange(toTarget(choice));
	}

	let current = $derived.by(() => {
		if (choice.startsWith('ssh:')) return getRemoteEngine(choice.slice(4));
		return engines.find((e) => e.kind === choice) ?? null;
	});

	function engineLabel(kind: string): string {
		if (kind === 'native') return t('ansible.engine_native');
		if (kind === 'wsl') return t('ansible.engine_wsl');
		if (kind === 'container') return t('ansible.engine_container');
		return t('ansible.engine_remote');
	}
</script>

<div class="picker">
	<select class="target-select" value={choice} onchange={onSelect} onfocus={refreshConnections}>
		{#if !choice}
			<option value="" disabled>{t('ansible.engine_choose')}</option>
		{/if}
		{#each engines as e (e.kind)}
			<option value={e.kind} disabled={!e.available}>
				{engineLabel(e.kind)}{e.detail && e.kind === 'wsl' ? ` · ${e.detail}` : ''}{e.detail && e.kind === 'container' ? ` · ${e.detail.split(' · ')[0]}` : ''}{e.available ? '' : ` — ${t('ansible.engine_unavailable')}`}
			</option>
		{/each}
		{#each connections as c (c.id)}
			<option value={`ssh:${c.id}`}>{t('ansible.engine_remote')} · {c.username}@{c.host}</option>
		{/each}
	</select>

	{#if current}
		<span class="chip" class:ok={current.available} class:bad={!current.available} title={current.reason ?? current.detail ?? ''}>
			<span class="dot"></span>
			{#if current.available}
				<span class="mono">{current.version ?? '—'}</span>
				{#if current.unofficial}<span class="dim">· {t('ansible.engine_unofficial')}</span>{/if}
				{#if current.kind === 'container' && current.detail}<span class="dim">· {current.detail.split(' · ')[1] ?? ''}</span>{/if}
			{:else}
				<span>{current.reason ?? t('ansible.engine_unavailable')}</span>
			{/if}
		</span>
	{:else if choice.startsWith('ssh:')}
		<span class="chip"><span class="spinner"></span>{t('ansible.engine_checking')}</span>
	{:else if nothingRuns}
		<span class="chip bad hint"><span class="dot"></span><span>{t('ansible.engine_none_hint')}</span></span>
	{/if}
</div>

<style>
	.picker {
		display: flex;
		align-items: center;
		gap: 10px;
		min-width: 0;
		flex-wrap: wrap;
	}

	.target-select {
		padding: 5px 10px;
		font-size: 0.8125rem;
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		font-family: inherit;
		max-width: 320px;
	}

	.chip {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		padding: 3px 9px;
		border: 1px solid var(--color-border);
		border-radius: 10px;
		font-size: 0.75rem;
		color: var(--color-text-secondary);
		max-width: 480px;
		min-width: 0;
	}

	.chip span:not(.dot):not(.spinner) {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	/* The hint is a sentence, not a status: let it wrap. */
	.chip.hint span:not(.dot) {
		white-space: normal;
		line-height: 1.4;
	}

	.dot {
		width: 7px;
		height: 7px;
		border-radius: 50%;
		flex-shrink: 0;
		background: var(--color-text-tertiary);
	}

	.chip.ok .dot {
		background: var(--color-success, #10b981);
	}

	.chip.bad .dot {
		background: var(--color-warning, #f59e0b);
	}

	.chip.bad {
		border-color: color-mix(in srgb, var(--color-warning, #f59e0b) 45%, var(--color-border));
	}

	.mono {
		font-family: var(--font-mono, monospace);
		color: var(--color-text-primary);
	}

	.dim {
		color: var(--color-text-tertiary);
	}

	.spinner {
		width: 10px;
		height: 10px;
		border: 2px solid color-mix(in srgb, var(--color-accent) 30%, transparent);
		border-top-color: var(--color-accent);
		border-radius: 50%;
		animation: spin 0.8s linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}
</style>
