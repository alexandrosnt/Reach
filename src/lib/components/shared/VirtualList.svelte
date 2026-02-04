<script lang="ts" generics="T">
	import type { Snippet } from 'svelte';

	interface Props {
		items: T[];
		itemHeight: number;
		children: Snippet<[T, number]>;
	}

	let {
		items,
		itemHeight,
		children
	}: Props = $props();

	let containerEl: HTMLDivElement | undefined = $state();
	let scrollTop = $state(0);
	let containerHeight = $state(0);

	let totalHeight = $derived(items.length * itemHeight);

	let startIndex = $derived(Math.floor(scrollTop / itemHeight));

	let visibleCount = $derived(
		Math.ceil(containerHeight / itemHeight) + 1
	);

	let endIndex = $derived(
		Math.min(startIndex + visibleCount, items.length)
	);

	let visibleItems = $derived(
		items.slice(startIndex, endIndex).map((item, i) => ({
			item,
			index: startIndex + i
		}))
	);

	let offsetY = $derived(startIndex * itemHeight);

	function onScroll(e: UIEvent) {
		const el = e.currentTarget as HTMLDivElement;
		scrollTop = el.scrollTop;
	}

	$effect(() => {
		if (!containerEl) return;

		const observer = new ResizeObserver((entries) => {
			for (const entry of entries) {
				containerHeight = entry.contentRect.height;
			}
		});

		observer.observe(containerEl);

		return () => {
			observer.disconnect();
		};
	});
</script>

<div
	class="virtual-list"
	bind:this={containerEl}
	onscroll={onScroll}
>
	<div class="virtual-list-spacer" style="height: {totalHeight}px;">
		<div class="virtual-list-visible" style="transform: translateY({offsetY}px);">
			{#each visibleItems as { item, index } (index)}
				<div class="virtual-list-item" style="height: {itemHeight}px;">
					{@render children(item, index)}
				</div>
			{/each}
		</div>
	</div>
</div>

<style>
	.virtual-list {
		overflow-y: auto;
		position: relative;
		width: 100%;
		height: 100%;
	}

	.virtual-list-spacer {
		position: relative;
		width: 100%;
	}

	.virtual-list-visible {
		position: absolute;
		top: 0;
		left: 0;
		width: 100%;
	}

	.virtual-list-item {
		overflow: hidden;
		box-sizing: border-box;
	}
</style>
