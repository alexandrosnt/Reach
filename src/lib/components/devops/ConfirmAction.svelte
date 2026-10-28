<script lang="ts">
	/**
	 * Confirmation for a change that cannot be taken back.
	 *
	 * It names what is affected, item by item, and where (host or cluster,
	 * with its environment), because acting on the wrong context is how the
	 * well-known outages happened. On production, or when `typeToConfirm` is
	 * set, the user types the name before the button works.
	 */
	import Modal from '#lib/components/shared/Modal.svelte';
	import Button from '#lib/components/shared/Button.svelte';
	import EnvBadge from '#lib/components/devops/EnvBadge.svelte';
	import type { Environment } from '#lib/ipc/containers.js';
	import { t } from '#lib/state/i18n.svelte.js';

	interface Props {
		open: boolean;
		title: string;
		/** One sentence on what happens. */
		message: string;
		/** Each thing affected, by name. */
		items: string[];
		/** Where: the host or cluster name. */
		where: string;
		environment: Environment;
		/** What happens to data, or how to undo; shown under the list. */
		note?: string;
		confirmLabel: string;
		/** Require typing this. Unset: the single item (or the host) on
		 *  production, nothing elsewhere. An empty string: never. */
		typeToConfirm?: string;
		onconfirm: () => void;
		onclose: () => void;
	}

	let { open, title, message, items, where, environment, note = '', confirmLabel, typeToConfirm, onconfirm, onclose }: Props = $props();

	let typed = $state('');
	let mustType = $derived(typeToConfirm ?? (environment === 'production' ? (items.length === 1 ? items[0] : where) : ''));
	let ready = $derived(!mustType || typed.trim() === mustType);

	$effect(() => {
		if (open) typed = '';
	});

	function go(): void {
		if (!ready) return;
		onconfirm();
		onclose();
	}
</script>

<Modal {open} {onclose} {title} maxWidth="460px">
	<div class="body">
		<div class="where">
			<span>{t('confirm.on')}</span>
			<strong>{where}</strong>
			<EnvBadge {environment} />
		</div>
		<p class="message">{message}</p>
		<ul class="items">
			{#each items.slice(0, 12) as item (item)}
				<li>{item}</li>
			{/each}
			{#if items.length > 12}
				<li class="more">{t('confirm.and_more', { count: items.length - 12 })}</li>
			{/if}
		</ul>
		{#if note}
			<p class="note">{note}</p>
		{/if}
		{#if mustType}
			<label class="type">
				<span>{t('confirm.type_to_confirm', { name: mustType })}</span>
				<!-- svelte-ignore a11y_autofocus -->
				<input
					autofocus
					spellcheck="false"
					autocomplete="off"
					autocapitalize="off"
					bind:value={typed}
					onkeydown={(e) => e.key === 'Enter' && go()}
				/>
			</label>
		{/if}
	</div>
	{#snippet actions()}
		<Button variant="ghost" onclick={onclose}>{t('common.cancel')}</Button>
		<Button variant="danger" disabled={!ready} onclick={go}>{confirmLabel}</Button>
	{/snippet}
</Modal>

<style>
	.body {
		display: flex;
		flex-direction: column;
		gap: 10px;
		font-size: var(--text-sm);
		color: var(--color-text-primary);
	}

	.where {
		display: flex;
		align-items: center;
		gap: 6px;
		flex-wrap: wrap;
		color: var(--color-text-secondary);
	}

	.where strong {
		color: var(--color-text-primary);
	}

	.message {
		margin: 0;
		line-height: 1.45;
	}

	.items {
		margin: 0;
		padding: 8px 10px 8px 26px;
		list-style: disc;
		max-height: 180px;
		overflow: auto;
		border-radius: var(--radius-sm);
		background: var(--color-bg-primary);
		font-family: var(--font-mono);
		font-size: var(--text-xs);
		line-height: 1.6;
	}

	.more {
		list-style: none;
		margin-left: -16px;
		color: var(--color-text-tertiary);
		font-family: var(--font-sans);
	}

	.note {
		margin: 0;
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
		line-height: 1.45;
	}

	.type {
		display: flex;
		flex-direction: column;
		gap: 6px;
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
	}

	.type input {
		padding: 8px 10px;
		border-radius: var(--radius-btn);
		border: 1px solid var(--color-border);
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		font-family: var(--font-mono);
		font-size: var(--text-sm);
	}

	.type input:focus {
		outline: none;
		border-color: var(--color-danger);
	}

</style>
