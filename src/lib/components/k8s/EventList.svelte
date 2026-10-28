<script lang="ts">
	/**
	 * Cluster events as short cards: what happened (the reason and message),
	 * to what, how often and when. Warnings lead with their shape and word.
	 */
	import StatusPill from '#lib/components/devops/StatusPill.svelte';
	import type { EventRow } from '#lib/ipc/k8s.js';
	import { ago } from '#lib/devops/status.js';
	import { t } from '#lib/state/i18n.svelte.js';
	import { eventStatus } from './model';

	interface Props {
		events: EventRow[];
		/** Hide the object column where every event is about the same one. */
		showObject?: boolean;
		showNamespace?: boolean;
		/** Open the object an event is about, when it can be shown. */
		onopen?: (e: EventRow) => void;
	}

	let { events, showObject = true, showNamespace = false, onopen }: Props = $props();
</script>

<ul class="events">
	{#each events as e, i (i)}
		<li class:warning={e.kind === 'Warning'}>
			<div class="head">
				<StatusPill status={eventStatus(e)} compact />
				<strong>{e.reason}</strong>
				{#if showObject}
					{#if onopen}
						<button type="button" class="obj" onclick={() => onopen(e)}>{e.object}</button>
					{:else}
						<span class="obj">{e.object}</span>
					{/if}
				{/if}
				{#if showNamespace && e.namespace}
					<span class="ns">{e.namespace}</span>
				{/if}
				<span class="spacer"></span>
				{#if e.count > 1}
					<span class="count" title={t('k8s.event_count_hint')}>×{e.count}</span>
				{/if}
				<span class="when">{ago(e.last)}</span>
			</div>
			<p class="msg">{e.message}</p>
		</li>
	{/each}
</ul>

<style>
	.events {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	li {
		padding: 8px 10px;
		border-radius: var(--radius-sm);
		border: 1px solid var(--color-border);
		border-left: 3px solid var(--color-border);
		background: var(--color-bg-elevated);
		font-size: var(--text-sm);
	}

	li.warning {
		border-left-color: var(--color-warning);
	}

	.head {
		display: flex;
		align-items: center;
		gap: 8px;
		flex-wrap: wrap;
		min-width: 0;
	}

	strong {
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.obj {
		font-family: var(--font-mono);
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
		overflow-wrap: anywhere;
	}

	button.obj {
		padding: 0;
		border: none;
		background: none;
		color: var(--color-accent);
		cursor: pointer;
		text-align: left;
	}

	button.obj:focus-visible {
		outline: 2px solid var(--color-accent);
	}

	.ns {
		font-size: var(--text-xs);
		color: var(--color-text-tertiary);
	}

	.spacer {
		flex: 1;
	}

	.count,
	.when {
		font-size: var(--text-xs);
		color: var(--color-text-tertiary);
		white-space: nowrap;
	}

	.msg {
		margin: 4px 0 0;
		line-height: 1.45;
		color: var(--color-text-secondary);
		overflow-wrap: anywhere;
	}
</style>
