<script lang="ts">
	import { t } from '$lib/state/i18n.svelte';
	import { getActiveProject, getProjectFiles, isCommandRunning, runCommand } from '$lib/state/ansible.svelte';
	import type { AnsibleExecutionTarget } from '$lib/ipc/ansible';
	import Button from '$lib/components/shared/Button.svelte';

	interface Props {
		target: AnsibleExecutionTarget;
	}

	let { target }: Props = $props();

	let project = $derived(getActiveProject());
	let files = $derived(getProjectFiles());
	let running = $derived(isCommandRunning());

	let selectedFile = $state<string | null>(null);

	function handleVaultAction(command: 'vaultEncrypt' | 'vaultDecrypt' | 'vaultView') {
		if (!project || !selectedFile) return;
		runCommand({
			projectId: project.id,
			command,
			target,
			vaultFile: selectedFile,
			extraArgs: []
		});
	}
</script>

<div class="vault-panel">
	<div class="form-section">
		<label class="field-label">{t('ansible.vault_file')}</label>
		<select class="field-select" bind:value={selectedFile}>
			<option value={null}>--</option>
			{#each files as f (f)}
				<option value={f}>{f}</option>
			{/each}
		</select>
	</div>

	{#if selectedFile}
		<div class="actions">
			<Button variant="primary" size="sm" disabled={running} onclick={() => handleVaultAction('vaultEncrypt')}>
				{t('ansible.vault_encrypt')}
			</Button>
			<Button variant="secondary" size="sm" disabled={running} onclick={() => handleVaultAction('vaultDecrypt')}>
				{t('ansible.vault_decrypt')}
			</Button>
			<Button variant="secondary" size="sm" disabled={running} onclick={() => handleVaultAction('vaultView')}>
				{t('ansible.vault_view')}
			</Button>
		</div>
	{:else}
		<p class="empty-text">{t('ansible.no_vault_files')}</p>
	{/if}
</div>

<style>
	.vault-panel {
		padding: 16px;
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.form-section {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.field-label {
		font-size: 0.75rem;
		font-weight: 600;
		color: var(--color-text-secondary);
		text-transform: uppercase;
		letter-spacing: 0.04em;
	}

	.field-select {
		padding: 8px 10px;
		border-radius: var(--radius-btn);
		border: 1px solid var(--color-border);
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		font-size: 0.8125rem;
		font-family: inherit;
	}

	.field-select:focus {
		outline: none;
		border-color: var(--color-accent);
	}

	.actions {
		display: flex;
		gap: 8px;
	}

	.empty-text {
		margin: 0;
		font-size: 0.8125rem;
		color: var(--color-text-secondary);
		font-style: italic;
	}
</style>
