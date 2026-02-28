<script lang="ts">
	import { onMount } from 'svelte';
	import { checkTool, isToolInstalled, getActiveProjectId } from '$lib/state/ansible.svelte';
	import AnsibleToolchainSetup from './AnsibleToolchainSetup.svelte';
	import AnsibleProjectList from './AnsibleProjectList.svelte';
	import AnsibleWorkspace from './AnsibleWorkspace.svelte';

	onMount(() => {
		checkTool();
	});
</script>

<div class="ansible-page">
	{#if !isToolInstalled()}
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
</style>
