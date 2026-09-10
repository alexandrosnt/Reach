<!--
	Start or join a share.

	The whole handshake is two pastes: the host gives a code, the guest gives
	one back. This dialog walks each side through its half and is otherwise
	out of the way — once connected it can be closed, and the bottom bar
	carries the status.

	The one setting that matters is stated in the user's own terms: "what may
	they do to my terminal". It defaults to read-only, and it is enforced on
	this machine regardless of what the other side asks for.
-->
<script lang="ts">
	import Modal from '$lib/components/shared/Modal.svelte';
	import Button from '$lib/components/shared/Button.svelte';
	import { t } from '$lib/state/i18n.svelte';
	import { addToast } from '$lib/state/toasts.svelte';
	import { getTabs, getActiveTab } from '$lib/state/tabs.svelte';
	import * as share from '$lib/state/share.svelte';
	import type { Mode } from '$lib/share/protocol';

	interface Props {
		open: boolean;
		onclose: () => void;
	}

	let { open, onclose }: Props = $props();

	let session = $derived(share.getSession());
	let phase = $derived(session?.phase ?? 'idle');
	let tabs = $derived(getTabs());

	let role = $state<'host' | 'guest' | null>(null);
	let localTabId = $state<string>('');
	let localMode = $state<Mode>('read-only');
	let pasted = $state('');
	let copied = $state(false);
	let busy = $state(false);

	// Default to the tab the user is looking at. Chosen once per opening;
	// following the active tab live would swap the selection under them.
	$effect(() => {
		if (open && !session) {
			localTabId = getActiveTab()?.id ?? '';
			role = null;
			pasted = '';
			copied = false;
		}
	});

	async function start(): Promise<void> {
		busy = true;
		try {
			await share.startHosting(localTabId || null, localMode);
		} finally {
			busy = false;
		}
	}

	async function join(): Promise<void> {
		if (!pasted.trim()) return;
		busy = true;
		try {
			await share.joinWithCode(pasted, localTabId || null, localMode);
			pasted = '';
		} finally {
			busy = false;
		}
	}

	async function connect(): Promise<void> {
		if (!pasted.trim()) return;
		busy = true;
		try {
			await share.acceptAnswer(pasted);
			if (share.getSession()?.failure?.kind !== 'bad-code') pasted = '';
		} finally {
			busy = false;
		}
	}

	async function copy(): Promise<void> {
		const code = session?.myCode;
		if (!code) return;
		try {
			const { writeText } = await import('@tauri-apps/plugin-clipboard-manager');
			await writeText(code);
		} catch {
			await navigator.clipboard?.writeText(code);
		}
		copied = true;
		setTimeout(() => (copied = false), 1800);
		addToast(t('sharing.copied'), 'success');
	}

	function changeMode(mode: Mode): void {
		localMode = mode;
		if (session) share.setLocalMode(mode);
	}

	function dismiss(): void {
		share.dismiss();
		onclose();
	}

	let failureText = $derived.by(() => {
		const f = session?.failure;
		if (!f) return '';
		switch (f.kind) {
			case 'no-path':
				return t('sharing.fail_no_path');
			case 'bad-code':
				return t('sharing.fail_bad_code');
			case 'dropped':
				return t('sharing.fail_dropped');
			case 'tampered':
				return t('sharing.fail_tampered');
			case 'timeout':
				return t('sharing.fail_timeout');
		}
	});
</script>

<Modal {open} onclose={phase === 'idle' || phase === 'connected' ? onclose : dismiss} title={t('sharing.title')} maxWidth="560px">
	<div class="share">
		{#if !session || phase === 'idle'}
			<!-- Role -->
			<div class="roles">
				<button class="role" class:picked={role === 'host'} onclick={() => (role = 'host')}>
					<strong>{t('sharing.start')}</strong>
					<span>{t('sharing.start_desc')}</span>
				</button>
				<button class="role" class:picked={role === 'guest'} onclick={() => (role = 'guest')}>
					<strong>{t('sharing.join')}</strong>
					<span>{t('sharing.join_desc')}</span>
				</button>
			</div>

			{#if role}
				<!-- What I share, and on what terms -->
				<label class="field">
					<span class="label">{t('sharing.my_terminal')}</span>
					<select bind:value={localTabId}>
						<option value="">{t('sharing.none_watch_only')}</option>
						{#each tabs as tab (tab.id)}
							<option value={tab.id}>{tab.title}</option>
						{/each}
					</select>
				</label>

				{#if localTabId}
					<div class="field">
						<span class="label">{t('sharing.they_may')}</span>
						<div class="segmented">
							<button class:on={localMode === 'read-only'} onclick={() => changeMode('read-only')}>
								{t('sharing.read_only')}
							</button>
							<button class:on={localMode === 'read-write'} onclick={() => changeMode('read-write')}>
								{t('sharing.read_write')}
							</button>
						</div>
						{#if localMode === 'read-write'}
							<p class="warn">{t('sharing.read_write_warn')}</p>
						{/if}
					</div>
				{/if}

				{#if role === 'guest'}
					<label class="field">
						<span class="label">{t('sharing.paste_code')}</span>
						<textarea bind:value={pasted} rows="3" spellcheck="false" placeholder="REACH-…"></textarea>
					</label>
				{/if}

				{#if session?.failure?.kind === 'bad-code'}
					<p class="error">{failureText}</p>
				{/if}
			{/if}
		{:else if phase === 'preparing'}
			<p class="status">{t('sharing.preparing')}</p>
		{:else if phase === 'exchanging'}
			<div class="field">
				<span class="label">{t('sharing.send_this')}</span>
				<div class="code">
					<code>{session.myCode}</code>
					<button class="mini" onclick={copy}>{copied ? t('sharing.copied') : t('sharing.copy')}</button>
				</div>
			</div>

			{#if session.role === 'host'}
				<label class="field">
					<span class="label">{t('sharing.paste_reply')}</span>
					<textarea bind:value={pasted} rows="3" spellcheck="false" placeholder="REACH-…"></textarea>
				</label>
				{#if session.failure?.kind === 'bad-code'}
					<p class="error">{failureText}</p>
				{/if}
			{:else}
				<p class="status">{t('sharing.waiting')}</p>
			{/if}
		{:else if phase === 'connecting'}
			<p class="status">{t('sharing.connecting')}</p>
		{:else if phase === 'connected'}
			<div class="summary">
				<div class="row">
					<span class="dim">{t('sharing.connected_to')}</span>
					<code>{session.remote?.title || t('sharing.remote_title')}</code>
				</div>
				<div class="row">
					<span class="dim">{t('sharing.they_may')}</span>
					<div class="segmented small">
						<button class:on={session.localMode === 'read-only'} onclick={() => changeMode('read-only')}>
							{t('sharing.read_only')}
						</button>
						<button class:on={session.localMode === 'read-write'} onclick={() => changeMode('read-write')}>
							{t('sharing.read_write')}
						</button>
					</div>
				</div>
				<div class="row">
					<span class="dim">{t('sharing.you_may')}</span>
					<span>
						{#if !session.remote?.sharing}
							{t('sharing.not_sharing')}
						{:else if session.remote.mode === 'read-write'}
							{t('sharing.read_write')}
						{:else}
							{t('sharing.read_only')}
						{/if}
					</span>
				</div>
			</div>
		{:else if phase === 'failed'}
			<p class="error">{failureText}</p>
			{#if session.failure?.detail}
				<p class="detail">{session.failure.detail}</p>
			{/if}
		{:else if phase === 'ended'}
			<p class="status">{t('sharing.ended')}</p>
		{/if}
	</div>

	{#snippet actions()}
		{#if !session || phase === 'idle'}
			<Button variant="secondary" onclick={onclose}>{t('common.cancel')}</Button>
			{#if role === 'host'}
				<Button variant="primary" disabled={busy} onclick={start}>{t('sharing.start')}</Button>
			{:else if role === 'guest'}
				<Button variant="primary" disabled={busy || !pasted.trim()} onclick={join}>{t('sharing.join')}</Button>
			{/if}
		{:else if phase === 'exchanging' && session.role === 'host'}
			<Button variant="secondary" onclick={dismiss}>{t('common.cancel')}</Button>
			<Button variant="primary" disabled={busy || !pasted.trim()} onclick={connect}>{t('sharing.connect')}</Button>
		{:else if phase === 'connected'}
			<Button variant="danger" onclick={() => share.end()}>{t('sharing.end')}</Button>
			<Button variant="primary" onclick={onclose}>{t('common.close')}</Button>
		{:else if phase === 'failed' || phase === 'ended'}
			<Button variant="primary" onclick={dismiss}>{t('common.close')}</Button>
		{:else}
			<Button variant="secondary" onclick={dismiss}>{t('common.cancel')}</Button>
		{/if}
	{/snippet}
</Modal>

<style>
	.share {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
	}

	.roles {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: var(--space-2);
	}

	.role {
		display: flex;
		flex-direction: column;
		gap: 4px;
		padding: var(--space-3);
		text-align: left;
		font-family: inherit;
		color: var(--color-text-secondary);
		background: var(--color-surface-sunken);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-card);
		cursor: pointer;
	}

	.role strong {
		font-size: var(--text-sm);
		color: var(--color-text-primary);
	}

	.role span {
		font-size: var(--text-xs);
		line-height: 1.4;
	}

	.role.picked {
		border-color: var(--color-accent);
		background: color-mix(in srgb, var(--color-accent) 8%, var(--color-surface-sunken));
	}

	.field {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.label {
		font-size: var(--text-xs);
		font-weight: 500;
		color: var(--color-text-secondary);
	}

	select,
	textarea {
		padding: 7px 10px;
		font-family: var(--font-mono);
		font-size: var(--text-xs);
		color: var(--color-text-primary);
		background: var(--color-surface-sunken);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-sm);
	}

	select {
		font-family: inherit;
	}

	textarea {
		resize: vertical;
		overflow-wrap: anywhere;
	}

	.segmented {
		display: inline-flex;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-sm);
		overflow: hidden;
		align-self: flex-start;
	}

	.segmented button {
		padding: 6px 12px;
		font-family: inherit;
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
		background: transparent;
		border: none;
		cursor: pointer;
	}

	.segmented button.on {
		color: var(--color-text-primary);
		background: var(--color-surface-active);
	}

	.segmented.small button {
		padding: 3px 9px;
		font-size: var(--text-2xs);
	}

	.warn {
		margin: 0;
		font-size: var(--text-xs);
		color: var(--color-warning);
	}

	.code {
		display: flex;
		align-items: flex-start;
		gap: var(--space-2);
	}

	.code code {
		flex: 1;
		min-width: 0;
		max-height: 120px;
		overflow: auto;
		padding: var(--space-2);
		font-family: var(--font-mono);
		font-size: var(--text-2xs);
		line-height: 1.5;
		color: var(--color-text-primary);
		background: var(--color-surface-sunken);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-sm);
		overflow-wrap: anywhere;
		user-select: all;
	}

	.mini {
		flex-shrink: 0;
		padding: 5px 10px;
		font-family: inherit;
		font-size: var(--text-xs);
		color: var(--color-accent);
		background: transparent;
		border: 1px solid color-mix(in srgb, var(--color-accent) 50%, var(--color-border));
		border-radius: var(--radius-sm);
		cursor: pointer;
	}

	.status {
		margin: 0;
		padding: var(--space-3);
		text-align: center;
		font-size: var(--text-sm);
		color: var(--color-text-secondary);
	}

	.summary {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}

	.row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-3);
		font-size: var(--text-sm);
		color: var(--color-text-primary);
	}

	.row code {
		font-family: var(--font-mono);
		font-size: var(--text-xs);
	}

	.dim {
		color: var(--color-text-tertiary);
		font-size: var(--text-xs);
	}

	.error {
		margin: 0;
		font-size: var(--text-sm);
		color: var(--color-danger);
	}

	.detail {
		margin: 0;
		font-size: var(--text-xs);
		line-height: 1.5;
		color: var(--color-text-secondary);
	}
</style>
