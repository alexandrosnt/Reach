<script lang="ts">
	import { t } from '$lib/state/i18n.svelte';
	import { getProjectFiles, getActiveProject, isCommandRunning, runCommand } from '$lib/state/ansible.svelte';
	import type { AnsibleCommandRequest, AnsibleExecutionTarget } from '$lib/ipc/ansible';
	import Button from '$lib/components/shared/Button.svelte';

	interface Props {
		target: AnsibleExecutionTarget;
	}

	let { target }: Props = $props();

	let files = $derived(getProjectFiles());
	let project = $derived(getActiveProject());
	let running = $derived(isCommandRunning());

	let playbooks = $derived(files.filter((f) => f.endsWith('.yml') || f.endsWith('.yaml')));
	let inventoryFiles = $derived(files.filter((f) => f.endsWith('.ini') || f.endsWith('.cfg') || f === 'hosts'));

	let selectedPlaybook = $state<string | null>(null);
	let selectedInventory = $state<string | null>(null);
	let extraArgs = $state('');

	function handleRun(command: 'playbook' | 'syntaxCheck') {
		if (!project || !selectedPlaybook) return;
		const request: AnsibleCommandRequest = {
			projectId: project.id,
			command,
			target,
			playbook: selectedPlaybook,
			inventoryFile: selectedInventory,
			extraArgs: extraArgs.trim() ? extraArgs.trim().split(/\s+/) : []
		};
		runCommand(request);
	}
</script>

<div class="playbook-panel">
	{#if playbooks.length === 0}
		<div class="empty-state">
			<p class="empty-text">{t('ansible.no_playbooks')}</p>
		</div>
	{:else}
		<div class="form-section">
			<label class="field-label">{t('ansible.playbook_select')}</label>
			<select class="field-select" bind:value={selectedPlaybook}>
				<option value={null}>--</option>
				{#each playbooks as pb (pb)}
					<option value={pb}>{pb}</option>
				{/each}
			</select>
		</div>

		{#if inventoryFiles.length > 0}
			<div class="form-section">
				<label class="field-label">{t('ansible.inventory_select')}</label>
				<select class="field-select" bind:value={selectedInventory}>
					<option value={null}>--</option>
					{#each inventoryFiles as inv (inv)}
						<option value={inv}>{inv}</option>
					{/each}
				</select>
			</div>
		{/if}

		<div class="form-section">
			<label class="field-label">{t('ansible.extra_args')}</label>
			<input
				type="text"
				class="field-input"
				bind:value={extraArgs}
				placeholder={t('ansible.extra_args_placeholder')}
			/>
		</div>

		<div class="actions">
			<Button
				variant="primary"
				size="sm"
				disabled={running || !selectedPlaybook}
				onclick={() => handleRun('playbook')}
			>
				{t('ansible.run_playbook')}
			</Button>
			<Button
				variant="secondary"
				size="sm"
				disabled={running || !selectedPlaybook}
				onclick={() => handleRun('syntaxCheck')}
			>
				{t('ansible.syntax_check')}
			</Button>
		</div>
	{/if}
</div>

<style>
	.playbook-panel {
		padding: 16px;
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.empty-state {
		padding: 32px;
		text-align: center;
	}

	.empty-text {
		margin: 0;
		font-size: 0.875rem;
		color: var(--color-text-secondary);
		font-style: italic;
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

	.field-select,
	.field-input {
		padding: 8px 10px;
		border-radius: var(--radius-btn);
		border: 1px solid var(--color-border);
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		font-size: 0.8125rem;
		font-family: inherit;
	}

	.field-select:focus,
	.field-input:focus {
		outline: none;
		border-color: var(--color-accent);
	}

	.actions {
		display: flex;
		gap: 8px;
	}
</style>
