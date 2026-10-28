<script lang="ts">
	/**
	 * A question a login asks while it runs: a password, a key's passphrase,
	 * the server's keyboard-interactive questions (one-time codes and the
	 * like), or a yes/no. One at a time, oldest first; withdrawn when the
	 * login no longer needs it.
	 */
	import { onMount, onDestroy } from 'svelte';
	import { listen, type UnlistenFn } from '@tauri-apps/api/event';
	import { invoke } from '@tauri-apps/api/core';
	import Modal from '#lib/components/shared/Modal.svelte';
	import Button from '#lib/components/shared/Button.svelte';
	import { t } from '#lib/state/i18n.svelte.js';
	import { addToast, dismissToast } from '#lib/state/toasts.svelte.js';

	interface AuthPrompt {
		promptId: string;
		host: string;
		port: number;
		kind: 'password' | 'passphrase' | 'keyboard' | 'confirm';
		title: string;
		instructions: string;
		fields: { text: string; echo: boolean }[];
	}

	let queue = $state<AuthPrompt[]>([]);
	let current = $derived(queue[0]);
	let answers = $state<string[]>([]);

	$effect(() => {
		answers = current ? current.fields.map(() => '') : [];
	});

	/** A notice with no answer while a login waits ("Confirm user presence for key …"). */
	interface AuthNotice {
		host: string;
		port: number;
		text: string | null;
		done: string | null;
	}
	let noticeId: string | undefined;

	let unlisten: UnlistenFn | undefined;
	let unlistenClosed: UnlistenFn | undefined;
	let unlistenNotice: UnlistenFn | undefined;
	onMount(async () => {
		unlistenNotice = await listen<AuthNotice>('ssh-auth-notice', (e) => {
			if (noticeId) dismissToast(noticeId);
			noticeId = undefined;
			if (e.payload.text) {
				noticeId = addToast(`${e.payload.host}: ${e.payload.text}`, 'info', 0).id;
			} else if (e.payload.done) {
				addToast(e.payload.done, 'success', 2000);
			}
		});
		unlisten = await listen<AuthPrompt>('ssh-auth-prompt', (e) => {
			queue = [...queue, e.payload];
		});
		unlistenClosed = await listen<string>('ssh-auth-prompt-closed', (e) => {
			queue = queue.filter((p) => p.promptId !== e.payload);
		});
	});
	onDestroy(() => {
		unlisten?.();
		unlistenClosed?.();
		unlistenNotice?.();
	});

	async function respond(ok: boolean): Promise<void> {
		const p = current;
		if (!p) return;
		const given = ok ? [...answers] : null;
		queue = queue.slice(1);
		answers = [];
		try {
			await invoke('ssh_auth_prompt_response', { promptId: p.promptId, answers: given });
		} catch (err) {
			console.error('Login prompt response failed:', err);
		}
	}

	let heading = $derived(current ? t(`authprompt.title_${current.kind}`, { host: current.host }) : '');
	// The backend's own sentence ("Enter passphrase for key …") under the heading.
	let detail = $derived(current && current.kind !== 'keyboard' ? current.title : '');
</script>

{#if current}
	<Modal open={true} onclose={() => respond(false)} zIndex={1000} title={heading} maxWidth="480px">
		<form class="form" onsubmit={(e) => { e.preventDefault(); respond(true); }}>
			{#if current.kind === 'keyboard' && current.title}
				<p class="msg strong">{current.title}</p>
			{/if}
			{#if detail}
				<p class="msg">{detail}</p>
			{/if}
			{#if current.instructions}
				<p class="msg pre">{current.instructions}</p>
			{/if}
			{#if current.kind !== 'confirm'}
				{#each current.fields as f, i (i)}
					<label class="field">
						<span class="label">{current.kind === 'keyboard' ? f.text : t(`authprompt.field_${current.kind}`)}</span>
						<!-- svelte-ignore a11y_autofocus -->
						<input
							class="input"
							type={f.echo ? 'text' : 'password'}
							bind:value={answers[i]}
							autocomplete="off"
							autocapitalize="off"
							spellcheck="false"
							autofocus={i === 0}
						/>
					</label>
				{/each}
			{/if}
			<p class="hint">{t('authprompt.host_hint', { host: `${current.host}:${current.port}` })}</p>
		</form>

		{#snippet actions()}
			<Button variant="secondary" onclick={() => respond(false)}>
				{current.kind === 'confirm' ? t('authprompt.no') : t('common.cancel')}
			</Button>
			<Button variant="primary" onclick={() => respond(true)}>
				{current.kind === 'confirm' ? t('authprompt.yes') : t('authprompt.continue')}
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

	.msg.pre {
		white-space: pre-wrap;
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
		color: var(--color-text-tertiary, var(--color-text-secondary));
	}

	@media (max-width: 700px) {
		.input {
			min-height: 44px;
		}
	}
</style>
