<script lang="ts">
	import { getUpdaterState, downloadAndInstall, skipStartupUpdate } from '$lib/state/updater.svelte';
	import { t } from '$lib/state/i18n.svelte';

	interface Props {
		open: boolean;
	}

	let { open }: Props = $props();

	const updater = getUpdaterState();

	let buttonLabel = $derived.by(() => {
		if (updater.installing) return t('updater.installing');
		if (updater.downloading) return t('updater.downloading', { progress: updater.downloadProgress });
		if (updater.error) return t('updater.retry');
		return t('updater.update_now');
	});

	let buttonDisabled = $derived(updater.downloading || updater.installing);
	let showEscapeHatch = $derived(updater.downloadAttempts >= 3 && !!updater.error);

	function handleUpdate() {
		downloadAndInstall();
	}

	function handleSkip() {
		skipStartupUpdate();
	}
</script>

{#if open}
	<div class="update-backdrop" role="presentation">
		<div class="update-card" role="alertdialog" aria-modal="true" aria-label={t('updater.title')}>
			<div class="card-header">
				<div class="icon-container">
					<svg width="32" height="32" viewBox="0 0 32 32" fill="none" xmlns="http://www.w3.org/2000/svg">
						<path d="M16 3L17.5 8.5L20 6L19 11L24 9.5L20.5 13L26 13.5L21 16L26 18.5L20.5 19L24 22.5L19 21L20 26L17.5 23.5L16 29L14.5 23.5L12 26L13 21L8 22.5L11.5 19L6 18.5L11 16L6 13.5L11.5 13L8 9.5L13 11L12 6L14.5 8.5L16 3Z" fill="var(--color-accent)" fill-opacity="0.15" stroke="var(--color-accent)" stroke-width="1.5" stroke-linejoin="round"/>
						<path d="M16 10V18M16 18L12.5 14.5M16 18L19.5 14.5" stroke="var(--color-accent)" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
						<line x1="12" y1="21" x2="20" y2="21" stroke="var(--color-accent)" stroke-width="1.5" stroke-linecap="round"/>
					</svg>
				</div>
				<h2 class="title">{t('updater.title')}</h2>
				{#if updater.updateVersion}
					<span class="version-badge">v{updater.updateVersion}</span>
				{/if}
			</div>

			{#if updater.updateNotes}
				<div class="release-notes">
					<h3 class="release-notes-heading">{t('updater.release_notes')}</h3>
					<div class="release-notes-content">
						<pre class="release-notes-text">{updater.updateNotes}</pre>
					</div>
				</div>
			{/if}

			{#if updater.downloading || updater.installing}
				<div class="progress-container">
					<div class="progress-track">
						<div
							class="progress-fill"
							class:installing={updater.installing}
							style="width: {updater.installing ? 100 : updater.downloadProgress}%"
						></div>
					</div>
					{#if updater.installing}
						<p class="progress-label">{t('updater.installing_desc')}</p>
					{/if}
				</div>
			{/if}

			{#if updater.error}
				<div class="error-message">
					<svg width="14" height="14" viewBox="0 0 14 14" fill="none">
						<circle cx="7" cy="7" r="6" stroke="var(--color-danger)" stroke-width="1.2"/>
						<path d="M7 4V7.5M7 9.5V10" stroke="var(--color-danger)" stroke-width="1.2" stroke-linecap="round"/>
					</svg>
					<span>{updater.error}</span>
				</div>
			{/if}

			<div class="actions">
				{#if showEscapeHatch}
					<button class="btn-ghost" onclick={handleSkip}>
						{t('updater.continue_without')}
					</button>
				{/if}
				<button
					class="btn-primary"
					disabled={buttonDisabled}
					onclick={handleUpdate}
				>
					{#if updater.downloading}
						<svg class="spinner" width="16" height="16" viewBox="0 0 16 16" fill="none">
							<circle cx="8" cy="8" r="6" stroke="rgba(255,255,255,0.25)" stroke-width="2"/>
							<path d="M14 8A6 6 0 0 0 8 2" stroke="white" stroke-width="2" stroke-linecap="round"/>
						</svg>
					{/if}
					{buttonLabel}
				</button>
			</div>
		</div>
	</div>
{/if}

<style>
	.update-backdrop {
		position: fixed;
		inset: 0;
		z-index: 300;
		display: flex;
		align-items: center;
		justify-content: center;
		background: rgba(0, 0, 0, 0.6);
		backdrop-filter: blur(8px);
		-webkit-backdrop-filter: blur(8px);
		animation: fadeIn var(--duration-default, 200ms) var(--ease-default, cubic-bezier(0.25, 0.1, 0.25, 1));
	}

	.update-card {
		background-color: var(--color-bg-elevated, #1c1c1e);
		border: 1px solid var(--color-border, rgba(255, 255, 255, 0.08));
		border-radius: var(--radius-modal, 16px);
		box-shadow: var(--shadow-elevated, 0 8px 32px rgba(0, 0, 0, 0.25));
		width: 90%;
		max-width: 440px;
		display: flex;
		flex-direction: column;
		overflow: hidden;
		animation: scaleIn var(--duration-default, 200ms) var(--ease-default, cubic-bezier(0.25, 0.1, 0.25, 1));
		font-family: var(--font-sans, 'Inter', system-ui, -apple-system, sans-serif);
	}

	.card-header {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 12px;
		padding: 28px 24px 16px;
	}

	.icon-container {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 56px;
		height: 56px;
		border-radius: 14px;
		background: rgba(10, 132, 255, 0.1);
	}

	.title {
		font-size: 1.125rem;
		font-weight: 600;
		color: var(--color-text-primary, #f5f5f7);
		margin: 0;
		letter-spacing: -0.01em;
	}

	.version-badge {
		display: inline-flex;
		align-items: center;
		padding: 3px 10px;
		border-radius: 20px;
		font-size: 0.75rem;
		font-weight: 500;
		color: var(--color-accent, #0a84ff);
		background: rgba(10, 132, 255, 0.12);
		letter-spacing: 0.01em;
	}

	.release-notes {
		padding: 0 24px 16px;
	}

	.release-notes-heading {
		font-size: 0.75rem;
		font-weight: 500;
		color: var(--color-text-secondary, #86868b);
		margin: 0 0 8px 0;
		text-transform: uppercase;
		letter-spacing: 0.04em;
	}

	.release-notes-content {
		max-height: 200px;
		overflow-y: auto;
		border-radius: 8px;
		background: rgba(255, 255, 255, 0.03);
		border: 1px solid var(--color-border, rgba(255, 255, 255, 0.08));
	}

	.release-notes-text {
		margin: 0;
		padding: 12px;
		font-family: var(--font-sans, 'Inter', system-ui, -apple-system, sans-serif);
		font-size: 0.8125rem;
		line-height: 1.55;
		color: var(--color-text-secondary, #86868b);
		white-space: pre-wrap;
		word-wrap: break-word;
	}

	.release-notes-content::-webkit-scrollbar {
		width: 6px;
	}

	.release-notes-content::-webkit-scrollbar-track {
		background: transparent;
	}

	.release-notes-content::-webkit-scrollbar-thumb {
		background: rgba(255, 255, 255, 0.1);
		border-radius: 3px;
	}

	.progress-container {
		padding: 0 24px 16px;
	}

	.progress-track {
		height: 6px;
		border-radius: 3px;
		background: rgba(255, 255, 255, 0.08);
		overflow: hidden;
	}

	.progress-fill {
		height: 100%;
		border-radius: 3px;
		background: var(--color-accent, #0a84ff);
		transition: width 300ms var(--ease-default, cubic-bezier(0.25, 0.1, 0.25, 1));
	}

	.progress-fill.installing {
		animation: pulse 1.5s ease-in-out infinite;
	}

	.progress-label {
		margin: 8px 0 0 0;
		font-size: 0.75rem;
		color: var(--color-text-secondary, #86868b);
		text-align: center;
	}

	.error-message {
		display: flex;
		align-items: flex-start;
		gap: 8px;
		margin: 0 24px 16px;
		padding: 10px 12px;
		border-radius: 8px;
		background: rgba(255, 69, 58, 0.08);
		border: 1px solid rgba(255, 69, 58, 0.15);
		font-size: 0.8125rem;
		color: var(--color-danger, #ff453a);
		line-height: 1.4;
	}

	.error-message svg {
		flex-shrink: 0;
		margin-top: 1px;
	}

	.actions {
		display: flex;
		align-items: center;
		justify-content: flex-end;
		gap: 8px;
		padding: 16px 24px 24px;
	}

	.btn-primary {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		gap: 8px;
		padding: 10px 20px;
		border-radius: 10px;
		border: none;
		font-family: var(--font-sans, 'Inter', system-ui, -apple-system, sans-serif);
		font-size: 0.875rem;
		font-weight: 500;
		color: #fff;
		background-color: var(--color-accent, #0a84ff);
		cursor: pointer;
		transition:
			background-color var(--duration-default, 200ms) var(--ease-default, cubic-bezier(0.25, 0.1, 0.25, 1)),
			opacity var(--duration-default, 200ms) var(--ease-default, cubic-bezier(0.25, 0.1, 0.25, 1)),
			transform var(--duration-default, 200ms) var(--ease-default, cubic-bezier(0.25, 0.1, 0.25, 1));
		user-select: none;
		white-space: nowrap;
	}

	.btn-primary:hover:not(:disabled) {
		background-color: var(--color-accent-hover, #409cff);
	}

	.btn-primary:active:not(:disabled) {
		transform: scale(0.97);
	}

	.btn-primary:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.btn-ghost {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		padding: 10px 16px;
		border-radius: 10px;
		border: none;
		font-family: var(--font-sans, 'Inter', system-ui, -apple-system, sans-serif);
		font-size: 0.8125rem;
		font-weight: 500;
		color: var(--color-text-secondary, #86868b);
		background: transparent;
		cursor: pointer;
		transition:
			background-color var(--duration-default, 200ms) var(--ease-default, cubic-bezier(0.25, 0.1, 0.25, 1)),
			color var(--duration-default, 200ms) var(--ease-default, cubic-bezier(0.25, 0.1, 0.25, 1));
		user-select: none;
		white-space: nowrap;
	}

	.btn-ghost:hover {
		background: rgba(255, 255, 255, 0.06);
		color: var(--color-text-primary, #f5f5f7);
	}

	.btn-ghost:active {
		transform: scale(0.97);
	}

	.spinner {
		animation: spin 0.8s linear infinite;
	}

	@keyframes fadeIn {
		from {
			opacity: 0;
		}
		to {
			opacity: 1;
		}
	}

	@keyframes scaleIn {
		from {
			opacity: 0;
			transform: scale(0.95);
		}
		to {
			opacity: 1;
			transform: scale(1);
		}
	}

	@keyframes pulse {
		0%, 100% {
			opacity: 1;
		}
		50% {
			opacity: 0.6;
		}
	}

	@keyframes spin {
		from {
			transform: rotate(0deg);
		}
		to {
			transform: rotate(360deg);
		}
	}
</style>
