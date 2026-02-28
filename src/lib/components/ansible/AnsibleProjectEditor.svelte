<script lang="ts">
	import type { AnsibleProject } from '$lib/ipc/ansible';
	import { createProject } from '$lib/state/ansible.svelte';
	import { t } from '$lib/state/i18n.svelte';
	import { open } from '@tauri-apps/plugin-dialog';
	import Modal from '$lib/components/shared/Modal.svelte';
	import Input from '$lib/components/shared/Input.svelte';
	import Button from '$lib/components/shared/Button.svelte';

	interface Props {
		open: boolean;
		onclose: () => void;
		oncreate: (project: AnsibleProject) => void;
	}

	let { open: isOpen, onclose, oncreate }: Props = $props();

	let name = $state('');
	let path = $state('');
	let description = $state('');
	let loading = $state(false);

	let canSubmit = $derived(name.trim().length > 0 && path.trim().length > 0 && !loading);

	function resetForm() {
		name = '';
		path = '';
		description = '';
		loading = false;
	}

	function handleClose() {
		resetForm();
		onclose();
	}

	async function browsePath() {
		try {
			const selected = await open({ directory: true });
			if (selected) {
				path = selected as string;
			}
		} catch {
			// User cancelled the dialog
		}
	}

	async function handleSubmit() {
		if (!canSubmit) return;

		loading = true;
		try {
			const project = await createProject(name.trim(), path.trim(), description.trim());
			resetForm();
			oncreate(project);
		} catch {
			loading = false;
		}
	}
</script>

<Modal open={isOpen} onclose={handleClose} title={t('ansible.new_project')}>
	<form class="form" onsubmit={handleSubmit}>
		<Input label={t('ansible.project_name')} bind:value={name} placeholder={t('ansible.project_name')} />

		<div class="path-row">
			<div class="path-input">
				<Input label={t('ansible.project_path')} bind:value={path} placeholder={t('ansible.project_path')} />
			</div>
			<Button variant="secondary" size="sm" onclick={browsePath}>
				{t('ansible.browse')}
			</Button>
		</div>

		<Input
			label={t('ansible.project_description')}
			bind:value={description}
			placeholder={t('ansible.project_description')}
		/>
	</form>

	{#snippet actions()}
		<Button variant="secondary" size="sm" onclick={handleClose} disabled={loading}>
			{t('ansible.cancel')}
		</Button>
		<Button variant="primary" size="sm" onclick={handleSubmit} disabled={!canSubmit}>
			{loading ? t('ansible.creating') : t('ansible.create')}
		</Button>
	{/snippet}
</Modal>

<style>
	.form {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.path-row {
		display: flex;
		align-items: flex-end;
		gap: 8px;
	}

	.path-input {
		flex: 1;
	}
</style>
