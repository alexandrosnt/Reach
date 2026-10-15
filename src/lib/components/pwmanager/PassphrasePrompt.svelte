<script lang="ts">
	/**
	 * The password manager's passphrase, asked for when a sign-in needs it
	 * (Settings → Password managers → Passphrase: Ask when needed). Mounted
	 * once, in the app shell.
	 *
	 * The backend asks one question per account at a time, however many
	 * connects are waiting for it, and withdraws it when nobody is waiting any
	 * more. What is typed is sent once, to the backend, and the field is
	 * emptied straight away; the backend never sends it back.
	 */
	import { onMount, onDestroy } from 'svelte';
	import { listen, type UnlistenFn } from '@tauri-apps/api/event';
	import Modal from '$lib/components/shared/Modal.svelte';
	import Button from '$lib/components/shared/Button.svelte';
	import { t } from '$lib/state/i18n.svelte';
	import { rememberLabel } from '$lib/state/pwmanager.svelte';
	import {
		PASSPHRASE_PROMPT_CLOSED_EVENT,
		PASSPHRASE_PROMPT_EVENT,
		pwmanagerPassphraseResponse,
		type PassphrasePrompt
	} from '$lib/ipc/pwmanager';

	let queue = $state<PassphrasePrompt[]>([]);
	let current = $derived(queue[0]);
	let value = $state('');
	let sending = $state(false);
	let sendError = $state<string | null>(null);

	function drop(id: string): void {
		queue = queue.filter((p) => p.promptId !== id);
		if (queue.length === 0) {
			value = '';
			sendError = null;
		}
	}

	let unlisten: UnlistenFn | undefined;
	let unlistenClosed: UnlistenFn | undefined;
	onMount(async () => {
		unlisten = await listen<PassphrasePrompt>(PASSPHRASE_PROMPT_EVENT, (e) => {
			queue = [...queue, e.payload];
		});
		unlistenClosed = await listen<string>(PASSPHRASE_PROMPT_CLOSED_EVENT, (e) => drop(e.payload));
	});
	onDestroy(() => {
		unlisten?.();
		unlistenClosed?.();
		value = '';
	});

	async function respond(ok: boolean): Promise<void> {
		const p = current;
		if (!p || sending) return;
		if (ok && value === '') return;
		const typed = ok ? value : null;
		// Out of the field at once, whatever the answer does.
		value = '';
		sending = true;
		sendError = null;
		try {
			await pwmanagerPassphraseResponse(p.promptId, typed);
			drop(p.promptId);
		} catch (e) {
			// Refused (too long, say): the question is still open; say why.
			// A cancel that fails means the question is gone anyway.
			if (ok) sendError = String(e);
			else drop(p.promptId);
		} finally {
			sending = false;
		}
	}

	let reasonText = $derived.by(() => {
		const p = current;
		if (!p) return '';
		if (p.reason === 'connect' && p.target) return t('pwmanager.prompt_reason_connect', { target: p.target });
		if (p.reason === 'list') return t('pwmanager.prompt_reason_list');
		if (p.reason === 'test') return t('pwmanager.prompt_reason_test');
		return t('pwmanager.prompt_reason_other', { name: p.providerName });
	});

	let keptText = $derived(
		current
			? current.rememberSecs > 0
				? t('pwmanager.prompt_kept', { duration: rememberLabel(current.rememberSecs) })
				: t('pwmanager.prompt_not_kept')
			: ''
	);
</script>

{#if current}
	<Modal
		open={true}
		onclose={() => respond(false)}
		zIndex={1100}
		title={t('pwmanager.prompt_title', { name: current.providerName })}
		maxWidth="440px"
	>
		{#key current.promptId}
			<form class="form" onsubmit={(e) => { e.preventDefault(); void respond(true); }}>
				<p class="msg strong">{reasonText}</p>
				{#if current.retry}
					<p class="msg error" role="alert">
						{t('pwmanager.prompt_retry')}
						{#if current.error}<span class="detail">{current.error}</span>{/if}
					</p>
				{/if}
				<label class="field">
					<span class="label">{t('pwmanager.prompt_label')}</span>
					<!-- svelte-ignore a11y_autofocus -->
					<input
						class="input"
						type="password"
						bind:value
						autocomplete="off"
						autocapitalize="off"
						spellcheck="false"
						autofocus
						disabled={sending}
					/>
				</label>
				{#if sendError}
					<p class="msg error" role="alert">{sendError}</p>
				{/if}
				<p class="hint">{keptText} {t('pwmanager.prompt_hint')}</p>
			</form>
		{/key}

		{#snippet actions()}
			<Button variant="secondary" onclick={() => respond(false)} disabled={sending}>{t('common.cancel')}</Button>
			<Button variant="primary" onclick={() => respond(true)} disabled={sending || value === ''}>
				{t('pwmanager.prompt_submit')}
			</Button>
		{/snippet}
	</Modal>
{/if}

<style>
	.form {
		display: flex;
		flex-direction: column;
		gap: 10px;
	}

	.msg {
		margin: 0;
		font-size: 0.8125rem;
		color: var(--color-text-secondary);
		overflow-wrap: anywhere;
	}

	.msg.strong {
		color: var(--color-text-primary);
		font-weight: 600;
	}

	.msg.error {
		display: flex;
		flex-direction: column;
		gap: 2px;
		color: var(--color-danger);
	}

	.detail {
		font-size: 0.75rem;
		opacity: 0.85;
	}

	.field {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.label {
		font-size: 0.75rem;
		color: var(--color-text-secondary);
	}

	.input {
		min-height: 34px;
		padding: 6px 10px;
		font: inherit;
		color: var(--color-text-primary);
		background: var(--color-bg-primary);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
	}

	.input:focus {
		outline: none;
		border-color: var(--color-accent);
	}

	.hint {
		margin: 0;
		font-size: 0.6875rem;
		line-height: 1.45;
		color: var(--color-text-tertiary, var(--color-text-secondary));
	}

	@media (max-width: 700px) {
		.input {
			min-height: 44px;
		}
	}
</style>
