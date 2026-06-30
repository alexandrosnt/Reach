<script lang="ts">
	import Modal from '$lib/components/shared/Modal.svelte';
	import Button from '$lib/components/shared/Button.svelte';
	import { t } from '$lib/state/i18n.svelte';

	interface Props {
		open: boolean;
		/** 'close' = window is closing; 'update' = an update wants to relaunch. */
		variant: 'close' | 'update';
		/** Number of active SSH connections that would be terminated. */
		count: number;
		/** Close & disconnect (close) / Update & restart (update). */
		onconfirm: () => void;
		/** Cancel/dismiss (close) or postpone (update — also fired on backdrop close). */
		oncancel: () => void;
	}

	let { open, variant, count, onconfirm, oncancel }: Props = $props();

	let title = $derived(
		variant === 'update' ? t('updater.relaunch_title') : t('session.close_active_title')
	);
	let message = $derived(
		variant === 'update'
			? t('updater.relaunch_message', { count: String(count) })
			: t('session.close_active_message', { count: String(count) })
	);
</script>

<Modal {open} onclose={oncancel} {title} maxWidth="440px">
	<p class="msg">{message}</p>

	{#snippet actions()}
		{#if variant === 'update'}
			<Button variant="secondary" onclick={oncancel}>{t('updater.postpone')}</Button>
			<Button variant="primary" onclick={onconfirm}>{t('updater.update_and_restart')}</Button>
		{:else}
			<Button variant="secondary" onclick={oncancel}>{t('common.cancel')}</Button>
			<Button variant="danger" onclick={onconfirm}>{t('session.close_anyway')}</Button>
		{/if}
	{/snippet}
</Modal>

<style>
	.msg {
		margin: 0;
		font-size: 0.875rem;
		line-height: 1.5;
		color: var(--color-text-primary);
	}
</style>
