<script lang="ts">
	/**
	 * Open a remote desktop without saving it first.
	 *
	 * The RDP equivalent of Quick Connect. Nothing here talks to the backend:
	 * it collects what a connection needs and opens a tab, and the tab's panel
	 * connects once it knows how big it is — the server renders at the size it
	 * will be seen at, which the dialog cannot know.
	 *
	 * Saved RDP sessions in the vault are the next step, not this one.
	 */
	import Modal from '$lib/components/shared/Modal.svelte';
	import { createTab } from '$lib/state/tabs.svelte';
	import { t } from '$lib/state/i18n.svelte';

	interface Props {
		open: boolean;
	}

	let { open = $bindable(false) }: Props = $props();

	let host = $state('');
	let portStr = $state('3389');
	let username = $state('');
	let password = $state('');
	let domain = $state('');

	let canConnect = $derived(host.trim() !== '' && username.trim() !== '');

	function close(): void {
		open = false;
	}

	function connect(): void {
		if (!canConnect) return;
		const port = parseInt(portStr, 10) || 3389;
		const h = host.trim();
		const u = username.trim();

		const tab = createTab('rdp', `${u}@${h}`);
		tab.rdpConnectParams = {
			id: tab.id,
			host: h,
			port,
			username: u,
			password,
			domain: domain.trim() || undefined,
			// Filled in by the panel from its own size.
			width: 0,
			height: 0,
		};

		// The password is not kept here once handed over.
		password = '';
		close();
	}

</script>

<Modal {open} onclose={close} title={t('session.rdp_connect')}>
	<!-- Enter in any field submits: that is what a form with a submit button
	     does on its own, so there is no key handler here. -->
	<form class="form" onsubmit={(e) => { e.preventDefault(); connect(); }}>
		<div class="row two">
			<label>
				<span>{t('session.host')}</span>
				<!-- svelte-ignore a11y_autofocus -->
				<input type="text" bind:value={host} placeholder="10.0.0.5" autocomplete="off" spellcheck="false" autofocus />
			</label>
			<label class="port">
				<span>{t('session.port')}</span>
				<input type="text" inputmode="numeric" bind:value={portStr} />
			</label>
		</div>

		<label>
			<span>{t('session.username')}</span>
			<input type="text" bind:value={username} autocomplete="off" spellcheck="false" />
		</label>

		<label>
			<span>{t('session.password')}</span>
			<input type="password" bind:value={password} autocomplete="off" />
		</label>

		<label>
			<span>{t('session.rdp_domain')}</span>
			<input type="text" bind:value={domain} autocomplete="off" spellcheck="false" />
		</label>

		<div class="actions">
			<button type="button" class="secondary" onclick={close}>{t('common.cancel')}</button>
			<button type="submit" class="primary" disabled={!canConnect}>{t('session.connect')}</button>
		</div>
	</form>
</Modal>

<style>
	.form {
		display: flex;
		flex-direction: column;
		gap: 12px;
		min-width: 320px;
	}

	.row.two {
		display: grid;
		grid-template-columns: 1fr 96px;
		gap: 8px;
	}

	label {
		display: flex;
		flex-direction: column;
		gap: 4px;
		font-size: 0.75rem;
		color: var(--color-text-secondary);
	}

	input {
		padding: 6px 10px;
		font: inherit;
		font-size: 0.8125rem;
		color: var(--color-text-primary);
		background: var(--color-surface-sunken, transparent);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-sm);
		outline: none;
	}

	input:focus {
		border-color: var(--color-accent);
	}

	.actions {
		display: flex;
		justify-content: flex-end;
		gap: 8px;
		margin-top: 4px;
	}

	button {
		padding: 6px 14px;
		font: inherit;
		font-size: 0.8125rem;
		font-weight: 500;
		border-radius: var(--radius-btn);
		border: 1px solid var(--color-border);
		cursor: pointer;
		transition: background-color 150ms ease, color 150ms ease;
	}

	.secondary {
		background: transparent;
		color: var(--color-text-secondary);
	}

	.secondary:hover {
		background: var(--color-surface-hover);
		color: var(--color-text-primary);
	}

	.primary {
		background: var(--color-accent);
		border-color: var(--color-accent);
		color: #fff;
	}

	.primary:hover:not(:disabled) {
		background: var(--color-accent-hover);
	}

	.primary:disabled {
		opacity: 0.5;
		cursor: default;
	}
</style>
