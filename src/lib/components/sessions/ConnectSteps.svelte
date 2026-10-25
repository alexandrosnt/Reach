<script lang="ts">
	import { t } from '$lib/state/i18n.svelte';
	import { CONNECT_STEPS as STEPS, connectStep, type ConnectStep as Step } from '$lib/state/connect-stages.svelte';

	interface Props {
		/** The connection being made; its steps are the ones shown. */
		id?: string;
		/** The connect is still running. */
		active: boolean;
		/** It ended in an error: the step it had reached is the one that broke. */
		failed: boolean;
	}

	let { id, active, failed }: Props = $props();

	const LABELS: Record<Step, string> = {
		connect: 'connect.step_connect',
		kex: 'connect.step_kex',
		hostkey: 'connect.step_hostkey',
		auth: 'connect.step_auth',
		shell: 'connect.step_shell'
	};

	// A connection that sends no steps (through jump hosts) keeps the plain
	// "Connecting…" on the button.
	let reached = $derived(connectStep(id));

	let at = $derived(reached ? STEPS.indexOf(reached) : -1);
	let shown = $derived(at >= 0 && (active || failed));

	function stepState(i: number): 'done' | 'current' | 'failed' | 'pending' {
		if (i < at) return 'done';
		if (i === at) return failed ? 'failed' : 'current';
		return 'pending';
	}
</script>

{#if shown}
	<ol class="steps" aria-label={t('connect.steps')}>
		{#each STEPS as step, i (step)}
			{@const s = stepState(i)}
			<li class="step {s}" aria-current={s === 'current' ? 'step' : undefined}>
				<span class="mark" aria-hidden="true">
					{#if s === 'done'}
						<svg width="10" height="10" viewBox="0 0 12 12" fill="none"><path d="M2.5 6.2l2.3 2.3 4.7-5" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" /></svg>
					{:else if s === 'failed'}
						<svg width="10" height="10" viewBox="0 0 12 12" fill="none"><path d="M3 3l6 6M9 3l-6 6" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" /></svg>
					{/if}
				</span>
				<span class="label">{t(LABELS[step])}</span>
			</li>
		{/each}
	</ol>
	<!-- Said once per step, not read out as the whole list again. -->
	<span class="sr-only" aria-live="polite">{reached ? t(LABELS[reached]) : ''}</span>
{/if}

<style>
	.steps {
		display: flex;
		margin: 2px 0 0;
		padding: 0;
		list-style: none;
	}

	.step {
		position: relative;
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 5px;
		font-size: 0.6875rem;
		color: var(--color-text-tertiary, var(--color-text-secondary));
	}

	/* The line between one step and the next, filled as it is passed. */
	.step:not(:first-child)::before {
		content: '';
		position: absolute;
		top: 8px;
		right: calc(50% + 11px);
		left: calc(-50% + 11px);
		height: 2px;
		border-radius: 1px;
		background: var(--color-border);
		transition: background-color 240ms var(--ease-default, ease);
	}

	.step.done::before,
	.step.current::before,
	.step.failed::before {
		background: var(--color-accent);
	}

	.mark {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 18px;
		height: 18px;
		border-radius: 50%;
		border: 2px solid var(--color-border);
		background: var(--color-bg-elevated, var(--color-bg-primary));
		color: #fff;
		transition:
			background-color 240ms var(--ease-default, ease),
			border-color 240ms var(--ease-default, ease);
	}

	.done .mark {
		background: var(--color-accent);
		border-color: var(--color-accent);
	}

	.current .mark {
		border-color: var(--color-accent);
		animation: step-pulse 1.1s ease-in-out infinite;
	}

	.failed .mark {
		background: var(--color-danger);
		border-color: var(--color-danger);
	}

	.label {
		max-width: 100%;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.done .label {
		color: var(--color-text-secondary);
	}

	.current .label {
		color: var(--color-text-primary);
		font-weight: 600;
	}

	.failed .label {
		color: var(--color-danger);
		font-weight: 600;
	}

	@keyframes step-pulse {
		50% {
			box-shadow: 0 0 0 4px color-mix(in srgb, var(--color-accent) 25%, transparent);
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.current .mark {
			animation: none;
			box-shadow: 0 0 0 3px color-mix(in srgb, var(--color-accent) 25%, transparent);
		}
	}

	.sr-only {
		position: absolute;
		width: 1px;
		height: 1px;
		overflow: hidden;
		clip: rect(0 0 0 0);
		white-space: nowrap;
	}
</style>
