<script lang="ts">
	import type { PluginUiState } from '$lib/ipc/plugin';
	import PluginUIElement from './PluginUIElement.svelte';

	interface Props {
		uiState: PluginUiState;
		onAction: (action: string) => void;
	}

	let { uiState, onAction }: Props = $props();
</script>

<div class="plugin-ui-renderer">
	{#if uiState.title}
		<h3 class="plugin-title">{uiState.title}</h3>
	{/if}

	{#each uiState.elements as element, i (i)}
		<PluginUIElement {element} {onAction} />
	{/each}
</div>

<style>
	.plugin-ui-renderer {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.plugin-title {
		margin: 0;
		font-family: var(--font-sans);
		font-size: 0.8125rem;
		font-weight: 700;
		color: var(--color-text-primary);
	}
</style>
