<script lang="ts">
	import CodeEditor from '$lib/components/editor/CodeEditor.svelte';
	import { sftpWriteFile } from '$lib/ipc/sftp';
	import { addToast } from '$lib/state/toasts.svelte';
	import { t } from '$lib/state/i18n.svelte';
	import { listen, type UnlistenFn } from '@tauri-apps/api/event';
	import { getCurrentWindow } from '@tauri-apps/api/window';
	import { onMount } from 'svelte';

	interface EditorTab {
		id: string;
		connectionId: string;
		path: string;
		filename: string;
		content: string;
		originalContent: string;
		language: string;
	}

	const availableLanguages = [
		'text', 'javascript', 'typescript', 'python', 'rust', 'go', 'java', 'c', 'cpp',
		'ruby', 'php', 'swift', 'kotlin', 'shell', 'css', 'scss', 'less', 'html', 'xml',
		'json', 'yaml', 'toml', 'markdown', 'sql', 'lua', 'r', 'dockerfile'
	];

	let tabs = $state<EditorTab[]>([]);
	let activeTabId = $state<string | null>(null);
	let saving = $state(false);

	let activeTab = $derived(tabs.find((t) => t.id === activeTabId));
	let dirty = $derived(activeTab ? activeTab.content !== activeTab.originalContent : false);

	function detectLanguage(filename: string): string {
		const ext = filename.includes('.') ? filename.slice(filename.lastIndexOf('.')).toLowerCase() : '';
		const map: Record<string, string> = {
			'.js': 'javascript', '.jsx': 'javascript', '.mjs': 'javascript', '.cjs': 'javascript',
			'.ts': 'typescript', '.tsx': 'typescript', '.py': 'python', '.rs': 'rust',
			'.html': 'html', '.htm': 'html', '.css': 'css', '.scss': 'scss', '.less': 'less',
			'.json': 'json', '.md': 'markdown', '.markdown': 'markdown', '.yaml': 'yaml', '.yml': 'yaml',
			'.sh': 'shell', '.bash': 'shell', '.zsh': 'shell', '.c': 'c', '.h': 'c',
			'.cpp': 'cpp', '.hpp': 'cpp', '.cc': 'cpp', '.cxx': 'cpp', '.java': 'java', '.php': 'php',
			'.sql': 'sql', '.xml': 'xml', '.svg': 'xml', '.go': 'go', '.rb': 'ruby',
			'.conf': 'shell', '.ini': 'shell', '.toml': 'toml', '.lua': 'lua', '.r': 'r',
			'.swift': 'swift', '.kt': 'kotlin', '.kts': 'kotlin', '.dart': 'dart',
			'.vue': 'vue', '.svelte': 'html', '.dockerfile': 'dockerfile', '.tf': 'hcl',
		};
		const basename = filename.toLowerCase();
		if (basename === 'dockerfile') return 'dockerfile';
		if (basename === 'makefile') return 'shell';
		if (basename === '.env' || basename.startsWith('.env.')) return 'shell';
		return map[ext] || 'text';
	}

	function makeTabId(connectionId: string, path: string): string {
		return `${connectionId}:${path}`;
	}

	function addOrActivateTab(connectionId: string, path: string, filename: string, content: string): void {
		const id = makeTabId(connectionId, path);
		const existing = tabs.find((t) => t.id === id);
		if (existing) {
			activeTabId = id;
			return;
		}
		tabs.push({
			id,
			connectionId,
			path,
			filename,
			content,
			originalContent: content,
			language: detectLanguage(filename)
		});
		activeTabId = id;
	}

	function closeTab(id: string): void {
		const tab = tabs.find((t) => t.id === id);
		if (tab && tab.content !== tab.originalContent) {
			const confirmed = window.confirm(t('editor.unsaved_changes'));
			if (!confirmed) return;
		}
		const idx = tabs.findIndex((t) => t.id === id);
		if (idx === -1) return;
		tabs.splice(idx, 1);

		if (activeTabId === id) {
			if (tabs.length > 0) {
				activeTabId = tabs[Math.min(idx, tabs.length - 1)].id;
			} else {
				activeTabId = null;
			}
		}

		if (tabs.length === 0) {
			getCurrentWindow().close();
		}
	}

	function updateContent(content: string): void {
		if (activeTab) {
			activeTab.content = content;
		}
	}

	function updateLanguage(language: string): void {
		if (activeTab) {
			activeTab.language = language;
		}
	}

	async function handleSave(): Promise<void> {
		if (!activeTab) return;
		saving = true;
		try {
			await sftpWriteFile(activeTab.connectionId, activeTab.path, activeTab.content);
			activeTab.originalContent = activeTab.content;
			addToast(t('editor.file_saved_toast'), 'success');
		} catch (err: unknown) {
			const message = err instanceof Error ? err.message : String(err);
			addToast(message, 'error');
		} finally {
			saving = false;
		}
	}

	let maximized = $state(false);

	async function checkMaximized(): Promise<void> {
		maximized = await getCurrentWindow().isMaximized();
	}

	function handleMinimize(): void {
		getCurrentWindow().minimize();
	}

	async function handleMaximize(): Promise<void> {
		await getCurrentWindow().toggleMaximize();
		maximized = await getCurrentWindow().isMaximized();
	}

	function handleCloseWindow(): void {
		const hasUnsaved = tabs.some((t) => t.content !== t.originalContent);
		if (hasUnsaved) {
			const confirmed = window.confirm(t('editor.unsaved_changes'));
			if (!confirmed) return;
		}
		getCurrentWindow().close();
	}

	let unlisten: UnlistenFn | undefined;

	let unlistenResize: UnlistenFn | undefined;

	onMount(() => {
		listen<{ connectionId: string; path: string; filename: string; content: string }>(
			'editor-open-file',
			(event) => {
				addOrActivateTab(
					event.payload.connectionId,
					event.payload.path,
					event.payload.filename,
					event.payload.content
				);
			}
		).then((fn) => {
			unlisten = fn;
		});

		getCurrentWindow().onResized(() => {
			checkMaximized();
		}).then((fn) => {
			unlistenResize = fn;
		});

		checkMaximized();

		return () => {
			unlisten?.();
			unlistenResize?.();
		};
	});
</script>

<div class="editor-window">
	<div class="titlebar" data-tauri-drag-region>
		<div class="titlebar-tabs" data-tauri-drag-region>
			{#each tabs as tab (tab.id)}
				<div
					class="editor-tab"
					class:active={tab.id === activeTabId}
					onclick={() => (activeTabId = tab.id)}
					onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') activeTabId = tab.id; }}
					role="tab"
					tabindex="0"
					aria-selected={tab.id === activeTabId}
				>
					<span class="tab-name">{tab.filename}</span>
					{#if tab.content !== tab.originalContent}
						<span class="dirty-dot"></span>
					{/if}
					<button
						class="tab-close-btn"
						onclick={(e) => { e.stopPropagation(); closeTab(tab.id); }}
						aria-label={t('common.close')}
					>
						<svg width="8" height="8" viewBox="0 0 8 8" fill="none">
							<path d="M1 1l6 6M7 1L1 7" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" />
						</svg>
					</button>
				</div>
			{/each}
		</div>

		<div class="titlebar-actions">
			<button class="window-btn" onclick={handleMinimize} aria-label={t('titlebar.minimize')}>
				<svg width="10" height="10" viewBox="0 0 10 10" fill="none">
					<path d="M1 5h8" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" />
				</svg>
			</button>
			<button class="window-btn" onclick={handleMaximize} aria-label={t('titlebar.maximize')}>
				{#if maximized}
					<svg width="10" height="10" viewBox="0 0 10 10" fill="none">
						<rect x="0.5" y="2.5" width="7" height="7" rx="1" stroke="currentColor" stroke-width="1.2" fill="none" />
						<path d="M2.5 2.5V1.5a1 1 0 011-1h5a1 1 0 011 1v5a1 1 0 01-1 1H7.5" stroke="currentColor" stroke-width="1.2" fill="none" />
					</svg>
				{:else}
					<svg width="10" height="10" viewBox="0 0 10 10" fill="none">
						<rect x="1" y="1" width="8" height="8" rx="1" stroke="currentColor" stroke-width="1.3" fill="none" />
					</svg>
				{/if}
			</button>
			<button class="window-btn close-btn" onclick={handleCloseWindow} aria-label={t('titlebar.close')}>
				<svg width="10" height="10" viewBox="0 0 10 10" fill="none">
					<path d="M1 1l8 8M9 1L1 9" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" />
				</svg>
			</button>
		</div>
	</div>

	{#if activeTab}
		<div class="toolbar">
			<span class="toolbar-path">{activeTab.path}</span>
			<div class="toolbar-spacer"></div>
			<select
				value={activeTab.language}
				onchange={(e) => updateLanguage(e.currentTarget.value)}
			>
				{#each availableLanguages as lang (lang)}
					<option value={lang}>{lang}</option>
				{/each}
			</select>
			<button class="save-btn" onclick={handleSave} disabled={saving || !dirty}>
				{saving ? t('editor.saving') : t('editor.save')}
			</button>
			<span class="shortcut-hint">Ctrl+S</span>
		</div>

		{#key activeTab.id}
			<div class="editor-content">
				<CodeEditor
					content={activeTab.content}
					language={activeTab.language}
					onchange={updateContent}
					onsave={handleSave}
				/>
			</div>
		{/key}
	{:else}
		<div class="empty-state">
			<p>{t('common.loading')}</p>
		</div>
	{/if}
</div>

<style>
	.editor-window {
		display: flex;
		flex-direction: column;
		width: 100vw;
		height: 100vh;
		overflow: hidden;
		background: var(--color-bg-primary, #0a0a0a);
	}

	.titlebar {
		display: flex;
		align-items: center;
		height: 38px;
		min-height: 38px;
		background: var(--color-bg-primary, #0a0a0a);
		border-bottom: 1px solid var(--color-border);
		user-select: none;
		-webkit-app-region: drag;
	}

	.titlebar-tabs {
		display: flex;
		flex: 1;
		overflow-x: auto;
		overflow-y: hidden;
		scrollbar-width: none;
		-webkit-app-region: drag;
	}

	.titlebar-tabs::-webkit-scrollbar {
		display: none;
	}

	.editor-tab {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 0 8px 0 12px;
		height: 100%;
		border: none;
		border-bottom: 2px solid transparent;
		background: transparent;
		color: var(--color-text-secondary);
		font-family: var(--font-sans);
		font-size: 0.75rem;
		cursor: pointer;
		white-space: nowrap;
		flex-shrink: 0;
		-webkit-app-region: no-drag;
		transition: background-color 150ms ease, color 150ms ease, border-color 150ms ease;
	}

	.editor-tab:hover {
		background-color: var(--color-bg-secondary);
		color: var(--color-text-primary);
	}

	.editor-tab.active {
		background-color: var(--color-bg-secondary);
		color: var(--color-text-primary);
		border-bottom-color: var(--color-accent);
	}

	.tab-name {
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.dirty-dot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: var(--color-warning, #ffd60a);
		flex-shrink: 0;
	}

	.tab-close-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 16px;
		height: 16px;
		border: none;
		border-radius: 4px;
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
		flex-shrink: 0;
		opacity: 0;
		transition: opacity 150ms ease, background-color 150ms ease;
	}

	.editor-tab:hover .tab-close-btn {
		opacity: 1;
	}

	.tab-close-btn:hover {
		background-color: rgba(255, 255, 255, 0.1);
		color: var(--color-text-primary);
	}

	.titlebar-actions {
		display: flex;
		align-items: center;
		padding: 0 8px;
		-webkit-app-region: no-drag;
	}

	.window-btn {
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
	}

	.window-btn:hover {
		background: rgba(255, 255, 255, 0.06);
		color: var(--color-text-primary);
	}

	.close-btn:hover {
		background: rgba(232, 17, 35, 0.9);
		color: white;
	}

	.toolbar {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 6px 12px;
		border-bottom: 1px solid var(--color-border);
		flex-shrink: 0;
	}

	.toolbar-path {
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
		opacity: 0.7;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.toolbar-spacer {
		flex: 1;
	}

	select {
		background: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
		color: var(--color-text-primary);
		border-radius: var(--radius-btn);
		padding: 4px 8px;
		font-size: 0.75rem;
		font-family: var(--font-sans);
	}

	.save-btn {
		background: var(--color-accent);
		color: white;
		border: none;
		border-radius: var(--radius-btn);
		padding: 5px 12px;
		font-size: 0.75rem;
		font-weight: 500;
		cursor: pointer;
	}

	.save-btn:disabled {
		opacity: 0.4;
		cursor: default;
	}

	.shortcut-hint {
		font-size: 0.625rem;
		color: var(--color-text-secondary);
		opacity: 0.6;
	}

	.editor-content {
		flex: 1;
		overflow: hidden;
		min-height: 0;
	}

	.empty-state {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: center;
		color: var(--color-text-secondary);
	}
</style>
