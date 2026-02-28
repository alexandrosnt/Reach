<script lang="ts">
	import { onMount } from 'svelte';
	import { t } from '$lib/state/i18n.svelte';
	import { generateHcl, writeGeneratedFiles } from '$lib/state/tofu.svelte';
	import type { GeneratedFile } from '$lib/ipc/tofu';
	import Button from '$lib/components/shared/Button.svelte';

	interface Props {
		onclose: () => void;
	}

	let { onclose }: Props = $props();

	let files = $state<GeneratedFile[]>([]);
	let loading = $state(true);
	let activeFileIndex = $state(0);
	let writing = $state(false);

	let activeFile = $derived(files[activeFileIndex]);

	onMount(() => {
		generateHcl()
			.then((result) => {
				files = result.files;
			})
			.catch(() => {
				files = [];
			})
			.finally(() => {
				loading = false;
			});
	});

	function onBackdropClick(e: MouseEvent) {
		if (e.target === e.currentTarget) {
			onclose();
		}
	}

	async function handleWrite() {
		writing = true;
		try {
			await writeGeneratedFiles(files);
		} finally {
			onclose();
		}
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="backdrop glass" onclick={onBackdropClick} onkeydown={() => {}}>
	<div class="modal" role="dialog" aria-modal="true" aria-label={t('tofu.hcl_preview')}>
		<header class="modal-header">
			<h2 class="modal-title">{t('tofu.hcl_preview')}</h2>
			<button class="modal-close" onclick={onclose} aria-label={t('common.close_dialog')}>
				<svg width="14" height="14" viewBox="0 0 14 14" fill="none">
					<path
						d="M1 1L13 13M13 1L1 13"
						stroke="currentColor"
						stroke-width="1.5"
						stroke-linecap="round"
					/>
				</svg>
			</button>
		</header>

		{#if loading}
			<div class="loading-state">
				<p class="loading-text">{t('common.loading')}...</p>
			</div>
		{:else if files.length === 0}
			<div class="empty-state">
				<p class="empty-text">{t('tofu.no_providers_to_generate')}</p>
			</div>
		{:else}
			<div class="tab-bar">
				{#each files as file, i (file.filename)}
					<button
						class="tab"
						class:active={i === activeFileIndex}
						onclick={() => (activeFileIndex = i)}
					>
						{file.filename}
					</button>
				{/each}
			</div>

			<div class="content-area">
				{#if activeFile}
					<pre>{activeFile.content}</pre>
				{/if}
			</div>

			<footer class="modal-footer">
				<p class="overwrite-warning">{t('tofu.overwrite_warning')}</p>
				<div class="footer-actions">
					<Button variant="secondary" size="sm" onclick={onclose}>
						{t('common.cancel')}
					</Button>
					<Button variant="primary" size="sm" disabled={writing} onclick={handleWrite}>
						{writing ? t('common.loading') : t('tofu.write_files')}
					</Button>
				</div>
			</footer>
		{/if}
	</div>
</div>

<style>
	.backdrop {
		position: fixed;
		inset: 0;
		z-index: 100;
		display: flex;
		align-items: center;
		justify-content: center;
		animation: fadeIn var(--duration-default) var(--ease-default);
	}

	.modal {
		background-color: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-modal);
		box-shadow: var(--shadow-elevated);
		width: 90%;
		max-width: 800px;
		max-height: 80vh;
		display: flex;
		flex-direction: column;
		overflow: hidden;
		animation: scaleIn var(--duration-default) var(--ease-default);
	}

	.modal-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 16px 20px;
		border-bottom: 1px solid var(--color-border);
	}

	.modal-title {
		font-size: 1rem;
		font-weight: 600;
		color: var(--color-text-primary);
		margin: 0;
	}

	.modal-close {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 28px;
		height: 28px;
		border: none;
		border-radius: 6px;
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
		transition: background-color var(--duration-default) var(--ease-default);
	}

	.modal-close:hover {
		background-color: rgba(255, 255, 255, 0.08);
		color: var(--color-text-primary);
	}

	.loading-state,
	.empty-state {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 64px 32px;
	}

	.loading-text,
	.empty-text {
		margin: 0;
		font-size: 0.875rem;
		color: var(--color-text-secondary);
	}

	.tab-bar {
		display: flex;
		gap: 0;
		border-bottom: 1px solid var(--color-border);
		overflow-x: auto;
		flex-shrink: 0;
	}

	.tab {
		padding: 10px 16px;
		font-size: 0.8125rem;
		font-weight: 500;
		font-family: var(--font-mono, monospace);
		color: var(--color-text-secondary);
		background: transparent;
		border: none;
		border-bottom: 2px solid transparent;
		cursor: pointer;
		white-space: nowrap;
		transition:
			color var(--duration-default) var(--ease-default),
			border-color var(--duration-default) var(--ease-default);
	}

	.tab:hover {
		color: var(--color-text-primary);
	}

	.tab.active {
		color: var(--color-accent);
		border-bottom-color: var(--color-accent);
	}

	.content-area {
		flex: 1;
		overflow: auto;
		background: var(--color-bg-primary);
		min-height: 0;
	}

	.content-area pre {
		margin: 0;
		padding: 16px 20px;
		font-family: var(--font-mono, monospace);
		font-size: 0.8125rem;
		line-height: 1.6;
		color: var(--color-text-primary);
		white-space: pre;
		tab-size: 2;
	}

	.modal-footer {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
		padding: 14px 20px;
		border-top: 1px solid var(--color-border);
	}

	.overwrite-warning {
		margin: 0;
		font-size: 0.75rem;
		color: var(--color-text-secondary);
		flex: 1;
		min-width: 0;
	}

	.footer-actions {
		display: flex;
		gap: 8px;
		flex-shrink: 0;
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
</style>
