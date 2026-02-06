<script lang="ts">
	import { getUpdaterState, downloadAndInstall, dismissUpdate } from '$lib/state/updater.svelte';

	const updater = getUpdaterState();

	let visible = $derived(updater.updateAvailable && !updater.dismissed && !updater.startupBlocking);
	let errorHidden = $state(false);
	let lastError = $state<string | null>(null);

	let showError = $derived(!!updater.error && !errorHidden);

	$effect(() => {
		if (updater.error && updater.error !== lastError) {
			lastError = updater.error;
			errorHidden = false;
		}
	});

	$effect(() => {
		if (!showError) return;

		const timeout = setTimeout(() => {
			errorHidden = true;
		}, 4000);

		return () => clearTimeout(timeout);
	});

	let dismissing = $state(false);

	function handleDismiss() {
		dismissing = true;
		setTimeout(() => {
			dismissUpdate();
			dismissing = false;
		}, 250);
	}
</script>

{#if visible}
	<div class="update-banner" class:dismissing role="banner" aria-live="polite">
		<div class="banner-content">
			<span class="banner-text">
				Reach v{updater.updateVersion} is available
			</span>

			<div class="banner-actions">
				{#if updater.installing}
					<span class="status-text">Installing...</span>
				{:else if updater.downloading}
					<div class="progress-area">
						<div class="progress-track">
							<div class="progress-fill" style="width: {updater.downloadProgress}%"></div>
						</div>
						<span class="progress-text">{updater.downloadProgress}%</span>
					</div>
				{:else if showError && updater.error}
					<span class="error-text">{updater.error}</span>
				{:else}
					<button class="btn-update" onclick={downloadAndInstall}>Update Now</button>
					<button class="btn-later" onclick={handleDismiss}>Later</button>
				{/if}
			</div>
		</div>
	</div>
{/if}

<style>
	.update-banner {
		position: fixed;
		top: 0;
		left: 0;
		right: 0;
		z-index: 150;
		height: 40px;
		background: linear-gradient(135deg, var(--color-accent) 0%, #0070e0 100%);
		border-bottom: 1px solid rgba(255, 255, 255, 0.1);
		box-shadow: 0 2px 12px rgba(0, 0, 0, 0.3);
		animation: slideDown 300ms var(--ease-default) forwards;
		font-family: var(--font-sans);
	}

	.update-banner.dismissing {
		animation: slideUp 250ms var(--ease-default) forwards;
	}

	.banner-content {
		display: flex;
		align-items: center;
		justify-content: space-between;
		height: 100%;
		padding: 0 16px;
		max-width: 100%;
	}

	.banner-text {
		font-size: 0.8125rem;
		font-weight: 500;
		color: #fff;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.banner-actions {
		display: flex;
		align-items: center;
		gap: 8px;
		flex-shrink: 0;
	}

	.btn-update {
		padding: 4px 14px;
		font-size: 0.75rem;
		font-weight: 600;
		font-family: var(--font-sans);
		color: #fff;
		background: rgba(255, 255, 255, 0.15);
		border: 1px solid rgba(255, 255, 255, 0.3);
		border-radius: 6px;
		cursor: pointer;
		white-space: nowrap;
		transition:
			background-color var(--duration-default) var(--ease-default),
			transform var(--duration-default) var(--ease-default);
		user-select: none;
	}

	.btn-update:hover {
		background: rgba(255, 255, 255, 0.25);
	}

	.btn-update:active {
		transform: scale(0.97);
	}

	.btn-later {
		padding: 4px 12px;
		font-size: 0.75rem;
		font-weight: 500;
		font-family: var(--font-sans);
		color: rgba(255, 255, 255, 0.8);
		background: transparent;
		border: none;
		border-radius: 6px;
		cursor: pointer;
		white-space: nowrap;
		transition:
			color var(--duration-default) var(--ease-default),
			background-color var(--duration-default) var(--ease-default);
		user-select: none;
	}

	.btn-later:hover {
		color: #fff;
		background: rgba(255, 255, 255, 0.1);
	}

	.btn-later:active {
		transform: scale(0.97);
	}

	.progress-area {
		display: flex;
		align-items: center;
		gap: 10px;
	}

	.progress-track {
		width: 140px;
		height: 4px;
		background: rgba(255, 255, 255, 0.2);
		border-radius: 2px;
		overflow: hidden;
	}

	.progress-fill {
		height: 100%;
		background: #fff;
		border-radius: 2px;
		transition: width 200ms var(--ease-default);
	}

	.progress-text {
		font-size: 0.75rem;
		font-weight: 600;
		color: #fff;
		min-width: 36px;
		text-align: right;
	}

	.status-text {
		font-size: 0.75rem;
		font-weight: 500;
		color: rgba(255, 255, 255, 0.9);
	}

	.error-text {
		font-size: 0.75rem;
		font-weight: 500;
		color: #ffcccc;
		max-width: 240px;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	@keyframes slideDown {
		from {
			opacity: 0;
			transform: translateY(-100%);
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
			transform: translateY(-100%);
		}
	}
</style>
