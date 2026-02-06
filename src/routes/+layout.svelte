<script lang="ts">
	import type { Snippet } from 'svelte';
	import '../app.css';
	import AppShell from '$lib/components/layout/AppShell.svelte';
	import { loadSettings, getSettings, syncTraySettings } from '$lib/state/settings.svelte';
	import { loadAISettings } from '$lib/state/ai.svelte';
	import { initShortcuts, cleanupShortcuts } from '$lib/state/shortcuts.svelte';
	import { startupUpdateCheck, startPeriodicChecks, stopPeriodicChecks } from '$lib/state/updater.svelte';

	let { children }: { children: Snippet } = $props();

	const settings = getSettings();

	$effect(() => {
		loadSettings();
		syncTraySettings();
		loadAISettings();
		initShortcuts();
		startupUpdateCheck();
		startPeriodicChecks();

		// Dismiss the preloader once the app is mounted
		const preloader = document.getElementById('preloader');
		if (preloader) {
			preloader.classList.add('hidden');
			setTimeout(() => preloader.remove(), 500);
		}

		return () => {
			cleanupShortcuts();
			stopPeriodicChecks();
		};
	});

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

<AppShell>
	{@render children()}
</AppShell>
