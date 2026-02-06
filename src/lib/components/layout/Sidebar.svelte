<script lang="ts">
	import FileExplorer from '$lib/components/explorer/FileExplorer.svelte';
	import SessionList from '$lib/components/sessions/SessionList.svelte';
	import PlaybookList from '$lib/components/playbook/PlaybookList.svelte';
	import TunnelManager from '$lib/components/tunnel/TunnelManager.svelte';
	import SnippetList from '$lib/components/snippets/SnippetList.svelte';

	type Section = 'sessions' | 'explorer' | 'playbooks' | 'tunnels' | 'snippets';

	const STORAGE_KEY = 'reach-sidebar-width';
	const MIN_WIDTH = 160;
	const MAX_WIDTH = 600;
	const DEFAULT_WIDTH = 240;
	const COLLAPSED_WIDTH = 48;

	interface Props {
		collapsed: boolean;
		connectionId?: string;
	}

	let { collapsed = $bindable(false), connectionId }: Props = $props();

	let activeSection = $state<Section>('sessions');
	let sidebarWidth = $state(loadWidth());
	let dragging = $state(false);

	function loadWidth(): number {
		try {
			const saved = localStorage.getItem(STORAGE_KEY);
			if (saved) {
				const n = parseInt(saved, 10);
				if (n >= MIN_WIDTH && n <= MAX_WIDTH) return n;
			}
		} catch {}
		return DEFAULT_WIDTH;
	}

	function saveWidth(w: number): void {
		try {
			localStorage.setItem(STORAGE_KEY, String(w));
		} catch {}
	}

	const sections: Array<{ id: Section; label: string; icon: string }> = [
		{
			id: 'sessions',
			label: 'Sessions',
			icon: 'M4 6h16M4 10h16M4 14h16M4 18h16'
		},
		{
			id: 'explorer',
			label: 'File Explorer',
			icon: 'M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z'
		},
		{
			id: 'playbooks',
			label: 'Playbooks',
			icon: 'M5 3l14 9-14 9V3z'
		},
		{
			id: 'tunnels',
			label: 'Tunnels',
			icon: 'M10 13a5 5 0 007.54.54l3-3a5 5 0 00-7.07-7.07l-1.72 1.71M14 11a5 5 0 00-7.54-.54l-3 3a5 5 0 007.07 7.07l1.71-1.71'
		},
		{
			id: 'snippets',
			label: 'Snippets',
			icon: 'M8 6h13M8 12h13M8 18h13M3 6h.01M3 12h.01M3 18h.01'
		}
	];

	function handleSectionClick(sectionId: Section): void {
		if (collapsed) {
			collapsed = false;
			activeSection = sectionId;
		} else if (activeSection === sectionId) {
			collapsed = true;
		} else {
			activeSection = sectionId;
		}
	}

	function toggleCollapsed(): void {
		collapsed = !collapsed;
	}

	function startResize(e: MouseEvent): void {
		e.preventDefault();
		dragging = true;
		const startX = e.clientX;
		const startW = sidebarWidth;

		function onMove(ev: MouseEvent): void {
			const w = Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, startW + ev.clientX - startX));
			sidebarWidth = w;
		}

		function onUp(): void {
			dragging = false;
			document.removeEventListener('mousemove', onMove);
			document.removeEventListener('mouseup', onUp);
			saveWidth(sidebarWidth);
		}

		document.addEventListener('mousemove', onMove);
		document.addEventListener('mouseup', onUp);
	}
</script>

<aside class="sidebar" class:no-transition={dragging} style:width="{collapsed ? COLLAPSED_WIDTH : sidebarWidth}px">
	<nav class="sidebar-nav">
		{#each sections as section (section.id)}
			<button
				class="nav-btn"
				class:active={activeSection === section.id && !collapsed}
				onclick={() => handleSectionClick(section.id)}
				title={section.label}
				aria-label={section.label}
			>
				<svg width="15" height="15" viewBox="0 0 24 24" fill="none">
					<path
						d={section.icon}
						stroke="currentColor"
						stroke-width="1.8"
						stroke-linecap="round"
						stroke-linejoin="round"
					/>
				</svg>

				{#if !collapsed}
					<span class="nav-label">{section.label}</span>
				{/if}
			</button>
		{/each}
	</nav>

	{#if !collapsed}
		<div class="sidebar-content">
			<div class="section-header">
				{sections.find((s) => s.id === activeSection)?.label ?? ''}
			</div>
			<div class="section-body">
				{#if activeSection === 'sessions'}
					<SessionList />
				{:else if activeSection === 'explorer'}
					<FileExplorer {connectionId} />
				{:else if activeSection === 'playbooks'}
					<PlaybookList {connectionId} />
				{:else if activeSection === 'tunnels'}
					<TunnelManager {connectionId} />
				{:else if activeSection === 'snippets'}
					<SnippetList {connectionId} />
				{/if}
			</div>
		</div>
	{/if}

	<div class="sidebar-footer">
		<button
			class="toggle-btn"
			onclick={toggleCollapsed}
			aria-label={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
		>
			<svg width="16" height="16" viewBox="0 0 16 16" fill="none">
				{#if collapsed}
					<path d="M6 3l5 5-5 5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
				{:else}
					<path d="M10 3L5 8l5 5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
				{/if}
			</svg>
		</button>
	</div>

	{#if !collapsed}
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div class="resize-handle" onmousedown={startResize}></div>
	{/if}
</aside>

{#if dragging}
	<!-- Overlay to prevent iframe/webview stealing mouse events during drag -->
	<div class="resize-overlay"></div>
{/if}

<style>
	.sidebar {
		position: relative;
		display: flex;
		flex-direction: column;
		height: 100%;
		background-color: var(--color-bg-secondary);
		border-right: 1px solid var(--color-border);
		overflow: hidden;
		transition: width var(--duration-default) var(--ease-default);
		user-select: none;
		flex-shrink: 0;
	}

	.sidebar-nav {
		display: flex;
		flex-direction: column;
		gap: 1px;
		padding: 6px 5px;
	}

	.nav-btn {
		display: flex;
		align-items: center;
		gap: 8px;
		width: 100%;
		padding: 5px 7px;
		border: none;
		border-radius: 6px;
		background: transparent;
		color: var(--color-text-secondary);
		font-family: var(--font-sans);
		font-size: 0.6875rem;
		cursor: pointer;
		white-space: nowrap;
		overflow: hidden;
		transition: background-color var(--duration-default) var(--ease-default),
			color var(--duration-default) var(--ease-default);
	}

	.nav-btn:hover {
		background-color: rgba(255, 255, 255, 0.06);
		color: var(--color-text-primary);
	}

	.nav-btn.active {
		background-color: rgba(255, 255, 255, 0.08);
		color: var(--color-text-primary);
	}

	.nav-label {
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.sidebar-content {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}

	.section-header {
		padding: 5px 12px;
		font-size: 0.625rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--color-text-secondary);
		border-top: 1px solid var(--color-border);
	}

	.section-body {
		flex: 1;
		overflow-y: auto;
		padding: 2px 6px;
	}

	.sidebar-footer {
		padding: 6px;
		border-top: 1px solid var(--color-border);
	}

	.toggle-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 100%;
		height: 28px;
		border: none;
		border-radius: var(--radius-btn);
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
		transition: background-color var(--duration-default) var(--ease-default),
			color var(--duration-default) var(--ease-default);
	}

	.toggle-btn:hover {
		background-color: rgba(255, 255, 255, 0.06);
		color: var(--color-text-primary);
	}

	.sidebar.no-transition {
		transition: none;
	}

	.resize-handle {
		position: absolute;
		top: 0;
		right: 0;
		width: 4px;
		height: 100%;
		cursor: col-resize;
		z-index: 10;
		transition: background-color 0.15s ease;
	}

	.resize-handle:hover,
	.resize-handle:active {
		background-color: var(--color-accent, #0a84ff);
	}

	.resize-overlay {
		position: fixed;
		inset: 0;
		z-index: 9999;
		cursor: col-resize;
	}
</style>
