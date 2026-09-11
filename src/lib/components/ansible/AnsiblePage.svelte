<script lang="ts">
	import { onMount } from 'svelte';
	import { t } from '$lib/state/i18n.svelte';
	import {
		checkTool,
		getToolStatus,
		isToolInstalled,
		loadEngines,
		areEnginesLoaded,
		hasLocalEngine,
		isSetupSkipped,
		getActiveProjectId
	} from '$lib/state/ansible.svelte';
	import AnsibleToolchainSetup from './AnsibleToolchainSetup.svelte';
	import AnsibleProjectList from './AnsibleProjectList.svelte';
	import AnsibleWorkspace from './AnsibleWorkspace.svelte';

	// One check when the page opens. The setup screen never re-checks on
	// its own — it did once, and each answer remounted the spinner, which
	// remounted the setup screen, which checked again, without end.
	onMount(() => {
		checkTool();
		loadEngines();
	});

	let firstCheck = $derived(getToolStatus() === null || !areEnginesLoaded());
	// The workspace opens when anything here can run a playbook, or when the
	// user chose to write projects now and run them elsewhere.
	let ready = $derived(isToolInstalled() || hasLocalEngine() || isSetupSkipped());
</script>

<div class="ansible-page">
	{#if firstCheck}
		<div class="checking-state">
			<div class="spinner"></div>
			<span class="checking-text">{t('ansible.checking')}</span>
		</div>
	{:else if !ready}
		<AnsibleToolchainSetup />
	{:else if !getActiveProjectId()}
		<AnsibleProjectList />
	{:else}
		<AnsibleWorkspace />
	{/if}
</div>

<style>
	.ansible-page {
		width: 100%;
		height: 100%;
		display: flex;
		flex-direction: column;
		background: var(--color-bg-primary);
		overflow: hidden;
	}

	.checking-state {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 10px;
		padding: 32px;
		margin: 16px;
		border-radius: var(--radius-btn);
		background-color: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
	}

	.spinner {
		width: 18px;
		height: 18px;
		border: 2px solid var(--color-border);
		border-top-color: var(--color-accent);
		border-radius: 50%;
		animation: spin 0.6s linear infinite;
	}

	.checking-text {
		font-size: 0.875rem;
		color: var(--color-text-secondary);
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}
</style>
