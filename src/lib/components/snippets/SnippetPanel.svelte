<script lang="ts">
	import { getSnippets, loadSnippets, addSnippet, removeSnippet, editSnippet } from '$lib/state/snippets.svelte';
	import { addToast } from '$lib/state/toasts.svelte';
	import { t } from '$lib/state/i18n.svelte';
	import { getActiveTab } from '$lib/state/tabs.svelte';
	import { sshSend } from '$lib/ipc/ssh';
	import { ptyWrite } from '$lib/ipc/pty';
	import { onMount } from 'svelte';

	interface Props {
		connectionId?: string;
	}

	let { connectionId }: Props = $props();

	let snippets = $derived(getSnippets());
	let activeTab = $derived(getActiveTab());

	let search = $state('');
	let showAdd = $state(false);
	let newName = $state('');
	let newCommand = $state('');
	let newDescription = $state('');
	let newTags = $state('');
	let editingId = $state<string | null>(null);

	let filtered = $derived.by(() => {
		if (!search.trim()) return snippets;
		const q = search.toLowerCase();
		return snippets.filter(s =>
			s.name.toLowerCase().includes(q) ||
			s.command.toLowerCase().includes(q) ||
			s.tags.some(t => t.toLowerCase().includes(q))
		);
	});

	function pasteToTerminal(command: string): void {
		if (!activeTab) return;
		const connId = activeTab.connectionId;
		if (!connId) return;
		const data = Array.from(new TextEncoder().encode(command));
		if (activeTab.type === 'ssh') {
			sshSend(connId, data);
		} else {
			ptyWrite(connId, data);
		}
		addToast(`Pasted: ${command.length > 40 ? command.slice(0, 40) + '...' : command}`, 'success');
	}

	function executeSnippet(command: string): void {
		pasteToTerminal(command + '\n');
	}

	async function handleAdd(): Promise<void> {
		if (!newName.trim() || !newCommand.trim()) return;
		try {
			await addSnippet(
				newName.trim(),
				newCommand.trim(),
				newDescription.trim() || undefined,
				newTags.split(',').map(t => t.trim()).filter(Boolean)
			);
			newName = '';
			newCommand = '';
			newDescription = '';
			newTags = '';
			showAdd = false;
		} catch (err) {
			addToast(String(err), 'error');
		}
	}

	async function handleDelete(id: string): Promise<void> {
		try {
			await removeSnippet(id);
		} catch (err) {
			addToast(String(err), 'error');
		}
	}

	function startEdit(snippet: typeof snippets[0]): void {
		editingId = snippet.id;
		newName = snippet.name;
		newCommand = snippet.command;
		newDescription = snippet.description ?? '';
		newTags = snippet.tags.join(', ');
	}

	async function handleEdit(): Promise<void> {
		if (!editingId || !newName.trim() || !newCommand.trim()) return;
		try {
			await editSnippet({
				id: editingId,
				name: newName.trim(),
				command: newCommand.trim(),
				description: newDescription.trim() || undefined,
				tags: newTags.split(',').map(t => t.trim()).filter(Boolean),
			});
			editingId = null;
			newName = '';
			newCommand = '';
			newDescription = '';
			newTags = '';
		} catch (err) {
			addToast(String(err), 'error');
		}
	}

	function cancelEdit(): void {
		editingId = null;
		showAdd = false;
		newName = '';
		newCommand = '';
		newDescription = '';
		newTags = '';
	}

	onMount(() => {
		loadSnippets();
	});
</script>

<div class="snippet-panel">
	<div class="panel-actions">
		<div class="search-row">
			<input class="search-input" type="text" placeholder="Search snippets..." bind:value={search} />
		</div>
		<button class="add-btn" onclick={() => (showAdd = true)}>
			<svg width="11" height="11" viewBox="0 0 24 24" fill="none">
				<path d="M12 5v14M5 12h14" stroke="currentColor" stroke-width="2" stroke-linecap="round" />
			</svg>
			Add
		</button>
	</div>

	{#if showAdd || editingId}
		<form class="snippet-form" onsubmit={(e) => { e.preventDefault(); editingId ? handleEdit() : handleAdd(); }}>
			<input class="form-input" type="text" placeholder="Name" bind:value={newName} />
			<textarea class="form-input form-textarea" placeholder="Command (e.g. systemctl restart nginx)" bind:value={newCommand} rows="2"></textarea>
			<input class="form-input" type="text" placeholder="Description (optional)" bind:value={newDescription} />
			<input class="form-input" type="text" placeholder="Tags (comma separated)" bind:value={newTags} />
			<div class="form-actions">
				<button class="form-cancel" type="button" onclick={cancelEdit}>Cancel</button>
				<button class="form-save" type="submit" disabled={!newName.trim() || !newCommand.trim()}>
					{editingId ? 'Update' : 'Save'}
				</button>
			</div>
		</form>
	{/if}

	<div class="snippet-list">
		{#each filtered as snippet (snippet.id)}
			<div class="snippet-card">
				<div class="snippet-header">
					<span class="snippet-name">{snippet.name}</span>
					<div class="snippet-actions">
						<button class="action-btn" onclick={() => pasteToTerminal(snippet.command)} title="Paste">
							<svg width="11" height="11" viewBox="0 0 24 24" fill="none">
								<rect x="9" y="9" width="13" height="13" rx="2" stroke="currentColor" stroke-width="1.5"/>
								<path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1" stroke="currentColor" stroke-width="1.5"/>
							</svg>
						</button>
						<button class="action-btn run-btn" onclick={() => executeSnippet(snippet.command)} title="Run">
							<svg width="11" height="11" viewBox="0 0 24 24" fill="currentColor">
								<path d="M8 5v14l11-7z"/>
							</svg>
						</button>
						<button class="action-btn" onclick={() => startEdit(snippet)} title="Edit">
							<svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
								<path d="M17 3a2.85 2.85 0 014 4L7.5 20.5 2 22l1.5-5.5Z"/>
							</svg>
						</button>
						<button class="action-btn delete-btn" onclick={() => handleDelete(snippet.id)} title="Delete">
							<svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
								<path d="M3 6h18M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2"/>
							</svg>
						</button>
					</div>
				</div>
				<pre class="snippet-command">{snippet.command}</pre>
				{#if snippet.description}
					<span class="snippet-desc">{snippet.description}</span>
				{/if}
				{#if snippet.tags.length > 0}
					<div class="snippet-tags">
						{#each snippet.tags as tag}
							<span class="tag">{tag}</span>
						{/each}
					</div>
				{/if}
			</div>
		{:else}
			<p class="empty">{search ? 'No matches' : 'No snippets yet'}</p>
		{/each}
	</div>
</div>

<style>
	.snippet-panel {
		display: flex;
		flex-direction: column;
		gap: 6px;
		padding: 4px 0;
		height: 100%;
	}

	.panel-actions {
		display: flex;
		gap: 4px;
		align-items: center;
	}

	.search-row {
		flex: 1;
		min-width: 0;
	}

	.search-input {
		width: 100%;
		padding: 5px 8px;
		border: 1px solid var(--color-border);
		border-radius: 6px;
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		font-family: var(--font-sans);
		font-size: 0.6875rem;
		outline: none;
		box-sizing: border-box;
	}

	.search-input:focus { border-color: var(--color-accent); }

	.add-btn {
		display: flex;
		align-items: center;
		gap: 3px;
		padding: 5px 8px;
		font-family: var(--font-sans);
		font-size: 0.6875rem;
		font-weight: 500;
		color: var(--color-accent);
		background: transparent;
		border: 1px solid var(--color-accent);
		border-radius: 6px;
		cursor: pointer;
		flex-shrink: 0;
		transition: background-color 0.15s ease;
	}

	.add-btn:hover { background: rgba(10, 132, 255, 0.1); }

	.snippet-form {
		display: flex;
		flex-direction: column;
		gap: 6px;
		padding: 8px;
		background: rgba(255, 255, 255, 0.02);
		border: 1px solid var(--color-border);
		border-radius: 8px;
	}

	.form-input {
		padding: 5px 8px;
		border: 1px solid var(--color-border);
		border-radius: 5px;
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		font-family: var(--font-sans);
		font-size: 0.6875rem;
		outline: none;
		box-sizing: border-box;
	}

	.form-input:focus { border-color: var(--color-accent); }

	.form-textarea {
		font-family: var(--font-mono, monospace);
		resize: vertical;
		min-height: 40px;
	}

	.form-actions {
		display: flex;
		justify-content: flex-end;
		gap: 4px;
	}

	.form-save, .form-cancel {
		padding: 4px 10px;
		font-family: var(--font-sans);
		font-size: 0.625rem;
		font-weight: 500;
		border: none;
		border-radius: 4px;
		cursor: pointer;
	}

	.form-save { background: var(--color-accent); color: #fff; }
	.form-save:disabled { opacity: 0.4; cursor: default; }
	.form-cancel { background: transparent; color: var(--color-text-secondary); }

	.snippet-list {
		display: flex;
		flex-direction: column;
		gap: 4px;
		overflow-y: auto;
		flex: 1;
	}

	.snippet-card {
		display: flex;
		flex-direction: column;
		gap: 4px;
		padding: 6px 8px;
		border-radius: 6px;
		transition: background-color 0.15s ease;
	}

	.snippet-card:hover { background: rgba(255, 255, 255, 0.04); }

	.snippet-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 4px;
	}

	.snippet-name {
		font-size: 0.6875rem;
		font-weight: 500;
		color: var(--color-text-primary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		flex: 1;
		min-width: 0;
	}

	.snippet-actions {
		display: flex;
		gap: 2px;
		opacity: 0;
		transition: opacity 0.15s ease;
	}

	.snippet-card:hover .snippet-actions { opacity: 1; }

	.action-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 22px;
		height: 22px;
		border: none;
		border-radius: 4px;
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
		transition: background-color 0.1s ease, color 0.1s ease;
	}

	.action-btn:hover { background: rgba(255, 255, 255, 0.08); color: var(--color-text-primary); }
	.run-btn:hover { color: var(--color-accent); }
	.delete-btn:hover { color: var(--color-danger); }

	.snippet-command {
		margin: 0;
		padding: 4px 6px;
		font-family: var(--font-mono, monospace);
		font-size: 0.625rem;
		color: var(--color-text-secondary);
		background: rgba(255, 255, 255, 0.03);
		border-radius: 4px;
		overflow-x: auto;
		white-space: pre-wrap;
		word-break: break-all;
	}

	.snippet-desc {
		font-size: 0.5625rem;
		color: var(--color-text-secondary);
		opacity: 0.7;
	}

	.snippet-tags {
		display: flex;
		flex-wrap: wrap;
		gap: 3px;
	}

	.tag {
		padding: 1px 5px;
		font-size: 0.5625rem;
		color: var(--color-text-secondary);
		background: rgba(255, 255, 255, 0.06);
		border-radius: 3px;
	}

	.empty {
		margin: 0;
		padding: 16px 0;
		text-align: center;
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
		opacity: 0.6;
	}
</style>
