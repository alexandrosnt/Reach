<script lang="ts">
	/**
	 * Language picker: a dropdown showing the flag, the endonym and the English
	 * name of each locale.
	 *
	 * Replaces the grid of flag cards the setup wizard used. That grid was fine
	 * at four languages and already 400px tall at eight; every translation
	 * contributed made the first thing a new user sees taller, and pushed the
	 * rest of the wizard off a laptop screen. A dropdown costs one row no matter
	 * how many locales ship.
	 *
	 * Not built on shared/Dropdown.svelte, which takes `{label, value}` strings
	 * and has no room for an icon or a second line — teaching it snippets would
	 * change a component every other settings panel depends on.
	 */
	import FlagIcon from './FlagIcon.svelte';
	import { LANGUAGES, findLanguage } from '$lib/i18n/languages';
	import { t } from '$lib/state/i18n.svelte';
	import { matchesQuery } from '$lib/utils/search';

	interface Props {
		/** Locale code. */
		value: string;
		onchange: (code: string) => void;
	}

	let { value, onchange }: Props = $props();

	let open = $state(false);
	let query = $state('');
	let active = $state(0);
	let rootEl: HTMLDivElement | undefined = $state();
	let inputEl: HTMLInputElement | undefined = $state();
	let current = $derived(findLanguage(value));

	/** Matches the endonym, the English name, or the locale code, so "Deutsch",
	 *  "german" and "de" all find German. */
	let matches = $derived(
		LANGUAGES.filter((l) => matchesQuery(query, [l.name, l.english, l.code]))
	);

	function choose(code: string) {
		open = false;
		query = '';
		if (code !== value) onchange(code);
	}

	function toggle() {
		open = !open;
		if (open) {
			query = '';
			// Start on the current language so Enter without typing is a no-op
			// rather than a silent jump to whatever sits at the top.
			active = Math.max(0, LANGUAGES.findIndex((l) => l.code === value));
			queueMicrotask(() => inputEl?.focus());
		}
	}

	// Keep the highlight inside the filtered list as it shrinks.
	$effect(() => {
		if (active >= matches.length) active = Math.max(0, matches.length - 1);
	});

	function onKeydown(e: KeyboardEvent) {
		if (!open) return;
		if (e.key === 'Escape') {
			e.stopPropagation();
			open = false;
			return;
		}
		if (e.key === 'ArrowDown') {
			e.preventDefault();
			active = matches.length ? (active + 1) % matches.length : 0;
		} else if (e.key === 'ArrowUp') {
			e.preventDefault();
			active = matches.length ? (active - 1 + matches.length) % matches.length : 0;
		} else if (e.key === 'Enter') {
			e.preventDefault();
			const pick = matches[active];
			if (pick) choose(pick.code);
		}
	}

	$effect(() => {
		if (!open) return;
		function onClickOutside(e: MouseEvent) {
			if (rootEl && !rootEl.contains(e.target as Node)) open = false;
		}
		document.addEventListener('click', onClickOutside, true);
		return () => document.removeEventListener('click', onClickOutside, true);
	});
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="select" bind:this={rootEl} onkeydown={onKeydown}>
	<button
		type="button"
		class="trigger"
		class:open
		onclick={toggle}
		aria-haspopup="listbox"
		aria-expanded={open}
	>
		<FlagIcon code={current.code} size={24} />
		<span class="names">
			<span class="native">{current.name}</span>
			{#if current.english !== current.name}
				<span class="english">{current.english}</span>
			{/if}
		</span>
		<svg class="chevron" width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden="true">
			<path d="M3 4.5L6 7.5L9 4.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
		</svg>
	</button>

	{#if open}
		<div class="menu">
			<input
				bind:this={inputEl}
				bind:value={query}
				class="search"
				type="text"
				autocomplete="off"
				spellcheck="false"
				placeholder={t('settings.language_search')}
				aria-label={t('settings.language_search')}
			/>
			{#if matches.length === 0}
				<p class="no-results">{t('settings.language_no_match')}</p>
			{/if}
			<ul class="options" role="listbox" aria-label="Language">
				{#each matches as lang, i (lang.code)}
				<li>
					<button
						type="button"
						class="option"
						class:selected={lang.code === value}
						class:active={i === active}
						role="option"
						aria-selected={lang.code === value}
						onclick={() => choose(lang.code)}
						onmouseenter={() => (active = i)}
					>
						<FlagIcon code={lang.code} size={22} />
						<span class="names">
							<span class="native">{lang.name}</span>
							{#if lang.english !== lang.name}
								<span class="english">{lang.english}</span>
							{/if}
						</span>
						{#if lang.code === value}
							<svg class="tick" width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
								<path d="M13.3 4.7L6.5 11.5L2.7 7.7" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" />
							</svg>
						{/if}
					</button>
				</li>
				{/each}
			</ul>
		</div>
	{/if}
</div>

<style>
	.select {
		position: relative;
		width: 100%;
	}

	.trigger {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		width: 100%;
		padding: 10px var(--space-3);
		background: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		cursor: pointer;
		text-align: left;
		transition: border-color var(--duration-default) var(--ease-default),
			background-color var(--duration-default) var(--ease-default);
	}

	.trigger:hover {
		background: var(--color-surface-hover);
	}

	.trigger.open {
		border-color: var(--color-accent);
	}

	.names {
		display: flex;
		flex-direction: column;
		gap: 1px;
		flex: 1;
		min-width: 0;
	}

	.native {
		font-size: var(--text-sm);
		font-weight: 600;
		color: var(--color-text-primary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.english {
		font-size: var(--text-2xs);
		color: var(--color-text-tertiary);
	}

	.chevron {
		flex-shrink: 0;
		color: var(--color-text-secondary);
		transition: transform var(--duration-default) var(--ease-default);
	}

	.trigger.open .chevron {
		transform: rotate(180deg);
	}

	.menu {
		position: absolute;
		z-index: 10;
		top: calc(100% + 4px);
		left: 0;
		right: 0;
		display: flex;
		flex-direction: column;
		/* Nine locales already overflow a short window, and the list only grows
		   as translations land — scroll inside the menu rather than off-screen. */
		max-height: 300px;
		padding: var(--space-1);
		background: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		box-shadow: var(--shadow-elevated);
	}

	/* The search box stays put while the results scroll under it. */
	.search {
		flex-shrink: 0;
		width: 100%;
		padding: 7px var(--space-2);
		margin-bottom: var(--space-1);
		font-size: var(--text-sm);
		font-family: inherit;
		color: var(--color-text-primary);
		background: var(--color-surface-sunken);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-sm);
		outline: none;
	}

	.search:focus {
		border-color: var(--color-accent);
	}

	.search::placeholder {
		color: var(--color-text-tertiary);
	}

	.options {
		flex: 1;
		overflow-y: auto;
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.no-results {
		margin: 0;
		padding: var(--space-3) var(--space-2);
		font-size: var(--text-xs);
		color: var(--color-text-tertiary);
		text-align: center;
	}

	.option {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		width: 100%;
		padding: 8px var(--space-2);
		background: none;
		border: none;
		border-radius: var(--radius-sm);
		cursor: pointer;
		text-align: left;
	}

	.option:hover,
	.option.active {
		background: var(--color-surface-hover);
	}

	.option.selected .native {
		color: var(--color-accent);
	}

	.tick {
		flex-shrink: 0;
		color: var(--color-accent);
	}
</style>
