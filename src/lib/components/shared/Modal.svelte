<script lang="ts">
	import type { Snippet } from 'svelte';
	import { t } from '$lib/state/i18n.svelte';

	interface Props {
		open: boolean;
		onclose: () => void;
		title?: string;
		maxWidth?: string;
		children: Snippet;
		actions?: Snippet;
	}

	let {
		open,
		onclose,
		title = '',
		maxWidth,
		children,
		actions
	}: Props = $props();

	$effect(() => {
		if (!open) return;

		function onKeydown(e: KeyboardEvent) {
			if (e.key === 'Escape') {
				onclose();
			}
		}

		document.addEventListener('keydown', onKeydown);

		return () => {
			document.removeEventListener('keydown', onKeydown);
		};
	});

	function onBackdropClick(e: MouseEvent) {
		if (e.target === e.currentTarget) {
			onclose();
		}
	}
</script>

{#if open}
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div
		class="backdrop glass"
		onkeydown={() => {}}
		onclick={onBackdropClick}
	>
		<div class="modal" role="dialog" aria-modal="true" aria-label={title || t('common.close_dialog')} style:max-width={maxWidth}>
			{#if title}
				<header class="modal-header">
					<h2 class="modal-title">{title}</h2>
					<button class="modal-close" onclick={onclose} aria-label={t('common.close_dialog')}>
						<svg width="14" height="14" viewBox="0 0 14 14" fill="none">
							<path d="M1 1L13 13M13 1L1 13" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
						</svg>
					</button>
				</header>
			{/if}

			<div class="modal-body">
				{@render children()}
			</div>

			{#if actions}
				<footer class="modal-actions">
					{@render actions()}
				</footer>
			{/if}
		</div>
	</div>
{/if}

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
		min-width: 320px;
		max-width: 520px;
		width: 90%;
		max-height: 85vh;
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

	.modal-body {
		padding: 20px;
		overflow-y: auto;
		flex: 1;
	}

	.modal-actions {
		display: flex;
		align-items: center;
		justify-content: flex-end;
		gap: 8px;
		padding: 16px 20px;
		border-top: 1px solid var(--color-border);
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
