<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { listen, type UnlistenFn } from '@tauri-apps/api/event';
	import Modal from '$lib/components/shared/Modal.svelte';
	import Button from '$lib/components/shared/Button.svelte';
	import { sshHostkeyResponse, type HostKeyPrompt } from '$lib/ipc/ssh';
	import { t } from '$lib/state/i18n.svelte';

	// Prompts queue up (e.g. a jump-host chain can produce several); show one at
	// a time, oldest first.
	let queue = $state<HostKeyPrompt[]>([]);
	let current = $derived(queue[0]);

	let unlisten: UnlistenFn | undefined;
	onMount(async () => {
		unlisten = await listen<HostKeyPrompt>('ssh-hostkey-prompt', (e) => {
			queue = [...queue, e.payload];
		});
	});
	onDestroy(() => unlisten?.());

	async function respond(accept: boolean): Promise<void> {
		const p = current;
		if (!p) return;
		queue = queue.slice(1);
		try {
			await sshHostkeyResponse(p.promptId, accept);
		} catch (err) {
			console.error('Host-key response failed:', err);
		}
	}

	let hostLabel = $derived(current ? `${current.host}:${current.port}` : '');
</script>

{#if current}
	<Modal
		open={true}
		onclose={() => respond(false)}
		zIndex={1000}
		title={current.changed ? t('hostkey.changed_title') : t('hostkey.unknown_title')}
		maxWidth="540px"
	>
		{#if current.changed}
			<div class="warn">
				<span class="warn-icon">⚠</span>
				<span>{t('hostkey.changed_warning', { host: hostLabel })}</span>
			</div>
		{:else}
			<p class="msg">{t('hostkey.unknown_message', { host: hostLabel })}</p>
		{/if}

		<dl class="kv">
			<dt>{t('hostkey.fingerprint_label', { type: current.keyType })}</dt>
			<dd class="mono">{current.fingerprint}</dd>
			{#if current.changed && current.oldFingerprint}
				<dt>{t('hostkey.previous_label')}</dt>
				<dd class="mono old">{current.oldFingerprint}</dd>
			{/if}
		</dl>

		{#snippet actions()}
			<Button variant="secondary" onclick={() => respond(false)}>{t('hostkey.reject')}</Button>
			<Button variant={current.changed ? 'danger' : 'primary'} onclick={() => respond(true)}>
				{t('hostkey.accept')}
			</Button>
		{/snippet}
	</Modal>
{/if}

<style>
	.msg {
		margin: 0 0 12px;
		font-size: 0.875rem;
		line-height: 1.5;
		color: var(--color-text-primary);
	}

	.warn {
		display: flex;
		gap: 8px;
		margin: 0 0 12px;
		padding: 10px 12px;
		font-size: 0.875rem;
		line-height: 1.5;
		color: var(--color-danger);
		background: rgba(255, 69, 58, 0.08);
		border: 1px solid rgba(255, 69, 58, 0.25);
		border-radius: var(--radius-btn);
	}

	.warn-icon {
		flex-shrink: 0;
	}

	.kv {
		margin: 0;
		display: grid;
		grid-template-columns: auto 1fr;
		gap: 4px 12px;
		align-items: baseline;
	}

	.kv dt {
		font-size: 0.6875rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--color-text-secondary);
		white-space: nowrap;
	}

	.kv dd {
		margin: 0;
		font-size: 0.8125rem;
		color: var(--color-text-primary);
		word-break: break-all;
	}

	.mono {
		font-family: var(--font-mono, monospace);
	}

	.old {
		color: var(--color-text-secondary);
		text-decoration: line-through;
	}
</style>
