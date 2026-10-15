<script lang="ts">
	/**
	 * Pick an entry from a password manager. Lists metadata only (name, user
	 * name, address); the secret itself is fetched by the backend at connect
	 * time and never comes here.
	 *
	 * Opened on top of another dialog (the session editor), so Escape is
	 * caught here before the dialogs below see it: one press closes this
	 * picker, not the editor and whatever was typed in it.
	 *
	 * Listing is a full sign-in to the password manager, so at most one request
	 * is out at a time. The first fetches every entry (the backend caches it);
	 * while it runs, typing only filters what is already shown. If the whole
	 * list fitted in that first answer, searching is done here; otherwise the
	 * backend is asked again (from its cache), one request at a time.
	 */
	import { tick, untrack } from 'svelte';
	import Modal from '$lib/components/shared/Modal.svelte';
	import Button from '$lib/components/shared/Button.svelte';
	import { pwmanagerList, type EntrySummary, type ProviderStatus } from '$lib/ipc/pwmanager';
	import { t } from '$lib/state/i18n.svelte';

	interface Props {
		open: boolean;
		/** Providers that can be picked from; the first is shown first. */
		providers: ProviderStatus[];
		onpick: (entry: EntrySummary, provider: ProviderStatus) => void;
		onclose: () => void;
	}

	let { open, providers, onpick, onclose }: Props = $props();

	/** The backend returns at most this many; more means "search instead". */
	const LIST_CAP = 500;
	const DEBOUNCE_MS = 250;

	const uid = `pwpick-${Math.random().toString(36).slice(2, 9)}`;

	let providerId = $state('');
	let query = $state('');
	let entries = $state<EntrySummary[]>([]);
	let loading = $state(false);
	let error = $state<string | null>(null);
	let active = $state(0);
	let searchEl = $state<HTMLInputElement | undefined>();
	let listEl = $state<HTMLDivElement | undefined>();

	/** Answers that arrive after the picker closed or the provider changed are dropped. */
	let seq = 0;
	/** A request is out; no other is sent until it answers. */
	let inflight = false;
	/** The query changed while a request was out: ask again when it answers. */
	let stale = false;
	/** Every entry, from the first (unfiltered) answer; null until it came. */
	let all = $state<EntrySummary[] | null>(null);
	/** That answer was the whole list (under the backend's cap), so filtering here is exact. */
	let complete = $state(false);
	/** The query the shown entries answer (when they came from the backend). */
	let shownQuery = '';

	let provider = $derived(providers.find((p) => p.id === providerId) ?? providers[0]);
	/** Only a different provider starts a new listing, not a new status object for the same one. */
	let providerKey = $derived(provider?.id ?? '');

	/** The backend's search, done here: case-insensitive, in name, user name or address. */
	function filterLocal(list: EntrySummary[], q: string): EntrySummary[] {
		const needle = q.trim().toLowerCase();
		if (!needle) return list;
		return list.filter(
			(e) =>
				e.name.toLowerCase().includes(needle) ||
				(e.username ?? '').toLowerCase().includes(needle) ||
				(e.uri ?? '').toLowerCase().includes(needle)
		);
	}

	function show(list: EntrySummary[]): void {
		entries = list;
		active = 0;
	}

	/** Send one request (unless one is out), then catch up with the query. */
	async function request(q: string, refresh: boolean): Promise<void> {
		if (!provider) return;
		if (inflight) {
			stale = true;
			return;
		}
		const mine = seq;
		const firstLoad = all === null || refresh;
		inflight = true;
		stale = false;
		loading = true;
		error = null;
		try {
			const list = await pwmanagerList(provider.id, firstLoad ? null : q, refresh);
			if (mine !== seq) return;
			if (firstLoad) {
				all = list;
				complete = list.length < LIST_CAP;
				shownQuery = '';
			} else {
				shownQuery = q.trim();
			}
			show(firstLoad ? filterLocal(list, query) : list);
		} catch (e) {
			if (mine !== seq) return;
			entries = [];
			error = String(e);
		} finally {
			if (mine === seq) {
				inflight = false;
				loading = false;
				// The list is only partial and what is shown does not answer
				// the search box (typed while waiting, or a refresh with a
				// search in it): ask for this query now. The backend answers
				// from its cache.
				if ((stale || firstLoad) && all !== null && !complete && !error && query.trim() !== shownQuery) {
					void request(query, false);
				}
				stale = false;
			}
		}
	}

	/** The search box changed. */
	function search(): void {
		if (all === null) {
			// The first load is still out (or failed): don't start another
			// sign-in; filter what it brings when it comes.
			stale = true;
			return;
		}
		if (complete) {
			show(filterLocal(all, query));
			return;
		}
		void request(query, false);
	}

	/** Start over for the provider shown: forget what was listed. */
	function reset(): void {
		seq++;
		inflight = false;
		stale = false;
		loading = false;
		all = null;
		complete = false;
		shownQuery = '';
		entries = [];
		error = null;
		active = 0;
	}

	// Fresh every time it opens.
	$effect(() => {
		if (!open) return;
		providerId = providers[0]?.id ?? '';
		query = '';
		void tick().then(() => searchEl?.focus());
	});

	// Opening, or switching provider: one full listing, straight away.
	// Follows the provider's id, not the selection: the selection being set
	// on open to the provider already shown starts nothing.
	$effect(() => {
		if (!open) return;
		void providerKey;
		untrack(() => {
			reset();
			void request('', false);
		});
	});

	// The list follows the search box, a moment after typing stops. Nothing
	// is sent while the first listing is out (see `search`).
	let firstQuery = true;
	$effect(() => {
		if (!open) {
			firstQuery = true;
			return;
		}
		void query;
		if (firstQuery) {
			firstQuery = false;
			return;
		}
		const delay = untrack(() => (all !== null && complete ? 0 : DEBOUNCE_MS));
		const timer = setTimeout(() => untrack(search), delay);
		return () => clearTimeout(timer);
	});

	// Escape closes only this dialog. Capture on window runs before the
	// document listeners every Modal (this one and the editor below) adds.
	$effect(() => {
		if (!open) return;
		const onKey = (e: KeyboardEvent) => {
			if (e.key !== 'Escape') return;
			e.stopPropagation();
			e.preventDefault();
			close();
		};
		window.addEventListener('keydown', onKey, true);
		return () => window.removeEventListener('keydown', onKey, true);
	});

	function close(): void {
		reset();
		onclose();
	}

	function choose(entry: EntrySummary): void {
		if (!provider) return;
		reset();
		onpick(entry, provider);
	}

	function move(to: number): void {
		if (entries.length === 0) return;
		active = Math.max(0, Math.min(entries.length - 1, to));
		void tick().then(() => {
			listEl?.querySelector<HTMLElement>(`#${uid}-opt-${active}`)?.scrollIntoView({ block: 'nearest' });
		});
	}

	function onSearchKey(e: KeyboardEvent): void {
		switch (e.key) {
			case 'ArrowDown':
				e.preventDefault();
				move(active + 1);
				break;
			case 'ArrowUp':
				e.preventDefault();
				move(active - 1);
				break;
			case 'PageDown':
				e.preventDefault();
				move(active + 8);
				break;
			case 'PageUp':
				e.preventDefault();
				move(active - 8);
				break;
			case 'Enter':
				e.preventDefault();
				if (entries[active]) choose(entries[active]);
				break;
		}
	}
</script>

<Modal {open} onclose={close} title={t('pwmanager.picker_title')} maxWidth="560px" zIndex={300}>
	<div class="picker">
		{#if providers.length > 1}
			<div class="providers" role="radiogroup" aria-label={t('pwmanager.provider')}>
				{#each providers as p (p.id)}
					<button
						type="button"
						class="provider-btn"
						class:active={provider?.id === p.id}
						role="radio"
						aria-checked={provider?.id === p.id}
						onclick={() => (providerId = p.id)}
					>
						{p.name}
					</button>
				{/each}
			</div>
		{/if}

		<div class="search-row">
			<input
				bind:this={searchEl}
				bind:value={query}
				class="search"
				type="search"
				role="combobox"
				aria-label={t('pwmanager.search_label', { name: provider?.name ?? '' })}
				aria-controls="{uid}-list"
				aria-expanded={entries.length > 0}
				aria-autocomplete="list"
				aria-activedescendant={entries[active] ? `${uid}-opt-${active}` : undefined}
				placeholder={t('pwmanager.search_placeholder')}
				autocomplete="off"
				autocapitalize="off"
				spellcheck="false"
				onkeydown={onSearchKey}
			/>
			<button
				type="button"
				class="icon-btn"
				onclick={() => void request(query, true)}
				disabled={loading}
				aria-label={t('pwmanager.refresh')}
				title={t('pwmanager.refresh')}
			>
				<svg class:spinning={loading} width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
					<path d="M21 12a9 9 0 1 1-2.64-6.36" />
					<path d="M21 3v6h-6" />
				</svg>
			</button>
		</div>

		<div class="status" role="status" aria-live="polite">
			{#if loading}
				{t('pwmanager.loading', { name: provider?.name ?? '' })}
			{:else if error}
				<span class="error">{error}</span>
			{:else if entries.length >= LIST_CAP}
				{t('pwmanager.list_capped', { count: String(LIST_CAP) })}
			{:else if entries.length > 0}
				{t('pwmanager.list_count', { count: String(entries.length) })}
			{/if}
		</div>

		<div class="list" id="{uid}-list" role="listbox" aria-label={t('pwmanager.entries')} bind:this={listEl}>
			{#if !loading && !error && entries.length === 0}
				<div class="empty">{query.trim() ? t('pwmanager.no_match') : t('pwmanager.empty')}</div>
			{/if}
			{#each entries as entry, i (entry.id)}
				<button
					type="button"
					id="{uid}-opt-{i}"
					class="entry"
					class:active={i === active}
					role="option"
					aria-selected={i === active}
					tabindex="-1"
					onmouseenter={() => (active = i)}
					onclick={() => choose(entry)}
				>
					<span class="entry-name">{entry.name || entry.id}</span>
					{#if entry.username || entry.uri}
						<span class="entry-meta">
							{#if entry.username}<span>{entry.username}</span>{/if}
							{#if entry.username && entry.uri}<span aria-hidden="true">·</span>{/if}
							{#if entry.uri}<span class="entry-uri">{entry.uri}</span>{/if}
						</span>
					{/if}
				</button>
			{/each}
		</div>

		<p class="hint">{t('pwmanager.picker_hint')}</p>
	</div>

	{#snippet actions()}
		<Button variant="secondary" onclick={close}>{t('common.cancel')}</Button>
	{/snippet}
</Modal>

<style>
	.picker {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.providers {
		display: flex;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		overflow: hidden;
	}

	.provider-btn {
		flex: 1;
		padding: 6px 10px;
		font-family: var(--font-sans);
		font-size: 0.75rem;
		font-weight: 500;
		border: none;
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
	}

	.provider-btn + .provider-btn {
		border-left: 1px solid var(--color-border);
	}

	.provider-btn.active {
		background-color: var(--color-accent);
		color: #fff;
	}

	.search-row {
		display: flex;
		gap: 6px;
	}

	.search {
		flex: 1;
		min-width: 0;
		padding: 8px 12px;
		font-family: var(--font-sans);
		font-size: 0.8125rem;
		color: var(--color-text-primary);
		background-color: var(--color-bg-primary);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		outline: none;
		box-sizing: border-box;
	}

	.search:focus {
		border-color: var(--color-accent);
	}

	.search::placeholder {
		color: var(--color-text-secondary);
		opacity: 0.5;
	}

	.icon-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 34px;
		flex-shrink: 0;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		background: var(--color-bg-elevated);
		color: var(--color-text-secondary);
		cursor: pointer;
	}

	.icon-btn:hover:not(:disabled) {
		color: var(--color-text-primary);
		background: var(--color-surface-hover);
	}

	.icon-btn:disabled {
		cursor: default;
	}

	.spinning {
		animation: spin 0.8s linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}

	.status {
		min-height: 1em;
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
	}

	.status .error {
		color: var(--color-danger);
		word-break: break-word;
	}

	.list {
		height: min(320px, 45vh);
		overflow-y: auto;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		background-color: var(--color-bg-secondary);
	}

	.empty {
		padding: 20px 16px;
		text-align: center;
		font-size: 0.8125rem;
		color: var(--color-text-secondary);
	}

	.entry {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 2px;
		width: 100%;
		padding: 7px 12px;
		border: none;
		border-bottom: 1px solid var(--color-border);
		background: transparent;
		font-family: var(--font-sans);
		text-align: left;
		cursor: pointer;
	}

	.entry:last-child {
		border-bottom: none;
	}

	.entry.active {
		background-color: color-mix(in srgb, var(--color-accent) 15%, transparent);
	}

	.entry-name {
		max-width: 100%;
		font-size: 0.8125rem;
		font-weight: 500;
		color: var(--color-text-primary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.entry-meta {
		display: flex;
		gap: 5px;
		max-width: 100%;
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
		overflow: hidden;
		white-space: nowrap;
	}

	.entry-uri {
		overflow: hidden;
		text-overflow: ellipsis;
		font-family: var(--font-mono);
	}

	.hint {
		margin: 0;
		font-size: 0.6875rem;
		color: var(--color-text-tertiary);
		line-height: 1.4;
	}
</style>
