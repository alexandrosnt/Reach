<script lang="ts">
	import { onMount } from 'svelte';
	import { t } from '$lib/state/i18n.svelte';
	import { getRoles, isRolesLoading, loadRoles, getActiveProject, isCommandRunning, runCommand } from '$lib/state/ansible.svelte';
	import type { AnsibleExecutionTarget } from '$lib/ipc/ansible';
	import Button from '$lib/components/shared/Button.svelte';

	interface Props {
		target: AnsibleExecutionTarget;
	}

	let { target }: Props = $props();

	let rolesList = $derived(getRoles());
	let loading = $derived(isRolesLoading());
	let running = $derived(isCommandRunning());
	let project = $derived(getActiveProject());

	let installName = $state('');

	onMount(() => {
		loadRoles();
	});

	async function handleInstall() {
		if (!project || !installName.trim()) return;
		await runCommand({
			projectId: project.id,
			command: 'galaxyRoleInstall',
			target,
			roleName: installName.trim(),
			extraArgs: []
		});
		installName = '';
		await loadRoles();
	}

	async function handleRemove(roleName: string) {
		if (!project) return;
		await runCommand({
			projectId: project.id,
			command: 'galaxyRoleRemove',
			target,
			roleName,
			extraArgs: []
		});
		await loadRoles();
	}
</script>

<div class="roles-panel">
	<div class="install-row">
		<input
			type="text"
			class="field-input"
			bind:value={installName}
			placeholder={t('ansible.role_name_placeholder')}
		/>
		<Button
			variant="primary"
			size="sm"
			disabled={running || !installName.trim()}
			onclick={handleInstall}
		>
			{t('ansible.install_role')}
		</Button>
	</div>

	{#if loading}
		<p class="loading-text">{t('common.loading')}</p>
	{:else if rolesList.length === 0}
		<p class="empty-text">{t('ansible.no_roles')}</p>
	{:else}
		<div class="item-list">
			{#each rolesList as role (role.name)}
				<div class="item-row">
					<div class="item-info">
						<span class="item-name">{role.name}</span>
						{#if role.version}
							<span class="item-version">{role.version}</span>
						{/if}
					</div>
					<Button variant="danger" size="sm" disabled={running} onclick={() => handleRemove(role.name)}>
						{t('ansible.remove_role')}
					</Button>
				</div>
			{/each}
		</div>
	{/if}
</div>

<style>
	.roles-panel {
		padding: 16px;
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.install-row {
		display: flex;
		gap: 8px;
		align-items: center;
	}

	.field-input {
		flex: 1;
		padding: 8px 10px;
		border-radius: var(--radius-btn);
		border: 1px solid var(--color-border);
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		font-size: 0.8125rem;
		font-family: inherit;
	}

	.field-input:focus {
		outline: none;
		border-color: var(--color-accent);
	}

	.loading-text,
	.empty-text {
		margin: 0;
		font-size: 0.8125rem;
		color: var(--color-text-secondary);
		font-style: italic;
	}

	.item-list {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.item-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 10px 12px;
		border-radius: var(--radius-btn);
		border: 1px solid var(--color-border);
		background: var(--color-bg-primary);
	}

	.item-info {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.item-name {
		font-size: 0.8125rem;
		font-weight: 500;
		color: var(--color-text-primary);
	}

	.item-version {
		font-size: 0.75rem;
		color: var(--color-text-secondary);
	}
</style>
