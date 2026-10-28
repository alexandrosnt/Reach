<script lang="ts">
	/**
	 * A state as a shape and words together: never colour alone. The
	 * engine's or cluster's own term is the tooltip.
	 */
	import type { Status } from '#lib/devops/status.js';

	let { status, compact = false }: { status: Status; compact?: boolean } = $props();

	const SHAPES = { ok: '●', busy: '◌', warn: '▲', bad: '✕', idle: '■' } as const;
</script>

<span class="pill {status.tone}" class:compact title={status.raw}>
	<span class="shape" class:spin={status.tone === 'busy'} aria-hidden="true">{SHAPES[status.tone]}</span>
	<span class="label">{status.label}</span>
</span>

<style>
	.pill {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		max-width: 100%;
		padding: 2px 8px 2px 6px;
		border-radius: 999px;
		font-size: var(--text-xs);
		font-weight: 500;
		line-height: 1.5;
		white-space: nowrap;
		color: var(--tone);
		background: color-mix(in srgb, var(--tone) 12%, transparent);
	}

	.pill.compact {
		padding: 0;
		background: none;
	}

	.label {
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.shape {
		font-size: 0.7em;
		line-height: 1;
	}

	.spin {
		display: inline-block;
		animation: spin 1.2s linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.spin {
			animation: none;
		}
	}

	.ok {
		--tone: var(--color-success);
	}
	.busy {
		--tone: var(--color-accent);
	}
	.warn {
		--tone: var(--color-warning);
	}
	.bad {
		--tone: var(--color-danger);
	}
	.idle {
		--tone: var(--color-text-secondary);
	}
</style>
