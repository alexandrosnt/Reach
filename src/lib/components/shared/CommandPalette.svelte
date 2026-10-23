<script lang="ts">
	/**
	 * One box to reach anything: a session to connect, an open tab, a page,
	 * or an action. Ctrl+Shift+P (⇧⌘P on a Mac), or the search button in the
	 * title bar, which is how a phone opens it.
	 *
	 * Sessions are connected by the session list (see palette.svelte.ts), so
	 * every prompt and check a click on the session would have still applies.
	 */
	import { tick } from 'svelte';
	import { isPaletteOpen, closePalette, requestSessionAction } from '$lib/state/palette.svelte';
	import { getTabs, activateTab, createTab } from '$lib/state/tabs.svelte';
	import { getActivePage, setActivePage } from '$lib/state/navigation.svelte';
	import { getEnabledDevopsTools } from '$lib/state/devops.svelte';
	import { openSettings, shortcutLabel } from '$lib/state/shortcuts.svelte';
	import { lock, vaultState } from '$lib/state/vault.svelte';
	import { sessionList, type SessionConfig } from '$lib/ipc/sessions';
	import { t } from '$lib/state/i18n.svelte';

	interface Item {
		id: string;
		group: string;
		label: string;
		detail?: string;
		hint?: string;
		/** Text matched besides the label. */
		keywords?: string;
		run: () => void;
	}

	let query = $state('');
	let selected = $state(0);
	let sessions = $state<SessionConfig[]>([]);
	let input = $state<HTMLInputElement>();
	let list = $state<HTMLDivElement>();

	// Never over a held vault: it lists every session's name and host.
	let open = $derived(isPaletteOpen() && !vaultState.held);

	// Fresh each time it opens: sessions change, and a query left from last
	// time would hide what someone came for.
	$effect(() => {
		if (!open) return;
		query = '';
		selected = 0;
		void sessionList()
			.then((s) => (sessions = s))
			.catch(() => (sessions = []));
		void tick().then(() => input?.focus());
	});

	let items = $derived.by((): Item[] => {
		const out: Item[] = [];
		for (const s of sessions) {
			out.push({
				id: `session:${s.id}`,
				group: t('palette.group_sessions'),
				label: s.name,
				detail: `${s.username ? s.username + '@' : ''}${s.host}`,
				keywords: s.tags.join(' '),
				run: () => requestSessionAction({ kind: 'connect', sessionId: s.id })
			});
		}
		for (const tab of getTabs()) {
			out.push({
				id: `tab:${tab.id}`,
				group: t('palette.group_tabs'),
				label: tab.title,
				detail: tab.sessionName && tab.sessionName !== tab.title ? tab.sessionName : undefined,
				run: () => {
					setActivePage('terminal');
					activateTab(tab.id);
				}
			});
		}
		const pages = [
			{ id: 'terminal' as const, label: () => t('nav.terminal') },
			...getEnabledDevopsTools().map((d) => ({ id: d.page, label: d.label }))
		];
		if (pages.length > 1) {
			for (const p of pages) {
				if (p.id === getActivePage()) continue;
				out.push({
					id: `page:${p.id}`,
					group: t('palette.group_pages'),
					label: p.label(),
					run: () => setActivePage(p.id)
				});
			}
		}
		out.push(
			{
				id: 'action:new-tab',
				group: t('palette.group_actions'),
				label: t('terminal.new_tab'),
				hint: shortcutLabel('T', { ctrl: true }),
				run: () => {
					setActivePage('terminal');
					createTab('local');
				}
			},
			{
				id: 'action:new-session',
				group: t('palette.group_actions'),
				label: t('session.new_session'),
				hint: shortcutLabel('N', { ctrl: true }),
				run: () => requestSessionAction({ kind: 'new-session' })
			},
			{
				id: 'action:quick-connect',
				group: t('palette.group_actions'),
				label: t('session.quick_connect'),
				hint: shortcutLabel('N', { ctrl: true, shift: true }),
				run: () => requestSessionAction({ kind: 'quick-connect' })
			},
			{
				id: 'action:settings',
				group: t('palette.group_actions'),
				label: t('titlebar.open_settings'),
				hint: shortcutLabel(',', { ctrl: true }),
				run: openSettings
			},
			{
				id: 'action:lock',
				group: t('palette.group_actions'),
				label: t('palette.lock_vault'),
				run: () => void lock()
			}
		);
		return out;
	});

	/** How well `text` matches: a word start beats the middle, the middle
	 *  beats letters in order (`wb1` finds web-1). -1 is no match. */
	function score(text: string, q: string): number {
		const s = text.toLowerCase();
		const at = s.indexOf(q);
		if (at === 0) return 300;
		if (at > 0) return /[\s\-_.@/]/.test(s[at - 1]) ? 250 : 200 - Math.min(at, 100);
		let i = 0;
		for (const ch of s) if (ch === q[i]) i++;
		return i === q.length ? 50 : -1;
	}

	let results = $derived.by((): Item[] => {
		const q = query.trim().toLowerCase();
		if (!q) {
			// Without a query, what people reach for most: tabs, actions,
			// pages, and a first few sessions.
			const tabsEtc = items.filter((i) => !i.id.startsWith('session:'));
			return [...tabsEtc.filter((i) => i.id.startsWith('tab:')), ...items.filter((i) => i.id.startsWith('session:')).slice(0, 6), ...tabsEtc.filter((i) => !i.id.startsWith('tab:'))];
		}
		return items
			.map((i) => ({ i, s: Math.max(score(i.label, q), i.detail ? score(i.detail, q) - 20 : -1, i.keywords ? score(i.keywords, q) - 30 : -1) }))
			.filter((r) => r.s >= 0)
			.sort((a, b) => b.s - a.s)
			.slice(0, 50)
			.map((r) => r.i);
	});

	$effect(() => {
		// A new query starts at the top.
		void results;
		selected = 0;
	});

	function run(item: Item | undefined): void {
		if (!item) return;
		closePalette();
		item.run();
	}

	async function onkeydown(e: KeyboardEvent): Promise<void> {
		if (e.key === 'Escape') {
			e.preventDefault();
			closePalette();
		} else if (e.key === 'ArrowDown' || (e.key === 'Tab' && !e.shiftKey)) {
			e.preventDefault();
			selected = (selected + 1) % Math.max(results.length, 1);
			await keepInView();
		} else if (e.key === 'ArrowUp' || (e.key === 'Tab' && e.shiftKey)) {
			e.preventDefault();
			selected = (selected - 1 + results.length) % Math.max(results.length, 1);
			await keepInView();
		} else if (e.key === 'Enter') {
			e.preventDefault();
			run(results[selected]);
		}
	}

	async function keepInView(): Promise<void> {
		await tick();
		list?.querySelector('.row.selected')?.scrollIntoView({ block: 'nearest' });
	}
</script>

{#if open}
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<div class="scrim" onclick={closePalette}></div>
	<div class="palette" role="dialog" aria-modal="true" aria-label={t('palette.title')}>
		<div class="search">
			<svg width="16" height="16" viewBox="0 0 24 24" fill="none" aria-hidden="true">
				<circle cx="11" cy="11" r="7" stroke="currentColor" stroke-width="1.8" />
				<path d="M20 20l-3.5-3.5" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" />
			</svg>
			<input
				bind:this={input}
				bind:value={query}
				{onkeydown}
				placeholder={t('palette.placeholder')}
				aria-label={t('palette.placeholder')}
				aria-controls="palette-list"
				aria-activedescendant={results[selected] ? `palette-${selected}` : undefined}
				autocomplete="off"
				spellcheck="false"
			/>
			<kbd class="esc">Esc</kbd>
		</div>
		<div class="list" id="palette-list" role="listbox" bind:this={list}>
			{#each results as item, i (item.id)}
				{#if i === 0 || results[i - 1].group !== item.group}
					<div class="group">{item.group}</div>
				{/if}
				<!-- svelte-ignore a11y_click_events_have_key_events -->
				<div
					id="palette-{i}"
					class="row"
					class:selected={i === selected}
					role="option"
					tabindex="-1"
					aria-selected={i === selected}
					onclick={() => run(item)}
					onmousemove={() => (selected = i)}
				>
					<span class="label">{item.label}</span>
					{#if item.detail}<span class="detail">{item.detail}</span>{/if}
					{#if item.hint}<kbd>{item.hint}</kbd>{/if}
				</div>
			{:else}
				<div class="empty">{t('palette.empty')}</div>
			{/each}
		</div>
		<div class="foot">{t('palette.help')}</div>
	</div>
{/if}

<style>
	.scrim {
		position: fixed;
		inset: 0;
		z-index: 900;
		background: rgba(0, 0, 0, 0.35);
	}

	/* Near the top, where the eye already is, clear of a phone's status bar
	   and camera like the rest of the app. */
	.palette {
		position: fixed;
		z-index: 901;
		top: calc(var(--safe-top) + min(12dvh, 96px));
		left: 50%;
		transform: translateX(-50%);
		width: min(600px, calc(100vw - var(--safe-left) - var(--safe-right) - 24px));
		max-height: calc(100dvh - var(--safe-top) - var(--safe-bottom) - min(12dvh, 96px) - 16px);
		display: flex;
		flex-direction: column;
		background: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-card);
		box-shadow: var(--shadow-elevated);
		overflow: hidden;
		animation: drop var(--duration-default) var(--ease-default);
	}

	@keyframes drop {
		from {
			opacity: 0;
			transform: translate(-50%, -6px);
		}
	}

	.search {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-3) var(--space-4);
		border-bottom: 1px solid var(--color-border);
		color: var(--color-text-secondary);
	}

	.search input {
		flex: 1;
		min-width: 0;
		border: none;
		outline: none;
		background: transparent;
		color: var(--color-text-primary);
		font-family: var(--font-sans);
		font-size: var(--text-md);
	}

	.search input::placeholder {
		color: var(--color-text-tertiary);
	}

	.list {
		overflow-y: auto;
		padding: var(--space-1);
		max-height: 420px;
	}

	.group {
		padding: var(--space-2) var(--space-3) var(--space-1);
		font-size: var(--text-2xs);
		font-weight: 600;
		letter-spacing: 0.04em;
		text-transform: uppercase;
		color: var(--color-text-tertiary);
	}

	.row {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		min-height: 36px;
		padding: 0 var(--space-3);
		border-radius: var(--radius-btn);
		color: var(--color-text-primary);
		font-size: var(--text-sm);
		cursor: pointer;
	}

	.row.selected {
		background: var(--color-surface-hover);
	}

	.label {
		flex-shrink: 0;
		max-width: 60%;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.detail {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		color: var(--color-text-secondary);
		font-family: var(--font-mono);
		font-size: var(--text-xs);
	}

	kbd {
		margin-left: auto;
		flex-shrink: 0;
		padding: 1px 6px;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-sm);
		font-family: var(--font-mono);
		font-size: var(--text-2xs);
		color: var(--color-text-tertiary);
	}

	.empty {
		padding: var(--space-5) var(--space-4);
		text-align: center;
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
	}

	.foot {
		padding: var(--space-2) var(--space-4);
		border-top: 1px solid var(--color-border);
		font-size: var(--text-2xs);
		color: var(--color-text-tertiary);
	}

	/* A phone has no keyboard hints to give. */
	@media (pointer: coarse) {
		.foot,
		.esc {
			display: none;
		}

		.row {
			min-height: 44px;
		}
	}
</style>
