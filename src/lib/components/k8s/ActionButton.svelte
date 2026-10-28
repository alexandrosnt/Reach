<script lang="ts">
	/**
	 * A detail-pane action. On a read-only cluster it stays visible but off,
	 * with the reason as its tooltip: a hidden button teaches nothing, a
	 * disabled one says where the switch is. The tooltip sits on a wrapper
	 * because some engines show no title for a disabled button.
	 */
	import type { Snippet } from 'svelte';
	import { t } from '#lib/state/i18n.svelte.js';

	interface Props {
		onclick: () => void;
		/** A change on a read-only cluster: drawn disabled, with the reason. */
		locked?: boolean;
		disabled?: boolean;
		title?: string;
		variant?: 'default' | 'primary' | 'danger';
		children: Snippet;
	}

	let { onclick, locked = false, disabled = false, title = '', variant = 'default', children }: Props = $props();
</script>

<span class="wrap" title={locked ? t('env.read_only_hint') : title}>
	<button type="button" class="act {variant}" disabled={locked || disabled} {onclick}>
		{@render children()}
	</button>
</span>

<style>
	.wrap {
		display: inline-flex;
	}

	.act {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		min-height: 32px;
		padding: 0 12px;
		border-radius: var(--radius-btn);
		border: 1px solid var(--color-border);
		background: var(--color-bg-elevated);
		color: var(--color-text-primary);
		font: inherit;
		font-size: var(--text-sm);
		white-space: nowrap;
		cursor: pointer;
	}

	.act:hover:not(:disabled) {
		background: var(--color-surface-hover);
	}

	.act:focus-visible {
		outline: 2px solid var(--color-accent);
		outline-offset: 1px;
	}

	.act:disabled {
		opacity: 0.45;
		cursor: not-allowed;
	}

	.primary {
		border-color: var(--color-accent);
		color: var(--color-accent);
	}

	.danger {
		border-color: color-mix(in srgb, var(--color-danger) 50%, var(--color-border));
		color: var(--color-danger);
	}

	@media (pointer: coarse) {
		.act {
			min-height: 44px;
		}
	}
</style>
