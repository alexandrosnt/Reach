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

	let org = $state('');
	let apiToken = $state('');

	function handleNext() {
		if (org.trim()) {
			updateSetting('pendingTursoOrg', org.trim());
		}
		if (apiToken.trim()) {
			updateSetting('pendingTursoApiToken', apiToken.trim());
		}
		onNext();
	}

</script>

<div class="step">
	<div class="step-header">
		<h2 class="step-title">{t('setup.turso_title')}</h2>
		<p class="step-subtitle">{t('setup.turso_subtitle')}</p>
		<p class="step-description">{t('setup.turso_description')}</p>
	</div>

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

	<div class="step-actions">
		<Button variant="ghost" size="md" onclick={onBack}>
			{t('setup.back')}
		</Button>
		<Button variant="primary" size="md" onclick={handleNext}>
			{t('setup.next')}
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

	.step-description {
		margin: 8px 0 0;
		font-size: 0.75rem;
		color: var(--color-text-secondary);
		line-height: 1.5;
		max-width: 380px;
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
