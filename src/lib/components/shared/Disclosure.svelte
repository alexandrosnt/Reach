<!--
	A settings section that folds. The header always shows the section's name
	and, while folded, a one-line summary of its current state — so a tab with
	five sections reads as five lines, and each is one click from its detail.
-->
<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		title: string;
		/** What the section is set to right now. Shown while folded. */
		summary?: string;
		/** Explanatory line under the header. Shown while open. */
		hint?: string;
		open?: boolean;
		children: Snippet;
	}

	// Open by default: a section people cannot see is a section they cannot
	// find. Folding is offered, never imposed.
	let { title, summary = '', hint = '', open = $bindable(true), children }: Props = $props();
</script>

<section class="disclosure" class:open>
	<button class="head" aria-expanded={open} onclick={() => (open = !open)}>
		<span class="title">{title}</span>
		{#if summary && !open}
			<span class="summary">{summary}</span>
		{/if}
		<svg class="chevron" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
			<path d="M6 9l6 6 6-6" />
		</svg>
	</button>
	{#if open}
		<div class="body">
			{#if hint}<p class="hint">{hint}</p>{/if}
			{@render children()}
		</div>
	{/if}
</section>

<style>
	.disclosure {
		border-top: 1px solid var(--color-border);
	}

	.head {
		display: flex;
		align-items: center;
		gap: 12px;
		width: 100%;
		padding: 12px 0;
		border: none;
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
		text-align: left;
		font-family: inherit;
	}

	.head:hover .title {
		color: var(--color-text-primary);
	}

	.head:focus-visible {
		outline: 2px solid var(--color-accent);
		outline-offset: -2px;
		border-radius: 4px;
	}

	.title {
		font-size: 0.6875rem;
		font-weight: 600;
		letter-spacing: 0.06em;
		text-transform: uppercase;
		color: var(--color-text-tertiary);
		white-space: nowrap;
	}

	.summary {
		flex: 1;
		min-width: 0;
		text-align: right;
		font-size: 0.8125rem;
		color: var(--color-text-secondary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.chevron {
		flex-shrink: 0;
		margin-left: auto;
		color: var(--color-text-tertiary);
		transition: transform 0.15s;
	}

	.summary + .chevron {
		margin-left: 0;
	}

	.open .chevron {
		transform: rotate(180deg);
	}

	.body {
		display: flex;
		flex-direction: column;
		gap: var(--space-2, 8px);
		padding-bottom: 16px;
	}

	.hint {
		margin: 0 0 4px;
		font-size: var(--text-xs, 0.75rem);
		color: var(--color-text-secondary);
	}
</style>
