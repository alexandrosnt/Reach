<script lang="ts">
	import { getSnippetsByCategory, type Snippet, type SnippetCategory } from '$lib/state/snippets.svelte';
	import { sshSend } from '$lib/ipc/ssh';
	import { ptyWrite } from '$lib/ipc/pty';
	import { addToast } from '$lib/state/toasts.svelte';
	import { getActiveTab } from '$lib/state/tabs.svelte';
	import { t } from '$lib/state/i18n.svelte';

	interface Props {
		connectionId?: string;
	}

	let { connectionId }: Props = $props();

	const categories: { id: SnippetCategory; label: string }[] = [
		{ id: 'terraform', label: 'Terraform' },
		{ id: 'docker', label: 'Docker' },
		{ id: 'kubernetes', label: 'K8s' },
		{ id: 'git', label: 'Git' },
		{ id: 'system', label: 'System' }
	];

	let activeCategory = $state<SnippetCategory>('terraform');
	let snippets = $derived(getSnippetsByCategory(activeCategory));
	let activeTab = $derived(getActiveTab());

	async function runSnippet(snippet: Snippet): Promise<void> {
		const tab = activeTab;
		if (!tab) {
			addToast(t('snippet.no_terminal'), 'error');
			return;
		}

		const command = snippet.command;
		const encoded = Array.from(new TextEncoder().encode(command));

		try {
			if (tab.type === 'ssh' && tab.connectionId) {
				await sshSend(tab.connectionId, encoded);
			} else if (tab.type === 'local') {
				await ptyWrite(tab.id, encoded);
			} else {
				addToast(t('snippet.wrong_tab_type'), 'error');
				return;
			}
			addToast(t('snippet.inserted', { name: snippet.name }), 'info');
		} catch (err) {
			addToast(`Failed to run snippet: ${err}`, 'error');
		}
	}

	function copySnippet(e: MouseEvent, snippet: Snippet): void {
		e.stopPropagation();
		navigator.clipboard.writeText(snippet.command);
		addToast(t('snippet.copied'), 'info');
	}
</script>

<div class="snippet-list">
	<div class="category-tabs">
		{#each categories as cat (cat.id)}
			<button
				class="category-tab"
				class:active={activeCategory === cat.id}
				onclick={() => (activeCategory = cat.id)}
			>
				{cat.label}
			</button>
		{/each}
	</div>

	<div class="snippets">
		{#each snippets as snippet (snippet.id)}
			<!-- svelte-ignore a11y_click_events_have_key_events -->
			<!-- svelte-ignore a11y_no_static_element_interactions -->
			<div class="snippet-item" onclick={() => runSnippet(snippet)} title={snippet.description}>
				<div class="snippet-header">
					<span class="snippet-name">{snippet.name}</span>
					<button
						class="copy-btn"
						onclick={(e) => copySnippet(e, snippet)}
						title={t('snippet.copy_tooltip')}
					>
						<svg width="12" height="12" viewBox="0 0 24 24" fill="none">
							<rect x="9" y="9" width="13" height="13" rx="2" stroke="currentColor" stroke-width="2" />
							<path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1" stroke="currentColor" stroke-width="2" />
						</svg>
					</button>
				</div>
				<code class="snippet-command">{snippet.command}</code>
			</div>
		{/each}
	</div>

	<div class="snippet-hint">
		{t('snippet.insert_tooltip')}
	</div>
</div>

<style>
	.snippet-list {
		display: flex;
		flex-direction: column;
		height: 100%;
		overflow: hidden;
	}

	.category-tabs {
		display: flex;
		gap: 1px;
		padding: 6px 8px;
		border-bottom: 1px solid var(--color-border);
		overflow-x: auto;
		scrollbar-width: none;
	}

	.category-tabs::-webkit-scrollbar {
		display: none;
	}

	.category-tab {
		padding: 4px 10px;
		font-size: 0.6875rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.03em;
		color: var(--color-text-secondary);
		background: transparent;
		border: 1px solid transparent;
		border-radius: var(--radius-btn);
		cursor: pointer;
		transition: all var(--duration-default) var(--ease-default);
		flex-shrink: 0;
		white-space: nowrap;
	}

	.category-tab:hover {
		color: var(--color-text-primary);
		background-color: rgba(255, 255, 255, 0.04);
	}

	.category-tab.active {
		color: var(--color-accent);
		border-color: var(--color-accent);
		background-color: rgba(10, 132, 255, 0.08);
	}

	.snippets {
		flex: 1;
		overflow-y: auto;
		padding: 6px;
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.snippet-item {
		display: flex;
		flex-direction: column;
		gap: 4px;
		padding: 8px 10px;
		background-color: rgba(255, 255, 255, 0.02);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		cursor: pointer;
		text-align: left;
		transition: all var(--duration-default) var(--ease-default);
	}

	.snippet-item:hover {
		background-color: rgba(255, 255, 255, 0.05);
		border-color: var(--color-accent);
	}

	.snippet-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
	}

	.snippet-name {
		font-size: 0.8125rem;
		font-weight: 500;
		color: var(--color-text-primary);
	}

	.copy-btn {
		padding: 4px;
		color: var(--color-text-secondary);
		background: transparent;
		border: none;
		border-radius: 4px;
		cursor: pointer;
		opacity: 0;
		transition: all var(--duration-default) var(--ease-default);
	}

	.snippet-item:hover .copy-btn {
		opacity: 1;
	}

	.copy-btn:hover {
		color: var(--color-text-primary);
		background-color: rgba(255, 255, 255, 0.1);
	}

	.snippet-command {
		font-family: var(--font-mono, 'JetBrains Mono', monospace);
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
		background-color: rgba(0, 0, 0, 0.2);
		padding: 4px 6px;
		border-radius: 4px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.snippet-hint {
		padding: 8px;
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
		text-align: center;
		border-top: 1px solid var(--color-border);
		opacity: 0.6;
	}
</style>
