<script lang="ts">
	/**
	 * A live log: follows the end until the user scrolls up, then holds still
	 * and offers a way back ("Jump to live"). Search highlights, or filters to
	 * the matching lines. Colour codes are removed so the text can be searched
	 * and copied as it reads. Only the newest lines are drawn, so a chatty
	 * container cannot make the page slow; Download saves everything kept.
	 */
	import { onDestroy } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { t } from '#lib/state/i18n.svelte.js';
	import { addToast } from '#lib/state/toasts.svelte.js';

	interface Props {
		/** Starts the stream; returns how to stop it. Re-run when it changes. */
		start: (onData: (chunk: string) => void) => Promise<() => void>;
		/** File name offered by Download, without extension. */
		name: string;
	}

	let { start, name }: Props = $props();

	/** Lines kept in memory; older ones are dropped. */
	const KEEP = 50_000;
	/** Lines drawn at once. */
	const SHOW = 4_000;

	let lines = $state<string[]>([]);
	let partial = '';
	let follow = $state(true);
	/** Paused: the view holds still, as it was, while lines keep arriving. */
	let frozen = $state<string[] | null>(null);
	let wrap = $state(true);
	let query = $state('');
	let onlyMatches = $state(false);
	let error = $state('');
	let box: HTMLDivElement | undefined = $state();
	let stop: (() => void) | null = null;

	// eslint-disable-next-line no-control-regex
	const ANSI = /\x1b\[[0-9;?]*[ -/]*[@-~]|\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)|\r/g;

	/**
	 * Starts a line that is Reach's own note (a stream that ended, a file
	 * shown as it is now), not the log's. The backend takes this mark out of
	 * whatever a container prints, so a log cannot pass for a note.
	 */
	const NOTE = '\u0001';

	function push(chunk: string): void {
		const text = partial + chunk.replace(ANSI, '');
		const parts = text.split('\n').filter((p, i, all) => !(p === '' && all[i + 1]?.startsWith(NOTE)));
		partial = parts.pop() ?? '';
		if (parts.length === 0) return;
		const next = lines.length + parts.length > KEEP ? lines.slice(lines.length + parts.length - KEEP) : lines.slice();
		next.push(...parts);
		lines = next;
	}

	$effect(() => {
		const begin = start;
		lines = [];
		partial = '';
		error = '';
		follow = true;
		frozen = null;
		let cancelled = false;
		begin(push)
			.then((s) => {
				if (cancelled) s();
				else stop = s;
			})
			.catch((e) => (error = String(e)));
		return () => {
			cancelled = true;
			stop?.();
			stop = null;
		};
	});

	onDestroy(() => stop?.());

	let needle = $derived(query.trim().toLowerCase());
	let shown = $derived.by(() => {
		const source = frozen ?? lines;
		const all = needle && onlyMatches ? source.filter((l) => l.toLowerCase().includes(needle)) : source;
		return all.length > SHOW ? all.slice(all.length - SHOW) : all;
	});
	let hiddenCount = $derived.by(() => {
		const source = frozen ?? lines;
		return (needle && onlyMatches ? source.filter((l) => l.toLowerCase().includes(needle)).length : source.length) - shown.length;
	});
	/** Lines that arrived while paused, shown on resume. */
	let arrived = $derived(frozen ? Math.max(0, lines.length - frozen.length) : 0);
	let matchCount = $derived(needle ? lines.reduce((n, l) => n + (l.toLowerCase().includes(needle) ? 1 : 0), 0) : 0);

	// Keep to the end while following.
	$effect(() => {
		void shown.length;
		if (follow && !frozen && box) queueMicrotask(() => box && (box.scrollTop = box.scrollHeight));
	});

	function onScroll(): void {
		if (!box || frozen) return;
		const atEnd = box.scrollHeight - box.scrollTop - box.clientHeight < 24;
		if (follow !== atEnd) follow = atEnd;
	}

	function togglePause(): void {
		if (frozen) {
			jumpToLive();
		} else {
			frozen = lines.slice();
			follow = false;
		}
	}

	function jumpToLive(): void {
		frozen = null;
		follow = true;
		if (box) box.scrollTop = box.scrollHeight;
	}

	function parts(line: string): { text: string; hit: boolean }[] {
		if (!needle) return [{ text: line, hit: false }];
		const out: { text: string; hit: boolean }[] = [];
		const low = line.toLowerCase();
		let i = 0;
		for (;;) {
			const at = low.indexOf(needle, i);
			if (at < 0) {
				out.push({ text: line.slice(i), hit: false });
				return out;
			}
			if (at > i) out.push({ text: line.slice(i, at), hit: false });
			out.push({ text: line.slice(at, at + needle.length), hit: true });
			i = at + needle.length;
		}
	}

	/** The backend opens the Save dialog, so the page never names a path. */
	async function download(): Promise<void> {
		try {
			const content = lines.filter((l) => !l.startsWith(NOTE)).join('\n') + (partial ? '\n' + partial : '') + '\n';
			if (await invoke<boolean>('devops_save_text', { defaultName: `${name}.log`, content })) addToast(t('logs.saved'), 'success');
		} catch (e) {
			addToast(String(e), 'error');
		}
	}
</script>

<div class="viewer">
	<div class="bar">
		<input class="search" type="search" placeholder={t('logs.search')} aria-label={t('logs.search')} bind:value={query} />
		{#if needle}
			<span class="count">{t('logs.matches', { count: matchCount })}</span>
			<label class="check"><input type="checkbox" bind:checked={onlyMatches} />{t('logs.only_matching')}</label>
		{/if}
		<span class="spacer"></span>
		<label class="check"><input type="checkbox" bind:checked={wrap} />{t('logs.wrap')}</label>
		<!-- In a narrow pane the words give way to icons; the names stay for tooltips and screen readers. -->
		<button type="button" class="tool" aria-pressed={!!frozen} onclick={togglePause} title={frozen ? t('logs.resume') : t('logs.pause')} aria-label={frozen ? t('logs.resume') : t('logs.pause')}>
			<span class="ico" aria-hidden="true">{frozen ? '▶' : '⏸'}</span><span class="txt">{frozen ? t('logs.resume') : t('logs.pause')}</span>
		</button>
		<button type="button" class="tool" onclick={() => (lines = [])} title={t('logs.clear')} aria-label={t('logs.clear')}>
			<span class="ico" aria-hidden="true">⌫</span><span class="txt">{t('logs.clear')}</span>
		</button>
		<button type="button" class="tool" onclick={download} title={t('logs.download')} aria-label={t('logs.download')}>
			<span class="ico" aria-hidden="true">⤓</span><span class="txt">{t('logs.download')}</span>
		</button>
	</div>

	<div class="text" class:wrap bind:this={box} onscroll={onScroll} role="log" aria-live="off">
		{#if error}
			<div class="error">{error}</div>
		{:else if lines.length === 0}
			<div class="empty">{t('logs.waiting')}</div>
		{/if}
		{#if hiddenCount > 0}
			<div class="older">{t('logs.older_hidden', { count: hiddenCount })}</div>
		{/if}
		{#each shown as line, i (i)}
			{#if line.startsWith(NOTE)}
				<div class="note" role="note">{line.slice(1)}</div>
			{:else}
				<div class="line">{#each parts(line) as p, j (j)}{#if p.hit}<mark>{p.text}</mark>{:else}{p.text}{/if}{/each}</div>
			{/if}
		{/each}
	</div>

	{#if frozen}
		<button type="button" class="live" onclick={jumpToLive}>{t('logs.paused_new', { count: arrived })} ↓</button>
	{:else if !follow}
		<button type="button" class="live" onclick={jumpToLive}>{t('logs.jump_to_live')} ↓</button>
	{/if}
</div>

<style>
	.viewer {
		position: relative;
		display: flex;
		flex-direction: column;
		min-height: 0;
		height: 100%;
		container-type: inline-size;
	}

	/* A narrow pane keeps the tools on one line: they tighten, the search gives way. */
	.tool .ico {
		display: none;
	}

	@container (max-width: 440px) {
		.bar {
			gap: 4px;
		}

		.tool {
			min-width: 28px;
			padding: 0 6px;
		}

		.tool .ico {
			display: inline;
		}

		.tool .txt {
			display: none;
		}

		.search {
			min-width: 60px;
		}
	}

	.bar {
		display: flex;
		align-items: center;
		gap: 8px;
		flex-wrap: wrap;
		padding: 6px 8px;
		border-bottom: 1px solid var(--color-border);
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
	}

	.search {
		/* Shrinks first, so the buttons stay on one line in a narrow pane. */
		flex: 1 1 100px;
		min-width: 80px;
		max-width: 320px;
		padding: 5px 8px;
		border-radius: var(--radius-btn);
		border: 1px solid var(--color-border);
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		font: inherit;
	}

	.search:focus {
		outline: none;
		border-color: var(--color-accent);
	}

	.spacer {
		flex: 1;
	}

	.check {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		cursor: pointer;
		white-space: nowrap;
	}

	.tool {
		min-height: 28px;
		padding: 0 8px;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		background: none;
		color: var(--color-text-primary);
		font: inherit;
		cursor: pointer;
	}

	.tool:hover {
		border-color: var(--color-accent);
	}

	.text {
		flex: 1;
		min-height: 0;
		overflow: auto;
		padding: 6px 10px 28px;
		font-family: var(--font-mono);
		/* A log shows the characters it holds: no ==> drawn as an arrow. */
		font-variant-ligatures: none;
		font-size: 12px;
		line-height: 1.55;
		color: var(--color-text-primary);
		background: var(--color-bg-primary);
		user-select: text;
	}

	.line {
		white-space: pre;
	}

	.wrap .line {
		white-space: pre-wrap;
		overflow-wrap: anywhere;
	}

	mark {
		background: color-mix(in srgb, var(--color-warning) 45%, transparent);
		color: inherit;
		border-radius: 2px;
	}

	.note {
		margin: 6px 0;
		padding: 4px 8px;
		border-left: 2px solid var(--color-border);
		color: var(--color-text-secondary);
		font-family: var(--font-sans);
		font-size: var(--text-xs);
		font-style: italic;
		white-space: normal;
	}

	.empty,
	.older {
		color: var(--color-text-tertiary);
		font-family: var(--font-sans);
		padding: 4px 0;
	}

	.error {
		color: var(--color-danger);
		font-family: var(--font-sans);
	}

	.live {
		position: absolute;
		left: 50%;
		bottom: 12px;
		transform: translateX(-50%);
		padding: 6px 14px;
		border: none;
		border-radius: 999px;
		background: var(--color-accent);
		color: #fff;
		font: inherit;
		font-size: var(--text-xs);
		font-weight: 600;
		box-shadow: var(--shadow-elevated);
		cursor: pointer;
	}
</style>
