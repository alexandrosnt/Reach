<!--
	The risk level of a recipe, as a single chip.

	Colour carries the meaning, so the word is always shown too — a red pill
	means nothing to someone who cannot distinguish it from the amber one.
-->
<script lang="ts">
	import { t } from '$lib/state/i18n.svelte';
	import type { Danger } from '$lib/ipc/recipes';

	interface Props {
		danger: Danger;
		compact?: boolean;
	}

	let { danger, compact = false }: Props = $props();
</script>

<span class="badge" data-danger={danger} class:compact>
	{t(`recipes.danger_${danger}`)}
</span>

<style>
	.badge {
		display: inline-flex;
		align-items: center;
		padding: 2px 8px;
		font-size: var(--text-2xs);
		font-weight: 600;
		letter-spacing: 0.02em;
		text-transform: uppercase;
		border-radius: 999px;
		border: 1px solid currentColor;
		white-space: nowrap;
	}

	.badge.compact {
		padding: 1px 6px;
	}

	.badge[data-danger='benign'] {
		color: var(--color-success);
		background: color-mix(in srgb, var(--color-success) 12%, transparent);
	}

	.badge[data-danger='mutating'] {
		color: var(--color-accent);
		background: color-mix(in srgb, var(--color-accent) 12%, transparent);
	}

	.badge[data-danger='sensitive'] {
		color: var(--color-warning);
		background: color-mix(in srgb, var(--color-warning) 14%, transparent);
	}

	.badge[data-danger='destructive'] {
		color: var(--color-danger);
		background: color-mix(in srgb, var(--color-danger) 14%, transparent);
	}
</style>
