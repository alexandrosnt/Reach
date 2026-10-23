<script lang="ts" module>
	export interface MenuItem {
		label: string;
		action?: () => void;
		danger?: boolean;
		disabled?: boolean;
		separator?: boolean;
		hint?: string;
		/** An SVG path (24×24, stroked) shown before the label. */
		icon?: string;
		/** The current choice, marked with a tick. */
		checked?: boolean;
	}
</script>

<script lang="ts">
	import ContextMenuBackdrop from '$lib/components/shared/ContextMenuBackdrop.svelte';
	import { safeInsets } from '$lib/edges';

	interface Props {
		x: number;
		y: number;
		items: MenuItem[];
		onclose: () => void;
	}

	let { x, y, items, onclose }: Props = $props();
	let menu = $state<HTMLDivElement>();

	// Keep the menu on screen, clear of a phone's status bar, camera cutout
	// and gesture bar as the rest of the app is.
	let pos = $derived.by(() => {
		const w = menu?.offsetWidth ?? 220;
		const h = menu?.offsetHeight ?? 300;
		const safe = safeInsets();
		return {
			left: Math.max(safe.left + 4, Math.min(x, window.innerWidth - safe.right - w - 4)),
			top: Math.max(safe.top + 4, Math.min(y, window.innerHeight - safe.bottom - h - 4))
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

	/** Any press outside the menu closes it. The backdrop catches most, and
	 *  swallows the click so nothing under it runs; but a menu opened from
	 *  the tab bar sits in the bar's layer, and a tap on the terminal went
	 *  past the backdrop and left the menu open. */
	function onpointerdown(e: PointerEvent): void {
		const target = e.target as Element;
		if (!menu || menu.contains(target) || target.classList?.contains('context-backdrop')) return;
		onclose();
	}
</script>

<svelte:window {onkeydown} onpointerdowncapture={onpointerdown} />
<ContextMenuBackdrop {onclose} />
<div class="menu" role="menu" bind:this={menu} style:left="{pos.left}px" style:top="{pos.top}px">
	{#each items as item, i (i)}
		{#if item.separator}
			<div class="sep" role="separator"></div>
		{:else}
			<button
				class="item"
				class:danger={item.danger}
				role={item.checked === undefined ? 'menuitem' : 'menuitemradio'}
				aria-checked={item.checked}
				disabled={item.disabled}
				onclick={() => run(item)}
			>
				<span class="lead">
					{#if item.icon}
						<svg width="16" height="16" viewBox="0 0 24 24" fill="none" aria-hidden="true">
							<path d={item.icon} stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" />
						</svg>
					{/if}
					<span>{item.label}</span>
				</span>
				{#if item.checked}
					<svg class="tick" width="14" height="14" viewBox="0 0 24 24" fill="none" aria-hidden="true">
						<path d="M5 12l5 5 9-10" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
					</svg>
				{:else if item.hint}<kbd>{item.hint}</kbd>{/if}
			</button>
		{/if}
	{/each}
</div>

<style>
	.menu {
		position: fixed;
		z-index: 1000;
		min-width: min(200px, calc(100vw - 8px));
		max-width: min(320px, calc(100vw - 8px));
		max-height: calc(100dvh - var(--safe-top) - var(--safe-bottom) - 8px);
		overflow-y: auto;
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

	.lead {
		display: flex;
		align-items: center;
		gap: 10px;
		min-width: 0;
	}

	.lead svg {
		flex-shrink: 0;
		color: var(--color-text-secondary);
	}

	.tick {
		flex-shrink: 0;
		color: var(--color-accent);
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
