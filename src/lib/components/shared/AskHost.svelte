<script lang="ts">
	import Modal from '$lib/components/shared/Modal.svelte';
	import Button from '$lib/components/shared/Button.svelte';
	import { currentQuestion, answer } from '$lib/state/ask.svelte';
	import { t } from '$lib/state/i18n.svelte';
	import { tick } from 'svelte';

	let question = $derived(currentQuestion());
	let buttons = $state<HTMLElement>();

	// Focus goes to Cancel, as it went into the browser's own box: Enter and
	// Escape then answer this question, not whatever was focused behind it,
	// and a stray Enter cancels rather than deletes.
	$effect(() => {
		if (!question) return;
		void tick().then(() => buttons?.querySelector('button')?.focus());
	});
</script>

<!-- Above any dialog the question comes from. Every caller asks before
     deleting or discarding something, so the confirming button is red. -->
<Modal open={question !== null} onclose={() => answer(false)} title={t('common.confirm')} maxWidth="420px" zIndex={1100}>
	<p class="message">{question?.message}</p>
	{#snippet actions()}
		<span class="buttons" bind:this={buttons}>
			<Button variant="secondary" size="sm" onclick={() => answer(false)}>{t('common.cancel')}</Button>
			<Button variant="danger" size="sm" onclick={() => answer(true)}>{question?.options.confirmLabel ?? t('common.confirm')}</Button>
		</span>
	{/snippet}
</Modal>

<style>
	/* A handle for focus only: the buttons lay out as the dialog's own. */
	.buttons {
		display: contents;
	}

	.message {
		margin: 0;
		font-size: var(--text-sm);
		line-height: 1.5;
		color: var(--color-text-primary);
		/* Questions put the reason and the ask on lines of their own. */
		white-space: pre-line;
		overflow-wrap: anywhere;
	}
</style>
