<script lang="ts">
	/**
	 * First wizard step: language and theme.
	 *
	 * Replaces LanguageStep, which spent the whole first screen on a grid of
	 * flag cards — 400px of it at eight locales, and growing with every
	 * translation contributed. Both choices are personalization, both apply
	 * live, and together they now cost less height than the language grid did
	 * alone.
	 */
	import { onMount } from 'svelte';
	import Button from '$lib/components/shared/Button.svelte';
	import LanguageSelect from '$lib/components/shared/LanguageSelect.svelte';
	import ThemeSwatch from '$lib/components/shared/ThemeSwatch.svelte';
	import { t, changeLocale } from '$lib/state/i18n.svelte';
	import { updateSetting, getSettings } from '$lib/state/settings.svelte';
	import {
		themeState,
		applyTheme,
		validateTheme,
		DARK,
		LIGHT,
		type Theme
	} from '$lib/state/theme.svelte';
	import { themeFetchRegistry, themeInstall, type ThemeEntry } from '$lib/ipc/theme';
	import { addToast } from '$lib/state/toasts.svelte';
	import { matchesQuery } from '$lib/utils/search';

	interface Props {
		onNext: () => void;
	}

	let { onNext }: Props = $props();

	const settings = getSettings();

	/** "System" follows the OS; every other entry is a real theme. Same sentinel
	 *  as the Appearance tab, so the two pickers agree on what is selected. */
	const SYSTEM = '__system__';

	/** Registry themes not yet on disk. Fetched so the picker is not just the two
	 *  built-ins — the marketplace is where the interesting themes live, and
	 *  making someone finish setup and find Settings to see any of them is a
	 *  poor trade for one network call that is allowed to fail. */
	let available = $state<ThemeEntry[]>([]);
	let installingId = $state<string | null>(null);

	onMount(async () => {
		try {
			available = await themeFetchRegistry();
		} catch {
			// Same reasoning as the plugins step: an unreachable registry is not
			// an error worth showing on the first screen. The built-ins remain.
			available = [];
		}
	});

	let installedIds = $derived(new Set(themeState.all.map((th) => th.id)));

	/** What is already on this machine: System, the built-ins, and anything
	 *  previously installed. These are choices — one click and they apply. */
	let ready = $derived([
		{ id: SYSTEM, name: t('settings.theme_system'), theme: null as Theme | null, entry: null },
		...themeState.all.map((th) => ({ id: th.id, name: th.name, theme: th, entry: null }))
	]);

	/**
	 * How many marketplace themes this step will ever render at once.
	 *
	 * The registry is expected to grow without bound, and the setup wizard is
	 * not a catalogue browser — rendering all of it here would repeat exactly
	 * the mistake the old language grid made, on a list with no ceiling. So the
	 * whole index is fetched (it is small per entry) but only a screenful is
	 * ever built: search narrows it, the count says what is being hidden, and
	 * Settings > Appearance is where the full catalogue lives.
	 */
	const MAX_SHOWN = 6;

	let themeQuery = $state('');

	/** Registry themes not on this machine. Kept in their own group rather than
	 *  mixed in with the defaults, because these are downloads, not choices —
	 *  a card that costs a network fetch should not sit in the same row as one
	 *  that does not. Once installed a theme moves up into `ready` on its own. */
	let matchingThemes = $derived(
		available
			.filter((e) => !installedIds.has(e.id))
			.filter((e) => matchesQuery(themeQuery, [e.name, e.description, e.author, e.appearance]))
	);

	let installable = $derived(
		matchingThemes
			.slice(0, MAX_SHOWN)
			.map((e) => ({ id: e.id, name: e.name, theme: null as Theme | null, entry: e }))
	);

	/** Only worth a search box once the list is longer than the cap. */
	let themeSearchable = $derived(
		available.filter((e) => !installedIds.has(e.id)).length > MAX_SHOWN
	);

	/** Install a registry theme, then select it — one click, not two. */
	async function installAndSelect(entry: ThemeEntry): Promise<void> {
		installingId = entry.id;
		try {
			const doc = await themeInstall(entry);
			const result = validateTheme(doc);
			if (!result.ok) {
				addToast(result.error, 'error');
				return;
			}
			themeState.installed = [
				...themeState.installed.filter((th) => th.id !== result.theme.id),
				result.theme
			];
			selectTheme(result.theme.id);
		} catch (err) {
			addToast(String(err), 'error');
		} finally {
			installingId = null;
		}
	}

	let selectedTheme = $derived(settings.themeId ?? (settings.theme === 'system' ? SYSTEM : null));

	function selectLanguage(code: string) {
		changeLocale(code);
		updateSetting('locale', code);
	}

	function selectTheme(id: string) {
		if (id === SYSTEM) {
			updateSetting('themeId', undefined);
			updateSetting('theme', 'system');
			applyTheme(window.matchMedia('(prefers-color-scheme: dark)').matches ? DARK : LIGHT);
			return;
		}
		const theme = themeState.all.find((th) => th.id === id);
		if (!theme) return;
		updateSetting('themeId', theme.id);
		// Keep `theme` consistent so anything still reading it, and the fallback
		// used when a themeId no longer resolves, stay sensible.
		updateSetting('theme', theme.appearance);
		applyTheme(theme);
	}
</script>

<div class="step">
	<div class="step-header">
		<h2 class="step-title">{t('setup.personalize_title')}</h2>
		<p class="step-subtitle">{t('setup.personalize_subtitle')}</p>
	</div>

	<div class="fields">
		<div class="field">
			<span class="field-label">{t('settings.language')}</span>
			<LanguageSelect value={settings.locale || 'en'} onchange={selectLanguage} />
		</div>

		<div class="field">
			<span class="field-label">{t('settings.theme')}</span>
			<div class="theme-grid">
				{#each ready as choice (choice.id)}
					<button
						type="button"
						class="theme-card"
						class:selected={selectedTheme === choice.id}
						onclick={() => selectTheme(choice.id)}
						aria-pressed={selectedTheme === choice.id}
					>
						<ThemeSwatch theme={choice.theme} dark={DARK} light={LIGHT} />
						<span class="theme-name">{choice.name}</span>
					</button>
				{/each}
			</div>
		</div>

		{#if themeSearchable || installable.length > 0}
			<div class="field">
				<div class="field-head">
					<span class="field-label">{t('themes.marketplace')}</span>
					{#if matchingThemes.length > installable.length}
						<span class="field-count">
							{t('themes.showing_n_of_m', {
								shown: installable.length,
								total: matchingThemes.length
							})}
						</span>
					{/if}
				</div>

				{#if themeSearchable}
					<input
						bind:value={themeQuery}
						class="theme-search"
						type="text"
						autocomplete="off"
						spellcheck="false"
						placeholder={t('themes.search')}
						aria-label={t('themes.search')}
					/>
				{/if}

				{#if installable.length === 0}
					<p class="theme-none">{t('themes.no_match')}</p>
				{/if}

				<div class="theme-grid">
					{#each installable as choice (choice.id)}
						<button
							type="button"
							class="theme-card pending"
							disabled={installingId === choice.id}
							onclick={() => choice.entry && installAndSelect(choice.entry)}
						>
							<ThemeSwatch theme={null} dark={DARK} light={LIGHT} entry={choice.entry} />
							<span class="theme-name">
								{installingId === choice.id ? t('themes.installing') : choice.name}
							</span>
						</button>
					{/each}
				</div>

				{#if matchingThemes.length > installable.length}
					<p class="theme-more">{t('themes.more_in_settings')}</p>
				{/if}
			</div>
		{/if}
	</div>

	<div class="step-actions">
		<Button variant="primary" size="md" onclick={onNext}>
			{t('setup.next')}
		</Button>
	</div>
</div>

<style>
	.step {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--space-5);
		width: 100%;
	}

	.step-header {
		text-align: center;
	}

	.step-title {
		margin: 0;
		font-size: var(--text-lg);
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.step-subtitle {
		margin: 6px 0 0;
		font-size: var(--text-sm);
		color: var(--color-text-secondary);
	}

	.fields {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
		width: 100%;
	}

	.field {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}

	.field-label {
		font-size: var(--text-2xs);
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.04em;
		color: var(--color-text-tertiary);
	}

	.field-head {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: var(--space-2);
	}

	.field-count {
		font-size: var(--text-2xs);
		color: var(--color-text-tertiary);
		font-variant-numeric: tabular-nums;
	}

	.theme-search {
		width: 100%;
		padding: 7px var(--space-2);
		font-size: var(--text-sm);
		font-family: inherit;
		color: var(--color-text-primary);
		background: var(--color-surface-sunken);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-sm);
		outline: none;
	}

	.theme-search:focus {
		border-color: var(--color-accent);
	}

	.theme-search::placeholder {
		color: var(--color-text-tertiary);
	}

	.theme-none,
	.theme-more {
		margin: 0;
		font-size: var(--text-xs);
		color: var(--color-text-tertiary);
		text-align: center;
	}

	.theme-grid {
		display: grid;
		/* Three across at the wizard's width; wraps on its own once installed
		   themes push past that, without a media query. */
		grid-template-columns: repeat(auto-fit, minmax(124px, 1fr));
		gap: var(--space-2);
	}

	.theme-card {
		display: flex;
		flex-direction: column;
		align-items: stretch;
		gap: 6px;
		padding: 6px;
		background: var(--color-bg-elevated);
		border: 2px solid var(--color-border);
		border-radius: var(--radius-card);
		cursor: pointer;
		transition: border-color var(--duration-default) var(--ease-default),
			background-color var(--duration-default) var(--ease-default);
	}

	.theme-card:hover {
		background: var(--color-surface-hover);
	}

	.theme-card.selected {
		border-color: var(--color-accent);
	}

	/* Not installed yet: dashed edge marks it as an action, not a choice. */
	.theme-card.pending {
		border-style: dashed;
	}

	.theme-card:disabled {
		cursor: default;
		opacity: 0.6;
	}

	.theme-name {
		font-size: var(--text-xs);
		font-weight: 600;
		color: var(--color-text-primary);
		text-align: center;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.step-actions {
		display: flex;
		justify-content: center;
		gap: var(--space-3);
		width: 100%;
	}
</style>

