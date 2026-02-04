<script lang="ts">
	import type { FileEntry } from '$lib/state/explorer.svelte';

	interface Props {
		entry: FileEntry;
		onclick: () => void;
		oncontextmenu?: (e: MouseEvent) => void;
	}

	let { entry, onclick, oncontextmenu }: Props = $props();

	let sizeText = $derived.by(() => {
		if (entry.isDirectory) return '';
		if (entry.size < 1024) return `${entry.size} B`;
		if (entry.size < 1024 * 1024) return `${(entry.size / 1024).toFixed(1)} KB`;
		if (entry.size < 1024 * 1024 * 1024) return `${(entry.size / (1024 * 1024)).toFixed(1)} MB`;
		return `${(entry.size / (1024 * 1024 * 1024)).toFixed(1)} GB`;
	});
</script>

<button class="file-node" onclick={onclick} oncontextmenu={oncontextmenu} type="button">
	<span class="file-icon">
		{#if entry.isDirectory}
			<svg width="16" height="16" viewBox="0 0 24 24" fill="none">
				<path
					d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"
					stroke="currentColor"
					stroke-width="1.5"
					stroke-linecap="round"
					stroke-linejoin="round"
				/>
			</svg>
		{:else}
			<svg width="16" height="16" viewBox="0 0 24 24" fill="none">
				<path
					d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8l-6-6z"
					stroke="currentColor"
					stroke-width="1.5"
					stroke-linecap="round"
					stroke-linejoin="round"
				/>
				<polyline
					points="14 2 14 8 20 8"
					stroke="currentColor"
					stroke-width="1.5"
					stroke-linecap="round"
					stroke-linejoin="round"
				/>
			</svg>
		{/if}
	</span>

	<span class="file-name">{entry.name}</span>

	{#if sizeText}
		<span class="file-size">{sizeText}</span>
	{/if}

	{#if entry.permissions}
		<span class="file-permissions">{entry.permissions}</span>
	{/if}
</button>

<style>
	.file-node {
		display: flex;
		align-items: center;
		gap: 8px;
		width: 100%;
		padding: 5px 8px;
		border: none;
		border-bottom: 1px solid var(--color-border);
		border-radius: 0;
		background: transparent;
		color: var(--color-text-primary);
		font-family: var(--font-sans);
		font-size: 0.75rem;
		cursor: pointer;
		text-align: left;
		transition: background-color var(--duration-default) var(--ease-default);
	}

	.file-node:hover {
		background-color: rgba(255, 255, 255, 0.06);
	}

	.file-node:active {
		background-color: rgba(255, 255, 255, 0.08);
	}

	.file-icon {
		display: flex;
		align-items: center;
		flex-shrink: 0;
		color: var(--color-text-secondary);
	}

	.file-name {
		flex: 1;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		min-width: 0;
	}

	.file-size {
		flex-shrink: 0;
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
		font-variant-numeric: tabular-nums;
	}

	.file-permissions {
		flex-shrink: 0;
		font-size: 0.6875rem;
		font-family: var(--font-mono, monospace);
		color: var(--color-text-secondary);
		opacity: 0.7;
	}
</style>
