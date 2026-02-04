<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		direction: 'horizontal' | 'vertical';
		initialRatio?: number;
		minSize?: number;
		first: Snippet;
		second: Snippet;
	}

	let {
		direction,
		initialRatio = 0.5,
		minSize = 100,
		first,
		second
	}: Props = $props();

	let ratio = $state(0.5);

	$effect(() => {
		ratio = initialRatio;
	});
	let containerEl: HTMLDivElement | undefined = $state(undefined);
	let containerWidth = $state(0);
	let containerHeight = $state(0);
	let isDragging = $state(false);

	let isHorizontal = $derived(direction === 'horizontal');

	let containerSize = $derived(isHorizontal ? containerWidth : containerHeight);

	let firstSize = $derived.by(() => {
		const size = ratio * containerSize;
		return Math.max(minSize, Math.min(size, containerSize - minSize));
	});

	let secondSize = $derived(containerSize - firstSize - 4);

	let firstStyle = $derived(
		isHorizontal
			? `width:${firstSize}px;height:100%`
			: `height:${firstSize}px;width:100%`
	);

	let secondStyle = $derived(
		isHorizontal
			? `width:${secondSize}px;height:100%`
			: `height:${secondSize}px;width:100%`
	);

	let dividerStyle = $derived(
		isHorizontal
			? `width:4px;height:100%;cursor:col-resize`
			: `height:4px;width:100%;cursor:row-resize`
	);

	$effect(() => {
		if (!containerEl) return;

		const observer = new ResizeObserver((entries) => {
			for (const entry of entries) {
				containerWidth = entry.contentRect.width;
				containerHeight = entry.contentRect.height;
			}
		});

		observer.observe(containerEl);

		return () => {
			observer.disconnect();
		};
	});

	function onDividerPointerDown(e: PointerEvent) {
		e.preventDefault();
		isDragging = true;

		const target = e.currentTarget as HTMLElement;
		target.setPointerCapture(e.pointerId);

		document.body.style.userSelect = 'none';
	}

	function onDividerPointerMove(e: PointerEvent) {
		if (!isDragging || !containerEl) return;

		const rect = containerEl.getBoundingClientRect();
		const pos = isHorizontal ? e.clientX - rect.left : e.clientY - rect.top;
		const total = isHorizontal ? rect.width : rect.height;

		if (total <= 0) return;

		const newRatio = pos / total;
		const minRatio = minSize / total;
		const maxRatio = 1 - minRatio;

		ratio = Math.max(minRatio, Math.min(newRatio, maxRatio));
	}

	function onDividerPointerUp(e: PointerEvent) {
		if (!isDragging) return;
		isDragging = false;

		const target = e.currentTarget as HTMLElement;
		target.releasePointerCapture(e.pointerId);

		document.body.style.userSelect = '';
	}
</script>

<div
	class="split-container"
	class:horizontal={isHorizontal}
	class:vertical={!isHorizontal}
	bind:this={containerEl}
>
	<div class="pane" style={firstStyle}>
		{@render first()}
	</div>

	<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
	<div
		class="divider"
		class:dragging={isDragging}
		style={dividerStyle}
		role="separator"
		aria-orientation={isHorizontal ? 'vertical' : 'horizontal'}
		tabindex="0"
		onpointerdown={onDividerPointerDown}
		onpointermove={onDividerPointerMove}
		onpointerup={onDividerPointerUp}
	></div>

	<div class="pane" style={secondStyle}>
		{@render second()}
	</div>
</div>

<style>
	.split-container {
		display: flex;
		width: 100%;
		height: 100%;
		overflow: hidden;
	}

	.split-container.horizontal {
		flex-direction: row;
	}

	.split-container.vertical {
		flex-direction: column;
	}

	.pane {
		overflow: hidden;
		min-width: 0;
		min-height: 0;
	}

	.divider {
		flex-shrink: 0;
		background-color: var(--color-border);
		transition: background-color var(--duration-default) var(--ease-default);
		touch-action: none;
	}

	.divider:hover,
	.divider.dragging {
		background-color: var(--color-accent);
	}

	.divider:focus-visible {
		outline: 2px solid var(--color-accent);
		outline-offset: -1px;
	}
</style>
