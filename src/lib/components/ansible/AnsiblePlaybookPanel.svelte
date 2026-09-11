<!--
	The playbook composer: one row in the run bar that says what will run.

	Playbook, inventory and extra arguments on the left; Syntax check, Dry
	run and Run on the right. It used to be a form above a half-height
	output; now the output has the whole page and the composer is a line.
-->
<script lang="ts">
	import { t } from '$lib/state/i18n.svelte';
	import { getProjectFiles, getActiveProject, isCommandRunning, runCommand } from '$lib/state/ansible.svelte';
	import type { AnsibleCommandRequest, AnsibleExecutionTarget } from '$lib/ipc/ansible';
	import Button from '$lib/components/shared/Button.svelte';

	interface Props {
		target: AnsibleExecutionTarget | null;
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

	// One playbook and one inventory: nothing to choose, so it is chosen.
	$effect(() => {
		if (selectedPlaybook === null && playbooks.length === 1) selectedPlaybook = playbooks[0];
		if (selectedInventory === null && inventoryFiles.length === 1) selectedInventory = inventoryFiles[0];
	});

	let ready = $derived(!running && !!target && !!selectedPlaybook);

	/**
	 * `check` is the dry run: `--check --diff` reports what would change and
	 * shows the diff, changing nothing. It is the button to reach for first.
	 */
	function handleRun(command: 'playbook' | 'syntaxCheck' | 'check') {
		if (!project || !target || !selectedPlaybook) return;
		const extra = extraArgs.trim() ? extraArgs.trim().split(/\s+/) : [];
		const request: AnsibleCommandRequest = {
			projectId: project.id,
			command: command === 'check' ? 'playbook' : command,
			target,
			playbook: selectedPlaybook,
			inventoryFile: selectedInventory,
			extraArgs: command === 'check' ? ['--check', '--diff', ...extra] : extra
		};
		runCommand(request);
	}
</script>

<div class="composer">
	{#if playbooks.length === 0}
		<span class="empty-text">{t('ansible.no_playbooks')}</span>
	{:else}
		<label class="field">
			<span class="field-label">{t('ansible.playbook')}</span>
			<select class="field-select" bind:value={selectedPlaybook}>
				<option value={null}>—</option>
				{#each playbooks as pb (pb)}
					<option value={pb}>{pb}</option>
				{/each}
			</select>
		</label>

		{#if inventoryFiles.length > 0}
			<label class="field">
				<span class="field-label">{t('ansible.inventory')}</span>
				<select class="field-select" bind:value={selectedInventory}>
					<option value={null}>—</option>
					{#each inventoryFiles as inv (inv)}
						<option value={inv}>{inv}</option>
					{/each}
				</select>
			</label>
		{/if}

		<label class="field grow">
			<span class="field-label">{t('ansible.extra_args')}</span>
			<input
				type="text"
				class="field-input mono"
				bind:value={extraArgs}
				placeholder={t('ansible.extra_args_placeholder')}
			/>
		</label>

		<div class="actions">
			<Button variant="ghost" size="sm" disabled={!ready} onclick={() => handleRun('syntaxCheck')}>
				{t('ansible.syntax_check')}
			</Button>
			<span title={t('ansible.check_diff_hint')}>
				<Button variant="secondary" size="sm" disabled={!ready} onclick={() => handleRun('check')}>
					{t('ansible.check_diff')}
				</Button>
			</span>
			<Button variant="primary" size="sm" disabled={!ready} onclick={() => handleRun('playbook')}>
				{t('ansible.run')}
			</Button>
		</div>
	{/if}
</div>

<style>
	.composer {
		display: flex;
		align-items: flex-end;
		flex-wrap: wrap;
		gap: 8px 12px;
		min-width: 0;
	}

	.empty-text {
		font-size: 0.8125rem;
		color: var(--color-text-secondary);
		font-style: italic;
	}

	.field {
		display: flex;
		flex-direction: column;
		gap: 4px;
		min-width: 0;
	}

	.field.grow {
		flex: 1 1 180px;
	}

	.field-label {
		font-size: 0.6875rem;
		font-weight: 600;
		color: var(--color-text-secondary);
		text-transform: uppercase;
		letter-spacing: 0.04em;
	}

	.field-select,
	.field-input {
		padding: 6px 10px;
		border-radius: var(--radius-btn);
		border: 1px solid var(--color-border);
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		font-size: 0.8125rem;
		font-family: inherit;
		max-width: 240px;
	}

	.field-input {
		max-width: none;
		width: 100%;
	}

	.field-input.mono {
		font-family: var(--font-mono, monospace);
		font-size: 0.75rem;
	}

	.field-select:focus,
	.field-input:focus {
		outline: none;
		border-color: var(--color-accent);
	}

	.actions {
		display: flex;
		gap: 6px;
		margin-left: auto;
		flex-wrap: wrap;
	}
</style>
