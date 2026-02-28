<script lang="ts">
	import { onMount } from 'svelte';
	import { checkTool, isToolInstalled, getActiveProjectId } from '$lib/state/tofu.svelte';
	import TofuToolchainSetup from './TofuToolchainSetup.svelte';
	import TofuProjectList from './TofuProjectList.svelte';
	import TofuWorkspace from './TofuWorkspace.svelte';

	onMount(() => {
		checkTool();
	});
</script>

<div class="tofu-page">
	{#if !isToolInstalled()}
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
</style>
