<script lang="ts">
	import { getSnippets, loadSnippets, addSnippet, removeSnippet, editSnippet, pasteSnippet } from '$lib/state/snippets.svelte';
	import { addToast } from '$lib/state/toasts.svelte';
	import { t } from '$lib/state/i18n.svelte';
	import { onMount } from 'svelte';

	interface Props {
		connectionId?: string;
	}

	let { connectionId }: Props = $props();

	let snippets = $derived(getSnippets());

	let search = $state('');
	let showAdd = $state(false);
	let newName = $state('');
	let newCommand = $state('');
	let newDescription = $state('');
	let newTags = $state('');
	let editingId = $state<string | null>(null);
	/** The snippet asked about before it is deleted, as a session is. */
	let deleteConfirm = $state<string | null>(null);

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
		if (!pasteSnippet(command)) return;
		addToast(t('snippets.pasted', { command: command.length > 40 ? command.slice(0, 40) + '…' : command }), 'success');
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
			deleteConfirm = null;
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
		<!-- The same search field and + button as the session list's. -->
		<label class="search-row">
			<svg class="search-icon" width="12" height="12" viewBox="0 0 24 24" fill="none" aria-hidden="true">
				<circle cx="11" cy="11" r="7" stroke="currentColor" stroke-width="2" />
				<path d="M20 20l-3.5-3.5" stroke="currentColor" stroke-width="2" stroke-linecap="round" />
			</svg>
			<input class="search-input" type="text" placeholder={t('snippets.search')} aria-label={t('snippets.search')} bind:value={search} />
		</label>
		<button class="add-btn" onclick={() => (showAdd = true)} title={t('snippets.add')} aria-label={t('snippets.add')}>
			<svg width="13" height="13" viewBox="0 0 24 24" fill="none" aria-hidden="true">
				<path d="M12 5v14M5 12h14" stroke="currentColor" stroke-width="2" stroke-linecap="round" />
			</svg>
		</button>
	</div>

	{#if showAdd || editingId}
		<form class="snippet-form" onsubmit={(e) => { e.preventDefault(); editingId ? handleEdit() : handleAdd(); }}>
			<input class="form-input" type="text" placeholder={t('snippets.name')} aria-label={t('snippets.name')} bind:value={newName} />
			<textarea class="form-input form-textarea" placeholder={t('snippets.command')} aria-label={t('snippets.command')} bind:value={newCommand} rows="2"></textarea>
			<input class="form-input" type="text" placeholder={t('snippets.description')} aria-label={t('snippets.description')} bind:value={newDescription} />
			<input class="form-input" type="text" placeholder={t('snippets.tags')} aria-label={t('snippets.tags')} bind:value={newTags} />
			<div class="form-actions">
				<button class="form-cancel" type="button" onclick={cancelEdit}>{t('common.cancel')}</button>
				<button class="form-save" type="submit" disabled={!newName.trim() || !newCommand.trim()}>
					{editingId ? t('snippets.update') : t('common.save')}
				</button>
			</div>
		</form>
	{/if}

	<div class="snippet-list">
		{#each filtered as snippet (snippet.id)}
			{#if deleteConfirm === snippet.id}
				<div class="delete-confirm">
					<span class="delete-confirm-text">{t('snippets.delete_confirm', { name: snippet.name })}</span>
					<button class="delete-confirm-btn" onclick={() => handleDelete(snippet.id)}>{t('common.confirm')}</button>
					<button class="delete-cancel-btn" onclick={() => (deleteConfirm = null)}>{t('common.cancel')}</button>
				</div>
			{:else}
				<div class="snippet-card">
					<div class="snippet-header">
						<span class="snippet-name">{snippet.name}</span>
						<div class="snippet-actions">
							<button class="action-btn" onclick={() => pasteToTerminal(snippet.command)} title={t('snippets.paste')} aria-label={t('snippets.paste')}>
								<svg width="11" height="11" viewBox="0 0 24 24" fill="none">
									<rect x="9" y="9" width="13" height="13" rx="2" stroke="currentColor" stroke-width="1.5"/>
									<path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1" stroke="currentColor" stroke-width="1.5"/>
								</svg>
							</button>
							<button class="action-btn run-btn" onclick={() => executeSnippet(snippet.command)} title={t('snippets.run')} aria-label={t('snippets.run')}>
								<svg width="11" height="11" viewBox="0 0 24 24" fill="currentColor">
									<path d="M8 5v14l11-7z"/>
								</svg>
							</button>
							<button class="action-btn" onclick={() => startEdit(snippet)} title={t('common.edit')} aria-label={t('common.edit')}>
								<svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
									<path d="M17 3a2.85 2.85 0 014 4L7.5 20.5 2 22l1.5-5.5Z"/>
								</svg>
							</button>
							<button class="action-btn delete-btn" onclick={() => (deleteConfirm = snippet.id)} title={t('common.delete')} aria-label={t('common.delete')}>
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
			{/if}
		{:else}
			<p class="empty">{search ? t('snippets.no_matches') : t('snippets.empty')}</p>
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
		align-items: stretch;
	}

	.search-row {
		flex: 1;
		min-width: 0;
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 4px 8px;
		background: var(--color-bg-primary);
		border: 1px solid var(--color-border);
		border-radius: 6px;
		cursor: text;
	}

	.search-row:focus-within {
		border-color: var(--color-accent);
	}

	.search-icon {
		flex-shrink: 0;
		color: var(--color-text-secondary);
		opacity: 0.5;
	}

	.search-input {
		flex: 1;
		min-width: 0;
		padding: 3px 0;
		border: none;
		background: transparent;
		color: var(--color-text-primary);
		font-family: var(--font-sans);
		font-size: var(--text-sm);
		outline: none;
	}

	.search-input::placeholder {
		color: var(--color-text-secondary);
		opacity: 0.5;
	}

	.add-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
		width: 30px;
		min-height: 30px;
		padding: 0;
		border: 1px solid var(--color-border);
		border-radius: 6px;
		background: transparent;
		color: var(--color-accent);
		cursor: pointer;
		transition:
			background-color var(--duration-default) var(--ease-default),
			border-color var(--duration-default) var(--ease-default);
	}

	.add-btn:hover {
		background-color: color-mix(in srgb, var(--color-accent) 10%, transparent);
		border-color: color-mix(in srgb, var(--color-accent) 55%, var(--color-border));
	}

	.snippet-form {
		display: flex;
		flex-direction: column;
		gap: 6px;
		padding: 8px;
		background: var(--color-surface-hover);
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

	.snippet-card:hover { background: var(--color-surface-hover); }

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

	.action-btn:hover { background: var(--color-surface-active); color: var(--color-text-primary); }
	.run-btn:hover { color: var(--color-accent); }
	.delete-btn:hover { color: var(--color-danger); }

	.snippet-command {
		margin: 0;
		padding: 4px 6px;
		font-family: var(--font-mono, monospace);
		font-size: 0.625rem;
		color: var(--color-text-secondary);
		background: var(--color-surface-hover);
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
		background: var(--color-surface-hover);
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

	.delete-confirm {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 6px 8px;
		background-color: rgba(255, 69, 58, 0.06);
		border: 1px solid rgba(255, 69, 58, 0.15);
		border-radius: var(--radius-card, 8px);
	}

	.delete-confirm-text {
		flex: 1;
		min-width: 0;
		font-size: var(--text-sm);
		color: var(--color-danger);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.delete-confirm-btn,
	.delete-cancel-btn {
		flex-shrink: 0;
		padding: 3px 8px;
		font-family: var(--font-sans);
		font-size: var(--text-sm);
		font-weight: 500;
		border: none;
		border-radius: 4px;
		cursor: pointer;
	}

	.delete-confirm-btn {
		background-color: var(--color-danger);
		color: #fff;
	}

	.delete-cancel-btn {
		background-color: var(--color-surface-hover);
		color: var(--color-text-secondary);
	}
</style>
