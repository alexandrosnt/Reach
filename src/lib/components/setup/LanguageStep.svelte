<script lang="ts">
	import Button from '$lib/components/shared/Button.svelte';
	import { t, changeLocale } from '$lib/state/i18n.svelte';
	import { updateSetting, getSettings } from '$lib/state/settings.svelte';

	interface Props {
		onNext: () => void;
	}

	let { onNext }: Props = $props();

	const settings = getSettings();

	const languages = [
		{ code: 'en', name: 'English', subtitle: 'English' },
		{ code: 'de', name: 'Deutsch', subtitle: 'German' },
		{ code: 'fr', name: 'Français', subtitle: 'French' },
		{ code: 'el', name: 'Ελληνικά', subtitle: 'Greek' },
		{ code: 'it', name: 'Italiano', subtitle: 'Italian' }
	];

	let selected = $state(settings.locale || 'en');

	function selectLanguage(code: string) {
		selected = code;
		changeLocale(code);
		updateSetting('locale', code);
	}
</script>

<div class="step">
	<div class="step-header">
		<h2 class="step-title">{t('setup.language_title')}</h2>
		<p class="step-subtitle">{t('setup.language_subtitle')}</p>
	</div>

	<div class="language-grid">
		{#each languages as lang (lang.code)}
			<button
				class="language-card"
				class:selected={selected === lang.code}
				onclick={() => selectLanguage(lang.code)}
			>
				<svg class="flag-icon" viewBox="0 0 60 40" fill="none" xmlns="http://www.w3.org/2000/svg">
					{#if lang.code === 'en'}
						<!-- Union Jack -->
						<rect width="60" height="40" fill="#012169"/>
						<path d="M0 0L60 40M60 0L0 40" stroke="#fff" stroke-width="6.5"/>
						<path d="M0 0L60 40M60 0L0 40" stroke="#C8102E" stroke-width="4"/>
						<path d="M30 0V40M0 20H60" stroke="#fff" stroke-width="10"/>
						<path d="M30 0V40M0 20H60" stroke="#C8102E" stroke-width="6"/>
					{:else if lang.code === 'de'}
						<!-- Germany -->
						<rect width="60" height="13.33" fill="#000"/>
						<rect y="13.33" width="60" height="13.34" fill="#DD0000"/>
						<rect y="26.67" width="60" height="13.33" fill="#FFCC00"/>
					{:else if lang.code === 'fr'}
						<!-- France -->
						<rect width="20" height="40" fill="#002395"/>
						<rect x="20" width="20" height="40" fill="#fff"/>
						<rect x="40" width="20" height="40" fill="#ED2939"/>
					{:else if lang.code === 'el'}
						<!-- Greece -->
						<rect width="60" height="40" fill="#0D5EAF"/>
						<rect y="4.44" width="60" height="4.44" fill="#fff"/>
						<rect y="13.33" width="60" height="4.44" fill="#fff"/>
						<rect y="22.22" width="60" height="4.44" fill="#fff"/>
						<rect y="31.11" width="60" height="4.44" fill="#fff"/>
						<rect width="22.22" height="17.78" fill="#0D5EAF"/>
						<rect x="8.89" y="0" width="4.44" height="17.78" fill="#fff"/>
						<rect x="0" y="6.67" width="22.22" height="4.44" fill="#fff"/>
					{:else if lang.code === 'it'}
						<!-- Italy -->
						<rect width="20" height="40" fill="#009246"/>
						<rect x="20" width="20" height="40" fill="#fff"/>
						<rect x="40" width="20" height="40" fill="#CE2B37"/>
					{/if}
				</svg>
				<span class="lang-name">{lang.name}</span>
				<span class="lang-subtitle">{lang.subtitle}</span>
			</button>
		{/each}
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
		gap: 32px;
		width: 100%;
	}

	.step-header {
		text-align: center;
	}

	.step-title {
		margin: 0;
		font-size: 1.25rem;
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.step-subtitle {
		margin: 6px 0 0;
		font-size: 0.8125rem;
		color: var(--color-text-secondary);
	}

	.language-grid {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(120px, 1fr));
		gap: 10px;
		width: 100%;
		max-width: 460px;
	}

	.language-card {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 8px;
		padding: 20px 16px;
		background: var(--color-bg-elevated);
		border: 2px solid var(--color-border);
		border-radius: var(--radius-card);
		cursor: pointer;
		transition: border-color var(--duration-default) var(--ease-default),
			background-color var(--duration-default) var(--ease-default);
	}

	.language-card:hover {
		background: rgba(255, 255, 255, 0.04);
	}

	.language-card.selected {
		border-color: var(--color-accent);
		background: rgba(10, 132, 255, 0.08);
	}

	.flag-icon {
		width: 48px;
		height: 32px;
		border-radius: 4px;
		overflow: hidden;
	}

	.lang-name {
		font-size: 0.875rem;
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.lang-subtitle {
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
	}

	.step-actions {
		display: flex;
		justify-content: center;
		gap: 12px;
		width: 100%;
	}
</style>
