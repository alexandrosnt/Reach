<script lang="ts">
	/**
	 * The top of every detail pane: what it is and where, its state, and its
	 * actions. On a phone the pane is full screen, so it leads with Back.
	 */
	import type { Snippet } from 'svelte';
	import FaIcon from '#lib/components/shared/FaIcon.svelte';
	import { faArrowLeft, faXmark } from '@fortawesome/free-solid-svg-icons';
	import StatusPill from '#lib/components/devops/StatusPill.svelte';
	import type { Status } from '#lib/devops/status.js';
	import { t } from '#lib/state/i18n.svelte.js';

	interface Props {
		kind: string;
		name: string;
		namespace?: string;
		status?: Status | null;
		narrow: boolean;
		readOnly: boolean;
		onclose: () => void;
		actions?: Snippet;
	}

	let { kind, name, namespace = '', status = null, narrow, readOnly, onclose, actions }: Props = $props();
</script>

<header class="head">
	<div class="title-row">
		{#if narrow}
			<button type="button" class="icon" onclick={onclose} aria-label={t('k8s.back')}><FaIcon icon={faArrowLeft} /></button>
		{/if}
		<div class="title">
			<span class="kind">{kind}{namespace ? ` · ${namespace}` : ''}</span>
			<h2 title={name}>{name}</h2>
		</div>
		{#if status}
			<StatusPill {status} />
		{/if}
		{#if !narrow}
			<button type="button" class="icon" onclick={onclose} aria-label={t('k8s.close_detail')}><FaIcon icon={faXmark} /></button>
		{/if}
	</div>
	{#if actions}
		<div class="actions">
			{@render actions()}
		</div>
		{#if readOnly}
			<p class="ro">{t('k8s.read_only_note')}</p>
		{/if}
	{/if}
</header>

<style>
	.head {
		display: flex;
		flex-direction: column;
		gap: 8px;
		padding: 10px 12px;
		border-bottom: 1px solid var(--color-border);
		flex-shrink: 0;
	}

	.title-row {
		display: flex;
		align-items: center;
		gap: 10px;
		min-width: 0;
	}

	.title {
		display: flex;
		flex-direction: column;
		min-width: 0;
		flex: 1;
	}

	.kind {
		font-size: var(--text-xs);
		color: var(--color-text-tertiary);
	}

	h2 {
		margin: 0;
		font-size: 15px;
		font-weight: 600;
		font-family: var(--font-mono);
		color: var(--color-text-primary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.icon {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 32px;
		height: 32px;
		flex-shrink: 0;
		border: none;
		border-radius: var(--radius-btn);
		background: none;
		color: var(--color-text-secondary);
		cursor: pointer;
	}

	.icon:hover {
		background: var(--color-surface-hover);
		color: var(--color-text-primary);
	}

	.icon:focus-visible {
		outline: 2px solid var(--color-accent);
	}

	.actions {
		display: flex;
		gap: 6px;
		flex-wrap: wrap;
		align-items: center;
	}

	.ro {
		margin: 0;
		font-size: var(--text-xs);
		color: var(--color-text-tertiary);
	}

	@media (pointer: coarse) {
		.icon {
			width: 44px;
			height: 44px;
		}
	}
</style>
