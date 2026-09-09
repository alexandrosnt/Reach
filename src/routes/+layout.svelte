<script lang="ts">
	import { applyTheme, themeState, loadInstalledThemes, DARK, LIGHT } from '$lib/state/theme.svelte';
	import type { Snippet } from 'svelte';
	import '../app.css';
	import AppShell from '$lib/components/layout/AppShell.svelte';
	import WelcomeScreen from '$lib/components/setup/WelcomeScreen.svelte';
	import { loadSettings, getSettings, syncTraySettings } from '$lib/state/settings.svelte';
	import { loadAISettings } from '$lib/state/ai.svelte';
	import { initShortcuts, cleanupShortcuts } from '$lib/state/shortcuts.svelte';
	import { startupUpdateCheck, startPeriodicChecks, stopPeriodicChecks } from '$lib/state/updater.svelte';
	import { changeLocale } from '$lib/state/i18n.svelte';
	import { loadSnippets } from '$lib/state/snippets.svelte';
	import { vaultState } from '$lib/state/vault.svelte';
	import * as mcp from '$lib/state/mcp.svelte';
	import { onMount } from 'svelte';

	let { children }: { children: Snippet } = $props();

	const isEditorWindow = typeof window !== 'undefined' && new URLSearchParams(window.location.search).has('editor');
	const settings = getSettings();

	onMount(() => {
		loadSettings();
		// Before the theme effect below resolves settings.themeId: until these
		// are in, themeState.all is just the two built-ins, so an installed
		// theme silently fell back to dark/light on every launch.
		loadInstalledThemes();
		syncTraySettings();
		loadAISettings();
		initShortcuts();
		startupUpdateCheck();
		startPeriodicChecks();

		// The Svelte app has loaded and mounted — signal real readiness. The
		// preloader listener in app.html reacts to this (no timer, no fake delay).
		window.dispatchEvent(new CustomEvent('app-ready'));

		return () => {
			cleanupShortcuts();
			stopPeriodicChecks();
		};
	});

	$effect(() => {
		changeLocale(settings.locale);
	});

	// Load snippets once vault is unlocked
	$effect(() => {
		if (!vaultState.locked) {
			loadSnippets();
		}
	});

	// MCP settings live in the vault, so they can only be restored once it is
	// open — reading them at process start would silently find nothing. This
	// restores the agent, mode and port, and restarts the server if it was
	// explicitly enabled before. Sharing is never restored: bringing the
	// server back honours a decision the user made, whereas bringing a shared
	// session back would be making one for them.
	let mcpRestored = false;
	$effect(() => {
		if (!vaultState.locked && !mcpRestored) {
			mcpRestored = true;
			mcp.restore();
		}
	});

	$effect(() => {
		document.documentElement.style.setProperty('--app-font-size', `${settings.fontSize}px`);
	});

	// Fonts are local/system only — no Google Fonts network fetch. The terminal
	// font resolves from what the OS has installed (JetBrains Mono is bundled
	// via app.css @font-face), falling back to `monospace`.

	// Resolve the setting to an actual theme and apply it. Previously this only
	// toggled a class, so the palette lived in two places (CSS for the app,
	// hardcoded JS for the terminal) and could not be extended past two options.
	$effect(() => {
		const setting = settings.theme;
		const themeId = settings.themeId;

		// An installed theme wins; otherwise fall back to the light/dark/system
		// setting so existing preferences keep working.
		const chosen = themeId ? themeState.all.find((t) => t.id === themeId) : undefined;
		if (chosen) {
			applyTheme(chosen);
			return;
		}

		if (setting === 'system') {
			const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
			applyTheme(prefersDark ? DARK : LIGHT);
		} else {
			applyTheme(setting === 'light' ? LIGHT : DARK);
		}
	});
</script>

{#if isEditorWindow}
	{@render children()}
{:else}
	{#if !settings.setupComplete}
		<WelcomeScreen />
	{/if}

	<AppShell>
		{@render children()}
	</AppShell>
{/if}
