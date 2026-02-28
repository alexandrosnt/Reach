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

	let inventoryFiles = $derived(files.filter((f) => f.endsWith('.ini') || f.endsWith('.cfg') || f === 'hosts'));

	let hostPattern = $state('all');
	let moduleName = $state('ping');
	let moduleArgs = $state('');
	let selectedInventory = $state<string | null>(null);

	const modules = ['ping', 'shell', 'command', 'copy', 'yum', 'apt', 'service', 'file', 'setup', 'debug'];

	function handleRun() {
		if (!project) return;
		runCommand({
			projectId: project.id,
			command: 'adHoc',
			target,
			hostPattern,
			moduleName,
			moduleArgs: moduleArgs.trim() || undefined,
			inventoryFile: selectedInventory,
			extraArgs: []
		});
	}
</script>

<div class="adhoc-panel">
	<div class="form-section">
		<label class="field-label">{t('ansible.host_pattern')}</label>
		<input
			type="text"
			class="field-input"
			bind:value={hostPattern}
			placeholder={t('ansible.host_pattern_placeholder')}
		/>
	</div>

	<div class="form-section">
		<label class="field-label">{t('ansible.module')}</label>
		<select class="field-select" bind:value={moduleName}>
			{#each modules as mod (mod)}
				<option value={mod}>{mod}</option>
			{/each}
		</select>
	</div>

	<div class="form-section">
		<label class="field-label">{t('ansible.module_args')}</label>
		<input
			type="text"
			class="field-input"
			bind:value={moduleArgs}
			placeholder={t('ansible.module_args_placeholder')}
		/>
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

	<div class="actions">
		<Button variant="primary" size="sm" disabled={running || !hostPattern.trim()} onclick={handleRun}>
			{t('ansible.run_adhoc')}
		</Button>
	</div>
</div>

<style>
	.adhoc-panel {
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
