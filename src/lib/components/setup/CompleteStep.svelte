<script lang="ts">
	import Button from '$lib/components/shared/Button.svelte';
	import { t } from '$lib/state/i18n.svelte';
	import { getSettings } from '$lib/state/settings.svelte';

	interface Props {
		onComplete: () => void;
		onBack: () => void;
	}

	let { onComplete, onBack }: Props = $props();

	const settings = getSettings();
	let hasTurso = $derived(!!settings.pendingTursoOrg);
</script>

<div class="step">
	<div class="step-header">
		<div class="checkmark">
			<svg width="48" height="48" viewBox="0 0 48 48" fill="none" xmlns="http://www.w3.org/2000/svg">
				<circle cx="24" cy="24" r="24" fill="var(--color-success)" opacity="0.15"/>
				<circle cx="24" cy="24" r="18" fill="var(--color-success)" opacity="0.25"/>
				<path d="M16 24L22 30L32 18" stroke="var(--color-success)" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"/>
			</svg>
		</div>
		<h2 class="step-title">{t('setup.complete_title')}</h2>
		<p class="step-subtitle">{t('setup.complete_subtitle')}</p>
	</div>

	<div class="summary">
		<div class="summary-item">
			<span class="summary-icon">
				<svg width="16" height="16" viewBox="0 0 16 16" fill="none"><path d="M13.3 4.7L6.5 11.5L2.7 7.7" stroke="var(--color-success)" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>
			</span>
			<span class="summary-label">{t('settings.language')}</span>
			<span class="summary-value">{settings.locale === 'en' ? 'English' : settings.locale}</span>
		</div>
		<div class="summary-item">
			<span class="summary-icon">
				<svg width="16" height="16" viewBox="0 0 16 16" fill="none"><path d="M13.3 4.7L6.5 11.5L2.7 7.7" stroke="var(--color-success)" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>
			</span>
			<span class="summary-label">{t('setup.turso_title')}</span>
			<span class="summary-value">{hasTurso ? settings.pendingTursoOrg : '\u2014'}</span>
		</div>
	</div>

	<div class="step-actions">
		<Button variant="ghost" size="md" onclick={onBack}>
			{t('setup.back')}
		</Button>
		<Button variant="primary" size="md" onclick={onComplete}>
			{t('setup.get_started')}
		</Button>
	</div>
</div>

<style>
	.step {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 28px;
		width: 100%;
	}

	.step-header {
		display: flex;
		flex-direction: column;
		align-items: center;
		text-align: center;
		gap: 12px;
	}

	.checkmark {
		margin-bottom: 4px;
	}

	.step-title {
		margin: 0;
		font-size: 1.25rem;
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.step-subtitle {
		margin: 0;
		font-size: 0.8125rem;
		color: var(--color-text-secondary);
	}

	.summary {
		display: flex;
		flex-direction: column;
		gap: 8px;
		width: 100%;
		max-width: 320px;
	}

	.summary-item {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 10px 14px;
		background: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
	}

	.summary-icon {
		display: flex;
		align-items: center;
		flex-shrink: 0;
	}

	.summary-label {
		font-size: 0.8125rem;
		font-weight: 500;
		color: var(--color-text-primary);
		flex: 1;
	}

	.summary-value {
		font-size: 0.75rem;
		color: var(--color-text-secondary);
	}

	.step-actions {
		display: flex;
		justify-content: center;
		gap: 12px;
		width: 100%;
	}
</style>
