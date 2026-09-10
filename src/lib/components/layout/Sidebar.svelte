<script lang="ts">
	import FileExplorer from '$lib/components/explorer/FileExplorer.svelte';
	import SessionList from '$lib/components/sessions/SessionList.svelte';
	import TunnelManager from '$lib/components/tunnel/TunnelManager.svelte';
	import PluginPanel from '$lib/components/plugin/PluginPanel.svelte';
	import SnippetPanel from '$lib/components/snippets/SnippetPanel.svelte';
	import RecipePanel from '$lib/components/recipes/RecipePanel.svelte';
	import NewIndicator from '$lib/components/shared/NewIndicator.svelte';
	import { sidebarHasNew, markSidebarSeen, newSince } from '$lib/state/whats-new.svelte';
	import { t } from '$lib/state/i18n.svelte';

	type Section = 'sessions' | 'explorer' | 'tunnels' | 'snippets' | 'recipes' | 'plugins';

	const STORAGE_KEY = 'reach-sidebar-width';
	/* `sidebarWidth` is the width of the *panel*, not of the whole sidebar. The
	   rail is added on top when rendering. Keeping the stored value meaning the
	   same thing as before the rail existed means saved widths stay correct
	   instead of every existing user losing 48px of panel. */
	const MIN_WIDTH = 160;
	const MAX_WIDTH = 600;
	const DEFAULT_WIDTH = 240;
	const RAIL_WIDTH = 48;
	const COLLAPSED_WIDTH = RAIL_WIDTH;

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

	let sections = $derived<Array<{ id: Section; label: string; icon: string }>>([
		{
			id: 'sessions',
			label: t('sidebar.sessions'),
			icon: 'M4 6h16M4 10h16M4 14h16M4 18h16'
		},
		{
			id: 'explorer',
			label: t('sidebar.explorer'),
			icon: 'M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z'
		},
		{
			id: 'tunnels',
			label: t('sidebar.tunnels'),
			icon: 'M10 13a5 5 0 007.54.54l3-3a5 5 0 00-7.07-7.07l-1.72 1.71M14 11a5 5 0 00-7.54-.54l-3 3a5 5 0 007.07 7.07l1.71-1.71'
		},
		{
			id: 'snippets',
			label: t('sidebar.snippets'),
			icon: 'M16 4h2a2 2 0 012 2v14a2 2 0 01-2 2H6a2 2 0 01-2-2V6a2 2 0 012-2h2M9 2h6a1 1 0 011 1v1a1 1 0 01-1 1H9a1 1 0 01-1-1V3a1 1 0 011-1z'
		},
		{
			id: 'recipes',
			label: t('sidebar.recipes'),
			// A book: a recipe is something written down to be followed again.
			icon: 'M4 19.5A2.5 2.5 0 016.5 17H20M6.5 2H20v20H6.5A2.5 2.5 0 014 19.5v-15A2.5 2.5 0 016.5 2zM9 7h7M9 11h5'
		},
		{
			id: 'plugins',
			label: t('sidebar.plugins'),
			icon: 'M13 2L3 14h9l-1 8 10-12h-9l1-8z'
		}
	]);

	function handleSectionClick(sectionId: Section): void {
		// Reaching the section is being shown it. Collapsing it again later does
		// not bring the badge back.
		markSidebarSeen(sectionId);

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
			// startW is the panel width, and the pointer moves the sidebar's outer
			// edge, so the delta applies to the panel directly.
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

<aside class="sidebar" class:collapsed class:no-transition={dragging} style:width="{collapsed ? COLLAPSED_WIDTH : sidebarWidth + RAIL_WIDTH}px">
	<nav class="sidebar-nav">
		{#each sections as section (section.id)}
			<button
				class="nav-btn"
				class:active={activeSection === section.id}
				class:has-new={sidebarHasNew(section.id)}
				onclick={() => handleSectionClick(section.id)}
				title={section.label}
				aria-label={section.label}
			>
				<svg width="19" height="19" viewBox="0 0 24 24" fill="none">
					<path
						d={section.icon}
						stroke="currentColor"
						stroke-width="1.8"
						stroke-linecap="round"
						stroke-linejoin="round"
					/>
				</svg>
				{#if sidebarHasNew(section.id)}
					<NewIndicator since={newSince({ kind: 'sidebar', section: section.id })} />
				{/if}
			</button>
		{/each}

		<button
			class="nav-btn rail-toggle"
			onclick={toggleCollapsed}
			title={collapsed ? t('sidebar.expand') : t('sidebar.collapse')}
			aria-label={collapsed ? t('sidebar.expand') : t('sidebar.collapse')}
		>
			<svg width="16" height="16" viewBox="0 0 16 16" fill="none">
				{#if collapsed}
					<path d="M6 3l5 5-5 5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
				{:else}
					<path d="M10 3L5 8l5 5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
				{/if}
			</svg>
		</button>
	</nav>

	<!-- Kept mounted (hidden when collapsed) so collapsing/expanding the sidebar
	     does NOT unmount the active panel and re-read the DB. Panels still mount/
	     unmount on section switch, just not on collapse. -->
	<div class="sidebar-content" class:hidden={collapsed}>
		<div class="section-header">
			{sections.find((s) => s.id === activeSection)?.label ?? ''}
		</div>
		<div class="section-body">
			{#if activeSection === 'sessions'}
				<SessionList />
			{:else if activeSection === 'explorer'}
				<FileExplorer {connectionId} />
			{:else if activeSection === 'tunnels'}
				<TunnelManager {connectionId} />
			{:else if activeSection === 'snippets'}
				<SnippetPanel {connectionId} />
			{:else if activeSection === 'recipes'}
				<RecipePanel />
			{:else if activeSection === 'plugins'}
				<PluginPanel {connectionId} />
			{/if}
		</div>
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
	/* Navigation and panel content are orthogonal concerns. Stacking them meant
	   the five nav buttons (149px) and the collapse row took their cut off the
	   top before the active panel got anything, so the session tree was left
	   ~186px of a 601px column — four rows — and every new tool would have taken
	   another slice. The nav is now a fixed rail beside the panel, so the panel
	   gets the full height and stays that size however many tools exist.
	   COLLAPSED_WIDTH is 48px, which is exactly the rail, so collapsing simply
	   hides the panel and leaves the rail. */
	.sidebar {
		position: relative;
		display: flex;
		flex-direction: row;
		height: 100%;
		background-color: var(--color-bg-secondary);
		border-right: 1px solid var(--color-border);
		overflow: hidden;
		transition: width var(--duration-default) var(--ease-default);
		user-select: none;
		flex-shrink: 0;
	}

	/* The width is remembered in localStorage and applied inline, so a sidebar
	   dragged to 600px on a desktop would still be 600px on a phone, leaving no
	   room for the terminal. Cap it against the viewport instead of the stored
	   value; the inline width still wins whenever it is the smaller of the two. */
	@media (max-width: 900px) {
		.sidebar:not(.collapsed) {
			max-width: 55vw;
		}
	}

	@media (max-width: 600px) {
		.sidebar:not(.collapsed) {
			max-width: 75vw;
		}
	}

	/* On a phone a 240px sidebar beside the terminal left the terminal 149px of a
	   390px screen — 62% of the display spent on navigation. Below 700px the
	   sidebar stops taking a column and overlays the content as a drawer, so the
	   terminal gets the full width and the existing collapse control opens and
	   closes the drawer. */
	@media (max-width: 700px) {
		.sidebar:not(.collapsed) {
			position: absolute;
			inset: 0 auto 0 0;
			z-index: 20;
			max-width: 85vw;
			box-shadow: var(--shadow-elevated);
		}
	}

	/* Finger-sized hit targets where there is no mouse. */
	@media (pointer: coarse) {
		.nav-btn {
			width: 40px;
			height: 40px;
		}

		.resize-handle {
			width: 12px;
		}
	}

	.sidebar-nav {
		flex: 0 0 48px;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--space-1);
		padding: var(--space-2) 0;
		border-right: 1px solid var(--color-border);
	}

	.rail-toggle {
		margin-top: auto;
	}

	/* A dot alone is easy to miss on a 48px rail, so the icon itself takes the
	   accent colour and a tinted ground. Steady, not animated: this has to sit
	   there until someone gets round to it, and a pulsing icon in the corner of
	   a terminal is a reason to close the app. */
	.nav-btn.has-new:not(.active) {
		color: var(--color-accent);
		background: color-mix(in srgb, var(--color-accent) 14%, transparent);
	}

	.nav-btn.has-new:not(.active):hover {
		background: color-mix(in srgb, var(--color-accent) 22%, transparent);
	}

	.nav-btn {
		position: relative;
		display: flex;
		align-items: center;
		justify-content: center;
		flex: 0 0 auto;
		width: 36px;
		height: 36px;
		padding: 0;
		border: none;
		border-radius: var(--radius-btn);
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
		transition: background-color var(--duration-default) var(--ease-default),
			color var(--duration-default) var(--ease-default);
	}

	.nav-btn:hover {
		background-color: var(--color-surface-hover);
		color: var(--color-text-primary);
	}

	.nav-btn.active {
		background-color: var(--color-surface-active);
		color: var(--color-text-primary);
	}

	.nav-btn.active::before {
		content: '';
		position: absolute;
		left: -6px;
		top: 50%;
		transform: translateY(-50%);
		width: 3px;
		height: 20px;
		border-radius: 0 2px 2px 0;
		background-color: var(--color-accent);
	}






	.sidebar-content {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}

	/* Hidden (not unmounted) when the sidebar is collapsed, so the active panel
	   keeps its loaded state instead of re-fetching on expand. */
	.sidebar-content.hidden {
		display: none;
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
		background-color: var(--color-accent, var(--color-accent));
	}

	.resize-overlay {
		position: fixed;
		inset: 0;
		z-index: 9999;
		cursor: col-resize;
	}
</style>
