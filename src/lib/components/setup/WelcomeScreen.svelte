<script lang="ts">
	import LanguageStep from './LanguageStep.svelte';
	import TursoStep from './TursoStep.svelte';
	import CompleteStep from './CompleteStep.svelte';
	import { t } from '$lib/state/i18n.svelte';
	import { updateSetting } from '$lib/state/settings.svelte';

	let currentStep = $state(0);
	const totalSteps = 3;

	function next() {
		if (currentStep < totalSteps - 1) {
			currentStep++;
		}
	}

	function back() {
		if (currentStep > 0) {
			currentStep--;
		}
	}

	function complete() {
		updateSetting('setupComplete', true);
	}
</script>

<div class="welcome-overlay">
	<div class="welcome-container">
		<!-- Step indicator dots -->
		<div class="step-dots">
			{#each Array(totalSteps) as _, i (i)}
				<div class="dot" class:active={i === currentStep} class:completed={i < currentStep}></div>
			{/each}
		</div>

		<!-- Welcome header (only on first step) -->
		{#if currentStep === 0}
			<div class="welcome-header">
				<h1 class="welcome-title">{t('setup.welcome_title')}</h1>
				<p class="welcome-subtitle">{t('setup.welcome_subtitle')}</p>
			</div>
		{/if}

		<!-- Step content -->
		<div class="step-content">
			{#if currentStep === 0}
				<LanguageStep onNext={next} />
			{:else if currentStep === 1}
				<TursoStep onNext={next} onBack={back} />
			{:else if currentStep === 2}
				<CompleteStep onComplete={complete} onBack={back} />
			{/if}
		</div>
	</div>
</div>

<style>
	.welcome-overlay {
		position: fixed;
		inset: 0;
		z-index: 200;
		display: flex;
		/* Scroll when the wizard is taller than the screen (small/mobile
		   viewports) so the action buttons stay reachable, and keep clear of
		   the status/navigation bars via safe-area insets. `margin: auto` on the
		   child centers it when it fits without clipping the top when it doesn't. */
		overflow-y: auto;
		background: rgba(0, 0, 0, 0.85);
		backdrop-filter: blur(24px);
		-webkit-backdrop-filter: blur(24px);
		padding: max(20px, env(safe-area-inset-top)) 16px max(20px, env(safe-area-inset-bottom));
	}

	.welcome-container {
		margin: auto;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 24px;
		padding: 40px clamp(20px, 5vw, 48px);
		max-width: 520px;
		width: 100%;
		background: var(--color-bg-secondary);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-modal);
		box-shadow: var(--shadow-elevated);
	}

	.step-dots {
		display: flex;
		gap: 8px;
	}

	.dot {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: var(--color-border);
		transition: background-color var(--duration-default) var(--ease-default),
			transform var(--duration-default) var(--ease-default);
	}

	.dot.active {
		background: var(--color-accent);
		transform: scale(1.25);
	}

	.dot.completed {
		background: var(--color-success);
	}

	.welcome-header {
		text-align: center;
	}

	.welcome-title {
		margin: 0;
		font-size: 1.5rem;
		font-weight: 700;
		color: var(--color-text-primary);
	}

	.welcome-subtitle {
		margin: 6px 0 0;
		font-size: 0.875rem;
		color: var(--color-text-secondary);
	}

	.step-content {
		width: 100%;
	}
</style>
