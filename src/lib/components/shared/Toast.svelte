<script lang="ts">
	import { getToasts, dismissToast, type ToastType } from '$lib/state/toasts.svelte';
	import { t } from '$lib/state/i18n.svelte';

	let toasts = $derived(getToasts());

	const dotColors: Record<ToastType, string> = {
		info: 'var(--color-accent)',
		success: 'var(--color-success)',
		warning: 'var(--color-warning)',
		error: 'var(--color-danger)'
	};
</script>

{#if toasts.length > 0}
	<div class="toast-container">
		{#each toasts as toast (toast.id)}
			<div
				class="toast"
				class:dismissing={toast.dismissing}
				class:detailed={toast.type === 'error' || toast.type === 'warning'}
				role="alert"
			>
				<span class="toast-dot" style="background-color: {dotColors[toast.type]}"></span>
				<span class="toast-message" title={toast.message}>{toast.message}</span>
				<button class="toast-close" onclick={() => dismissToast(toast.id)} aria-label={t('common.dismiss')}>
					<svg width="10" height="10" viewBox="0 0 10 10" fill="none">
						<path d="M1 1L9 9M9 1L1 9" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
					</svg>
				</button>
			</div>
		{/each}
	</div>
{/if}

<style>
	.toast-container {
		position: fixed;
		top: 80px;
		left: 50%;
		transform: translateX(-50%);
		z-index: 200;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 6px;
		pointer-events: none;
	}

	.toast {
		pointer-events: auto;
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 8px 16px;
		max-width: 360px;
		background-color: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
		border-radius: 20px;
		/* The pill is sized for "Copied" and clipped a one-line message with an
		   ellipsis, which made every backend error unreadable — a failure is
		   precisely the toast worth reading in full. `.detailed` widens it and
		   lets the text wrap; the radius drops because a 20px pill looks broken
		   once it is more than one line tall. */
		box-shadow: var(--shadow-elevated);
		animation: slideDown 250ms var(--ease-default) forwards;
	}

	.toast.detailed {
		max-width: 520px;
		align-items: flex-start;
		border-radius: var(--radius-card);
	}

	.toast.detailed .toast-dot {
		/* Line it up with the first line of text rather than the block centre. */
		margin-top: 6px;
	}

	.toast.detailed .toast-message {
		white-space: normal;
		overflow-wrap: anywhere;
		/* Four lines is enough for a server message without letting a runaway
		   payload cover the app; the full text stays on the title attribute. */
		display: -webkit-box;
		-webkit-line-clamp: 4;
		line-clamp: 4;
		-webkit-box-orient: vertical;
	}

	.toast.dismissing {
		animation: slideUp 250ms var(--ease-default) forwards;
	}

	.toast-dot {
		flex-shrink: 0;
		width: 7px;
		height: 7px;
		border-radius: 50%;
	}

	.toast-message {
		flex: 1;
		font-size: 0.8125rem;
		color: var(--color-text-primary);
		line-height: 1.3;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.toast-close {
		flex-shrink: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		width: 20px;
		height: 20px;
		border: none;
		border-radius: 50%;
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
		transition: background-color 150ms ease;
	}

	.toast-close:hover {
		background-color: var(--color-surface-active);
		color: var(--color-text-primary);
	}

	@keyframes slideDown {
		from {
			opacity: 0;
			transform: translateY(-20px);
		}
		to {
			opacity: 1;
			transform: translateY(0);
		}
	}

	@keyframes slideUp {
		from {
			opacity: 1;
			transform: translateY(0);
		}
		to {
			opacity: 0;
			transform: translateY(-20px);
		}
	}
</style>
