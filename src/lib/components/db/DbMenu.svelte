<script lang="ts" module>
	export interface MenuItem {
		label: string;
		action?: () => void;
		danger?: boolean;
		disabled?: boolean;
		separator?: boolean;
		hint?: string;
	}
</script>

<script lang="ts">
	import ContextMenuBackdrop from '$lib/components/shared/ContextMenuBackdrop.svelte';

	interface Props {
		x: number;
		y: number;
		items: MenuItem[];
		onclose: () => void;
	}

	let { x, y, items, onclose }: Props = $props();
	let menu = $state<HTMLDivElement>();

	// Keep the menu on screen near the right and bottom edges.
	let pos = $derived.by(() => {
		const w = menu?.offsetWidth ?? 220;
		const h = menu?.offsetHeight ?? 300;
		return {
			left: Math.max(4, Math.min(x, window.innerWidth - w - 4)),
			top: Math.max(4, Math.min(y, window.innerHeight - h - 4))
		};
	});

	function run(item: MenuItem): void {
		if (item.disabled || item.separator) return;
		onclose();
		item.action?.();
	}

	function onkeydown(e: KeyboardEvent): void {
		if (e.key === 'Escape') onclose();
	}
</script>

<svelte:window {onkeydown} />
<ContextMenuBackdrop {onclose} />
<div class="menu" role="menu" bind:this={menu} style:left="{pos.left}px" style:top="{pos.top}px">
	{#each items as item, i (i)}
		{#if item.separator}
			<div class="sep" role="separator"></div>
		{:else}
			<button
				class="item"
				class:danger={item.danger}
				role="menuitem"
				disabled={item.disabled}
				onclick={() => run(item)}
			>
				<span>{item.label}</span>
				{#if item.hint}<kbd>{item.hint}</kbd>{/if}
			</button>
		{/if}
	{/each}
</div>

<style>
	.menu {
		position: fixed;
		z-index: 1000;
		min-width: 200px;
		max-width: 320px;
		padding: 4px;
		border-radius: var(--radius-card, 8px);
		border: 1px solid var(--color-border);
		background: var(--color-bg-elevated);
		box-shadow: var(--shadow-elevated, 0 8px 24px rgba(0, 0, 0, 0.35));
	}

	.item {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 16px;
		width: 100%;
		padding: 6px 10px;
		border: none;
		border-radius: var(--radius-sm);
		background: transparent;
		color: var(--color-text-primary);
		font-size: var(--text-sm);
		text-align: left;
		cursor: pointer;
	}

	.item:hover:not(:disabled) {
		background: var(--color-surface-hover);
	}

	.item:disabled {
		color: var(--color-text-tertiary);
		cursor: default;
	}

	.item.danger {
		color: var(--color-danger);
	}

	kbd {
		font-family: var(--font-mono);
		font-size: var(--text-2xs, 10px);
		color: var(--color-text-tertiary);
	}

	.sep {
		height: 1px;
		margin: 4px 6px;
		background: var(--color-border);
	}
</style>
