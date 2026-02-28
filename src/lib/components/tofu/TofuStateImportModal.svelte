<script lang="ts">
	import { t } from '$lib/state/i18n.svelte';
	import Modal from '$lib/components/shared/Modal.svelte';
	import Button from '$lib/components/shared/Button.svelte';

	interface Props {
		open: boolean;
		onclose: () => void;
		onimport: (address: string, id: string) => void;
	}

	let { open, onclose, onimport }: Props = $props();

	let address = $state('');
	let resourceId = $state('');

	function handleImport() {
		const trimmedAddr = address.trim();
		const trimmedId = resourceId.trim();
		if (!trimmedAddr || !trimmedId) return;
		onimport(trimmedAddr, trimmedId);
		address = '';
		resourceId = '';
	}

	function handleClose() {
		address = '';
		resourceId = '';
		onclose();
	}
</script>

<Modal open={open} onclose={handleClose} title={t('tofu.state_import_title')}>
	<div class="form">
		<div class="field">
			<label class="label" for="import-address">{t('tofu.state_import_address')}</label>
			<input
				id="import-address"
				type="text"
				class="input"
				placeholder={t('tofu.state_import_address_placeholder')}
				bind:value={address}
			/>
			<span class="help">{t('tofu.state_import_address_help')}</span>
		</div>

		<div class="field">
			<label class="label" for="import-id">{t('tofu.state_import_id')}</label>
			<input
				id="import-id"
				type="text"
				class="input"
				placeholder={t('tofu.state_import_id_placeholder')}
				bind:value={resourceId}
				onkeydown={(e) => { if (e.key === 'Enter') handleImport(); }}
			/>
			<span class="help">{t('tofu.state_import_id_help')}</span>
		</div>
	</div>

	{#snippet actions()}
		<Button variant="secondary" size="sm" onclick={handleClose}>{t('common.cancel')}</Button>
		<Button variant="primary" size="sm" onclick={handleImport} disabled={!address.trim() || !resourceId.trim()}>
			{t('tofu.state_import')}
		</Button>
	{/snippet}
</Modal>

<style>
	.form {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.field {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.label {
		font-size: 0.8125rem;
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.input {
		width: 100%;
		padding: 7px 10px;
		font-size: 0.8125rem;
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		outline: none;
		font-family: monospace;
		box-sizing: border-box;
	}

	.input:focus {
		border-color: var(--color-accent);
	}

	.input::placeholder {
		color: var(--color-text-secondary);
		opacity: 0.5;
	}

	.help {
		font-size: 0.75rem;
		color: var(--color-text-secondary);
		opacity: 0.7;
	}
</style>
