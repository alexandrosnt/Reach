<script lang="ts">
	import { onMount } from 'svelte';
	import { t } from '$lib/state/i18n.svelte';
	import { getDependencyGraph, isGraphLoading, loadDependencyGraph } from '$lib/state/tofu.svelte';
	import Button from '$lib/components/shared/Button.svelte';

	let graph = $derived(getDependencyGraph());
	let loading = $derived(isGraphLoading());

	let viewBoxX = $state(0);
	let viewBoxY = $state(0);
	let viewBoxScale = $state(1);
	let isPanning = $state(false);
	let panStartX = $state(0);
	let panStartY = $state(0);

	const DEFAULT_WIDTH = 800;
	const DEFAULT_HEIGHT = 600;

	const providerColors: Record<string, string> = {
		aws: '#f59e0b',
		azure: '#0078d4',
		azurerm: '#0078d4',
		google: '#ea4335',
		docker: '#0db7ed',
		kubernetes: '#326ce5'
	};

	const DEFAULT_COLOR = '#6b7280';

	function getNodeColor(providerId: string): string {
		const key = providerId.toLowerCase();
		for (const [prefix, color] of Object.entries(providerColors)) {
			if (key.includes(prefix)) return color;
		}
		return DEFAULT_COLOR;
	}

	function getNodeCenter(node: { x: number; y: number }): { cx: number; cy: number } {
		return { cx: node.x + 100, cy: node.y + 25 };
	}

	function handleMouseDown(e: MouseEvent) {
		isPanning = true;
		panStartX = e.clientX;
		panStartY = e.clientY;
	}

	function handleMouseMove(e: MouseEvent) {
		if (!isPanning) return;
		const dx = (e.clientX - panStartX) * viewBoxScale;
		const dy = (e.clientY - panStartY) * viewBoxScale;
		viewBoxX -= dx;
		viewBoxY -= dy;
		panStartX = e.clientX;
		panStartY = e.clientY;
	}

	function handleMouseUp() {
		isPanning = false;
	}

	function handleWheel(e: WheelEvent) {
		e.preventDefault();
		let newScale = viewBoxScale + e.deltaY * 0.001;
		if (newScale < 0.3) newScale = 0.3;
		if (newScale > 3) newScale = 3;
		viewBoxScale = newScale;
	}

	function handleRefresh() {
		loadDependencyGraph();
	}

	onMount(() => {
		loadDependencyGraph();
	});
</script>

<div class="graph-panel">
	<header class="header">
		<h2 class="title">{t('tofu.graph_title')}</h2>
		<div class="header-actions">
			<Button variant="secondary" size="sm" onclick={handleRefresh} disabled={loading}>
				{t('common.refresh')}
			</Button>
		</div>
	</header>

	{#if loading}
		<div class="loading-state">
			<div class="spinner"></div>
			<p class="loading-text">{t('tofu.graph_loading')}</p>
		</div>
	{:else if !graph || graph.nodes.length === 0}
		<div class="empty-state">
			<svg width="48" height="48" viewBox="0 0 24 24" fill="none" class="empty-icon">
				<circle cx="5" cy="12" r="2" stroke="currentColor" stroke-width="1.5" />
				<circle cx="19" cy="6" r="2" stroke="currentColor" stroke-width="1.5" />
				<circle cx="19" cy="18" r="2" stroke="currentColor" stroke-width="1.5" />
				<path d="M7 11l10-4M7 13l10 4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
			</svg>
			<p class="empty-text">{t('tofu.graph_empty')}</p>
		</div>
	{:else}
		{#if graph.edges.length === 0}
			<p class="no-deps-message">{t('tofu.graph_no_deps')}</p>
		{/if}

		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div class="svg-container">
			<svg
				class="graph-svg"
				viewBox="{viewBoxX} {viewBoxY} {DEFAULT_WIDTH * viewBoxScale} {DEFAULT_HEIGHT * viewBoxScale}"
				onmousedown={handleMouseDown}
				onmousemove={handleMouseMove}
				onmouseup={handleMouseUp}
				onmouseleave={handleMouseUp}
				onwheel={handleWheel}
			>
				<defs>
					<marker
						id="arrowhead"
						markerWidth="10"
						markerHeight="7"
						refX="10"
						refY="3.5"
						orient="auto"
					>
						<polygon points="0 0, 10 3.5, 0 7" fill="var(--color-text-secondary)" />
					</marker>
				</defs>

				{#each graph.edges as edge (edge.fromId + '-' + edge.toId)}
					{@const sourceNode = graph.nodes.find((n) => n.id === edge.fromId)}
					{@const targetNode = graph.nodes.find((n) => n.id === edge.toId)}
					{#if sourceNode && targetNode}
						{@const source = getNodeCenter(sourceNode)}
						{@const target = getNodeCenter(targetNode)}
						<line
							x1={source.cx}
							y1={source.cy}
							x2={target.cx}
							y2={target.cy}
							stroke="var(--color-text-secondary)"
							stroke-width="1.5"
							opacity="0.5"
							marker-end="url(#arrowhead)"
						/>
					{/if}
				{/each}

				{#each graph.nodes as node (node.id)}
					{@const color = getNodeColor(node.providerId)}
					<rect
						x={node.x}
						y={node.y}
						width="200"
						height="50"
						rx="8"
						fill={color}
						opacity="0.9"
					/>
					<text
						x={node.x + 100}
						y={node.y + 25}
						text-anchor="middle"
						dominant-baseline="central"
						fill="white"
						font-size="12"
						font-weight="500"
						font-family="var(--font-sans)"
					>
						{node.label}
					</text>
				{/each}
			</svg>
		</div>
	{/if}
</div>

<style>
	.graph-panel {
		width: 100%;
		height: 100%;
		display: flex;
		flex-direction: column;
		overflow: hidden;
		background: var(--color-bg-primary);
		padding: 24px;
	}

	.header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		margin-bottom: 24px;
		gap: 12px;
		flex-wrap: wrap;
		flex-shrink: 0;
	}

	.title {
		margin: 0;
		font-size: 1.25rem;
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.header-actions {
		display: flex;
		gap: 8px;
	}

	.loading-state {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 16px;
		padding: 64px 32px;
		flex: 1;
	}

	.spinner {
		width: 24px;
		height: 24px;
		border: 2px solid var(--color-border);
		border-top-color: var(--color-accent);
		border-radius: 50%;
		animation: spin 0.6s linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}

	.loading-text {
		margin: 0;
		font-size: 0.875rem;
		color: var(--color-text-secondary);
	}

	.empty-state {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 16px;
		padding: 64px 32px;
		text-align: center;
		flex: 1;
	}

	.empty-icon {
		color: var(--color-text-secondary);
		opacity: 0.4;
	}

	.empty-text {
		margin: 0;
		font-size: 0.875rem;
		color: var(--color-text-secondary);
		max-width: 400px;
		line-height: 1.5;
	}

	.no-deps-message {
		margin: 0 0 12px 0;
		font-size: 0.8125rem;
		color: var(--color-text-secondary);
		text-align: center;
		flex-shrink: 0;
	}

	.svg-container {
		flex: 1;
		min-height: 0;
		border-radius: 8px;
		overflow: hidden;
		background: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
	}

	.graph-svg {
		width: 100%;
		height: 100%;
		display: block;
		cursor: grab;
	}

	.graph-svg:active {
		cursor: grabbing;
	}

	@media (max-width: 700px) {
		.graph-panel {
			padding: 12px;
		}

		.header {
			margin-bottom: 16px;
		}
	}
</style>
