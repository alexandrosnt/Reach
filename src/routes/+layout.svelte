<script lang="ts">
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
	import { onMount } from 'svelte';

	let { children }: { children: Snippet } = $props();

	const isEditorWindow = typeof window !== 'undefined' && new URLSearchParams(window.location.search).has('editor');
	const settings = getSettings();

	onMount(() => {
		loadSettings();
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

	$effect(() => {
		document.documentElement.style.setProperty('--app-font-size', `${settings.fontSize}px`);
	});

	// Fonts are local/system only — no Google Fonts network fetch. The terminal
	// font resolves from what the OS has installed (JetBrains Mono is bundled
	// via app.css @font-face), falling back to `monospace`.

	$effect(() => {
		const theme = settings.theme;
		const root = document.documentElement;
		root.classList.remove('dark', 'light');

		if (theme === 'system') {
			const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
			root.classList.add(prefersDark ? 'dark' : 'light');
		} else {
			root.classList.add(theme);
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
