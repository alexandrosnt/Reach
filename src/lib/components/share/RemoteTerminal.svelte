<!--
	The other person's terminal.

	An xterm instance fed by their `out` frames. It is sized to *their*
	terminal, not to this pane: a mirror must have the same column count as
	its source or every wrapped line renders wrong. So the pane scrolls if
	the geometry does not fit, rather than reflowing what cannot be reflowed.

	Keystrokes are forwarded only if they said read-write, and even then it
	is their machine that decides — this is a courtesy filter, not the
	enforcement. The enforcement is theirs, on the receiving end of `in`.
-->
<script lang="ts">
	import { onMount } from 'svelte';
	import { Terminal } from '@xterm/xterm';
	import { Unicode11Addon } from '@xterm/addon-unicode11';
	import '@xterm/xterm/css/xterm.css';
	import { themeState } from '$lib/state/theme.svelte';
	import { getSettings } from '$lib/state/settings.svelte';
	import { t } from '$lib/state/i18n.svelte';
	import * as share from '$lib/state/share.svelte';

	let container: HTMLDivElement | undefined = $state();
	let term: Terminal | undefined;

	let session = $derived(share.getSession());
	let remote = $derived(session?.remote ?? null);
	let canType = $derived(remote?.mode === 'read-write');

	onMount(() => {
		if (!container) return;
		const settings = getSettings();
		term = new Terminal({
			fontFamily: settings.fontFamily || 'monospace',
			fontSize: settings.fontSize ?? 14,
			cursorBlink: false,
			cursorStyle: 'bar',
			scrollback: 5000,
			allowProposedApi: true,
			// Their size, not ours. Updated below when their hello or a
			// resize arrives.
			cols: remote?.cols ?? 80,
			rows: remote?.rows ?? 24,
			theme: { ...themeState.current.terminal }
		});
		term.loadAddon(new Unicode11Addon());
		term.unicode.activeVersion = '11';
		term.open(container);

		const encoder = new TextEncoder();
		term.onData((data) => {
			share.sendInput(encoder.encode(data));
		});

		const unsubscribe = share.subscribeRemoteOutput((data) => term?.write(data));

		return () => {
			unsubscribe();
			term?.dispose();
			term = undefined;
		};
	});

	// Follow their geometry. This is the whole reason the pane exists as a
	// separate thing rather than a second tab.
	$effect(() => {
		const cols = remote?.cols;
		const rows = remote?.rows;
		if (term && cols && rows && (term.cols !== cols || term.rows !== rows)) {
			term.resize(cols, rows);
		}
	});

	// xterm keeps its palette in JS, so a theme change must be pushed in.
	$effect(() => {
		const theme = themeState.current.terminal;
		if (term) term.options.theme = { ...theme };
	});
</script>

<div class="remote-pane">
	<header class="remote-head">
		<span class="remote-dot" class:live={session?.phase === 'connected'}></span>
		<span class="remote-title">{remote?.title || t('sharing.remote_title')}</span>
		<span class="remote-mode" class:rw={canType}>
			{canType ? t('sharing.read_write') : t('sharing.read_only')}
		</span>
		{#if remote}
			<span class="remote-size">{remote.cols}×{remote.rows}</span>
		{/if}
	</header>

	<div class="remote-body" class:view-only={!canType}>
		<div class="remote-term" bind:this={container}></div>
	</div>

	{#if !canType}
		<p class="remote-hint">{t('sharing.remote_view_only')}</p>
	{/if}
</div>

<style>
	.remote-pane {
		display: flex;
		flex-direction: column;
		min-width: 0;
		min-height: 0;
		background: var(--color-bg-primary);
	}

	.remote-head {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		padding: 4px var(--space-3);
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
		background: var(--color-bg-secondary);
		border-bottom: 1px solid var(--color-border);
	}

	.remote-dot {
		width: 7px;
		height: 7px;
		border-radius: 50%;
		background: var(--color-text-tertiary);
	}

	.remote-dot.live {
		background: var(--color-success);
	}

	.remote-title {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-family: var(--font-mono);
		color: var(--color-text-primary);
	}

	.remote-mode {
		padding: 1px 7px;
		font-size: var(--text-2xs);
		font-weight: 600;
		letter-spacing: 0.03em;
		text-transform: uppercase;
		color: var(--color-text-tertiary);
		border: 1px solid var(--color-border);
		border-radius: 999px;
	}

	.remote-mode.rw {
		color: var(--color-accent);
		border-color: color-mix(in srgb, var(--color-accent) 50%, var(--color-border));
	}

	.remote-size {
		font-family: var(--font-mono);
		font-size: var(--text-2xs);
		color: var(--color-text-tertiary);
	}

	.remote-body {
		flex: 1;
		min-height: 0;
		/* Their geometry may exceed ours; scroll rather than reflow. */
		overflow: auto;
		padding: 4px;
	}

	.remote-body.view-only :global(.xterm-cursor-layer) {
		opacity: 0.4;
	}

	.remote-term {
		width: max-content;
		height: 100%;
	}

	.remote-hint {
		margin: 0;
		padding: 3px var(--space-3);
		font-size: var(--text-2xs);
		color: var(--color-text-tertiary);
		background: var(--color-bg-secondary);
		border-top: 1px solid var(--color-border);
	}
</style>
