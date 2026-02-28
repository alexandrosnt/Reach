<script lang="ts">
	import { t } from '$lib/state/i18n.svelte';
	import { getCommandOutput, isCommandRunning, clearOutput } from '$lib/state/ansible.svelte';
	import Button from '$lib/components/shared/Button.svelte';

	let scrollContainer: HTMLDivElement | undefined = $state(undefined);

	let output = $derived(getCommandOutput());
	let running = $derived(isCommandRunning());

	// Auto-scroll to bottom when new output lines arrive
	$effect(() => {
		if (output.length > 0 && scrollContainer) {
			scrollContainer.scrollTop = scrollContainer.scrollHeight;
		}
	});

	function lineClass(stream: string): string {
		if (stream === 'stderr') return 'line-stderr';
		if (stream === 'system') return 'line-system';
		return 'line-stdout';
	}
</script>

<div class="command-output">
	<div class="header">
		<div class="header-left">
			<span class="title">{t('ansible.output')}</span>
			{#if running}
				<span class="running-indicator">
					<span class="spinner"></span>
					{t('ansible.command_running')}
				</span>
			{/if}
		</div>
		{#if output.length > 0}
			<Button variant="ghost" size="sm" onclick={clearOutput}>
				{t('ansible.clear')}
			</Button>
		{/if}
	</div>

	<div class="terminal" bind:this={scrollContainer}>
		{#if output.length === 0}
			<div class="empty-message">{t('ansible.no_output')}</div>
		{:else}
			{#each output as entry, i (i)}
				<div class="output-line {lineClass(entry.stream)}">{entry.line}</div>
			{/each}
		{/if}
	</div>
</div>

<style>
	.command-output {
		display: flex;
		flex-direction: column;
		gap: 8px;
		height: 100%;
	}

	.header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		flex-shrink: 0;
	}

	.header-left {
		display: flex;
		align-items: center;
		gap: 10px;
	}

	.title {
		font-size: 0.8125rem;
		font-weight: 600;
		color: var(--color-text-primary);
		text-transform: uppercase;
		letter-spacing: 0.04em;
	}

	.running-indicator {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		font-size: 0.75rem;
		color: var(--color-accent);
		font-weight: 500;
	}

	.spinner {
		display: inline-block;
		width: 12px;
		height: 12px;
		border: 2px solid transparent;
		border-top-color: var(--color-accent);
		border-radius: 50%;
		animation: spin 0.8s linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}

	.terminal {
		flex: 1;
		min-height: 120px;
		max-height: 400px;
		overflow-y: auto;
		padding: 10px 12px;
		border-radius: var(--radius-btn);
		background-color: color-mix(in srgb, var(--color-bg-primary) 90%, black);
		border: 1px solid var(--color-border);
		font-family: monospace;
		font-size: 0.75rem;
		line-height: 1.6;
	}

	.empty-message {
		color: var(--color-text-secondary);
		font-style: italic;
		opacity: 0.6;
	}

	.output-line {
		white-space: pre-wrap;
		word-break: break-all;
	}

	.line-stdout {
		color: var(--color-text-primary);
	}

	.line-stderr {
		color: var(--color-danger);
	}

	.line-system {
		color: var(--color-accent);
	}
</style>
