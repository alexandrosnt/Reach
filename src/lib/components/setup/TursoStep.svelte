<script lang="ts">
	import Button from '$lib/components/shared/Button.svelte';
	import Input from '$lib/components/shared/Input.svelte';
	import { t } from '$lib/state/i18n.svelte';
	import { updateSetting } from '$lib/state/settings.svelte';

	interface Props {
		onNext: () => void;
		onBack: () => void;
	}

	let { onNext, onBack }: Props = $props();

	let mode = $state<'local' | 'cloud' | null>(null);
	let org = $state('');
	let apiToken = $state('');

	function handleNext() {
		if (mode === 'cloud') {
			if (org.trim()) {
				updateSetting('pendingTursoOrg', org.trim());
			}
			if (apiToken.trim()) {
				updateSetting('pendingTursoApiToken', apiToken.trim());
			}
		}
		onNext();
	}

</script>

<div class="step">
	<div class="step-header">
		<h2 class="step-title">{t('setup.storage_title')}</h2>
		<p class="step-subtitle">{t('setup.storage_subtitle')}</p>
	</div>

	<div class="mode-cards">
		<button class="mode-card" class:selected={mode === 'local'} onclick={() => (mode = 'local')}>
			<div class="mode-icon">
				<svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
					<rect x="2" y="2" width="20" height="8" rx="2" />
					<rect x="2" y="14" width="20" height="8" rx="2" />
					<circle cx="6" cy="6" r="1" fill="currentColor" />
					<circle cx="6" cy="18" r="1" fill="currentColor" />
				</svg>
			</div>
			<span class="mode-name">{t('setup.local_only')}</span>
			<span class="mode-desc">{t('setup.local_only_desc')}</span>
		</button>

		<button class="mode-card" class:selected={mode === 'cloud'} onclick={() => (mode = 'cloud')}>
			<div class="mode-icon">
				<svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
					<path d="M18 10h-1.26A8 8 0 1 0 9 20h9a5 5 0 0 0 0-10z" />
				</svg>
			</div>
			<span class="mode-name">{t('setup.cloud_sync')}</span>
			<span class="mode-desc">{t('setup.cloud_sync_desc')}</span>
		</button>
	</div>

	{#if mode === 'cloud'}
		<div class="form">
			<div class="form-field">
				<Input
					label={t('setup.turso_org_label')}
					placeholder={t('setup.turso_org_placeholder')}
					bind:value={org}
				/>
			</div>

			<div class="form-field">
				<Input
					label={t('setup.turso_token_label')}
					type="password"
					placeholder={t('setup.turso_token_placeholder')}
					bind:value={apiToken}
				/>
			</div>
		</div>
	{/if}

	<div class="step-actions">
		<Button variant="ghost" size="md" onclick={onBack}>
			{t('setup.back')}
		</Button>
		<Button variant="primary" size="md" onclick={handleNext} disabled={!mode}>
			{t('setup.next')}
		</Button>
	</div>
</div>

<style>
	.step {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 24px;
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

	.mode-cards {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 12px;
		width: 100%;
		max-width: 380px;
	}

	.mode-card {
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

	.mode-card:hover {
		background: rgba(255, 255, 255, 0.04);
	}

	.mode-card.selected {
		border-color: var(--color-accent);
		background: rgba(10, 132, 255, 0.08);
	}

	.mode-icon {
		color: var(--color-text-secondary);
		transition: color var(--duration-default) var(--ease-default);
	}

	.mode-card.selected .mode-icon {
		color: var(--color-accent);
	}

	.mode-name {
		font-size: 0.875rem;
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.mode-desc {
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
		text-align: center;
		line-height: 1.4;
	}

	.form {
		display: flex;
		flex-direction: column;
		gap: 14px;
		width: 100%;
		max-width: 380px;
	}

	.form-field {
		width: 100%;
	}

	.step-actions {
		display: flex;
		justify-content: space-between;
		align-items: center;
		width: 100%;
		max-width: 380px;
	}
</style>
