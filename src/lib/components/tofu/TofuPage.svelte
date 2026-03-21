<script lang="ts">
	import { onMount } from 'svelte';
	import { t } from '$lib/state/i18n.svelte';
	import { checkTool, isToolInstalled, isToolChecking, getActiveProjectId } from '$lib/state/tofu.svelte';
	import TofuToolchainSetup from './TofuToolchainSetup.svelte';
	import TofuProjectList from './TofuProjectList.svelte';
	import TofuWorkspace from './TofuWorkspace.svelte';

	onMount(() => {
		checkTool();
	});
</script>

<div class="tofu-page">
	{#if isToolChecking()}
		<div class="checking-state">
			<div class="spinner"></div>
			<span class="checking-text">{t('tofu.checking')}</span>
		</div>
	{:else if !isToolInstalled()}
		<TofuToolchainSetup />
	{:else if !getActiveProjectId()}
		<TofuProjectList />
	{:else}
		<TofuWorkspace />
	{/if}
</div>

<style>
	.tofu-page {
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
