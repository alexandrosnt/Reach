<script lang="ts">
	function autoFocus(node: HTMLElement) { node.focus(); }

	import { t } from '$lib/state/i18n.svelte';
	import { getCurrentPath, getEntries, setCurrentPath, setEntries } from '$lib/state/explorer.svelte';
	import { sftpListDir, sftpUpload, sftpDownload, sftpDelete, sftpRename, sftpMkdir, sftpTouch, sftpReadFile } from '$lib/ipc/sftp';
	import { sshSend } from '$lib/ipc/ssh';
	import { openEditor } from '$lib/state/editor.svelte';
	import { addToast } from '$lib/state/toasts.svelte';
	import FileNode from './FileNode.svelte';
	import type { FileEntry } from '$lib/state/explorer.svelte';
	import { listen, type UnlistenFn } from '@tauri-apps/api/event';
	import { getCurrentWebview } from '@tauri-apps/api/webview';
	import { save } from '@tauri-apps/plugin-dialog';
	import { onMount } from 'svelte';
	import {
		addTransfer,
		updateTransferProgress,
		completeTransfer,
		failTransfer,
		getTransfers,
		removeTransfer,
		type Transfer
	} from '$lib/state/transfers.svelte';

	interface Props {
		connectionId?: string;
	}

	let { connectionId }: Props = $props();

	let loading = $state(false);
	let error = $state<string | undefined>();
	let prevConnectionId = $state<string | undefined>();
	let dragging = $state(false);
	let uploading = $state(false);

	// Context menu state — entry is undefined for background (empty space) clicks
	let contextMenu = $state<{ x: number; y: number; entry?: FileEntry } | undefined>();
	let renaming = $state<{ entry: FileEntry; newName: string } | undefined>();
	let deleteConfirm = $state<{ entry: FileEntry } | undefined>();
	let explorerEl: HTMLDivElement | undefined = $state();

	let currentPath = $derived(connectionId ? getCurrentPath(connectionId) : '/');
	let entries = $derived(connectionId ? getEntries(connectionId) : []);
	let activeTransfers = $derived(getTransfers().filter((t: Transfer) => t.status === 'uploading' || t.status === 'downloading'));

	let pathSegments = $derived.by(() => {
		const parts = currentPath.split('/').filter(Boolean);
		return [
			{ name: '/', path: '/' },
			...parts.map((part: string, i: number) => ({
				name: part,
				path: '/' + parts.slice(0, i + 1).join('/')
			}))
		];
	});

	let fileSearch = $state('');

	let sortedEntries = $derived.by(() => {
		let filtered = entries;
		const q = fileSearch.trim().toLowerCase();
		if (q) {
			filtered = entries.filter((e: FileEntry) => e.name.toLowerCase().includes(q));
		}
		const dirs = filtered
			.filter((e: FileEntry) => e.isDirectory)
			.sort((a: FileEntry, b: FileEntry) => a.name.localeCompare(b.name));
		const files = filtered
			.filter((e: FileEntry) => !e.isDirectory)
			.sort((a: FileEntry, b: FileEntry) => a.name.localeCompare(b.name));
		return [...dirs, ...files];
	});

	function cdToFolder(entry: FileEntry): void {
		if (!connectionId || !entry.isDirectory) return;
		const cmd = `cd ${entry.path}\n`;
		sshSend(connectionId, Array.from(new TextEncoder().encode(cmd)));
		addToast(t('explorer.cd_sent', { path: entry.path }), 'success');
	}

	function openContextMenu(e: MouseEvent, entry: FileEntry): void {
		e.preventDefault();
		e.stopPropagation();
		contextMenu = { x: e.clientX, y: e.clientY, entry };
	}

	function openBackgroundContextMenu(e: MouseEvent): void {
		e.preventDefault();
		contextMenu = { x: e.clientX, y: e.clientY };
	}

	function closeContextMenu(): void {
		contextMenu = undefined;
	}

	async function handleDownload(): Promise<void> {
		if (!connectionId || !contextMenu?.entry) return;
		const entry = contextMenu.entry;
		closeContextMenu();

		const localPath = await save({ defaultPath: entry.name });
		if (!localPath) return;

		let transferId: string | undefined;
		let progressUnlisten: UnlistenFn | undefined;
		let completeUnlisten: UnlistenFn | undefined;
		let errorUnlisten: UnlistenFn | undefined;

		try {
			transferId = await sftpDownload(connectionId, entry.path, localPath);
			addTransfer(transferId, entry.name, entry.size, 'downloading');

			const tid = transferId;

			progressUnlisten = await listen<{
				id: string;
				filename: string;
				bytesTransferred: number;
				totalBytes: number;
				percent: number;
			}>(`transfer-progress-${tid}`, (event) => {
				updateTransferProgress(
					event.payload.id,
					event.payload.bytesTransferred,
					event.payload.totalBytes,
					event.payload.percent
				);
			});

			completeUnlisten = await listen(`transfer-complete-${tid}`, () => {
				completeTransfer(tid);
			});

			errorUnlisten = await listen<string>(`transfer-error-${tid}`, (event) => {
				const errMsg = typeof event.payload === 'string' ? event.payload : 'Download failed';
				failTransfer(tid, errMsg);
			});

			await new Promise<void>((resolve, reject) => {
				const checkInterval = setInterval(() => {
					const transfers = getTransfers();
					const tr = transfers.find((x: Transfer) => x.id === tid);
					if (!tr || tr.status === 'completed') {
						clearInterval(checkInterval);
						resolve();
					} else if (tr.status === 'error') {
						clearInterval(checkInterval);
						reject(new Error(tr.error ?? 'Download failed'));
					}
				}, 200);
			});
		} catch (err) {
			if (transferId) {
				failTransfer(transferId, String(err));
			}
		} finally {
			progressUnlisten?.();
			completeUnlisten?.();
			errorUnlisten?.();
		}
	}

	async function handleEdit(): Promise<void> {
		if (!connectionId || !contextMenu?.entry) return;
		const entry = contextMenu.entry;
		closeContextMenu();

		try {
			const content = await sftpReadFile(connectionId, entry.path);
			await openEditor(connectionId, entry.path, entry.name, content);
		} catch (err) {
			addToast(t('explorer.open_file_error', { error: String(err) }), 'error');
		}
	}

	function handleDelete(): void {
		if (!contextMenu?.entry) return;
		const entry = contextMenu.entry;
		closeContextMenu();
		deleteConfirm = { entry };
	}

	async function confirmDelete(): Promise<void> {
		if (!connectionId || !deleteConfirm) return;
		const { entry } = deleteConfirm;
		deleteConfirm = undefined;
		try {
			await sftpDelete(connectionId, entry.path);
			refresh();
		} catch (err) {
			error = `Delete failed: ${err}`;
		}
	}

	function cancelDelete(): void {
		deleteConfirm = undefined;
	}

	function startRename(): void {
		if (!contextMenu?.entry) return;
		renaming = { entry: contextMenu.entry, newName: contextMenu.entry.name };
		closeContextMenu();
	}

	async function commitRename(): Promise<void> {
		if (!connectionId || !renaming) return;
		const { entry, newName } = renaming;
		const trimmed = newName.trim();
		renaming = undefined;

		if (!trimmed || trimmed === entry.name) return;

		const parent = entry.path.replace(/\/[^/]+\/?$/, '') || '/';
		const newPath = parent === '/' ? `/${trimmed}` : `${parent}/${trimmed}`;

		try {
			await sftpRename(connectionId, entry.path, newPath);
			refresh();
		} catch (err) {
			error = `Rename failed: ${err}`;
		}
	}

	function handleRenameKeydown(e: KeyboardEvent): void {
		if (e.key === 'Enter') {
			e.preventDefault();
			commitRename();
		} else if (e.key === 'Escape') {
			renaming = undefined;
		}
	}

	let previewFile = $state<{ name: string; path: string; content: string; size: number } | undefined>();

	async function handlePreview(): Promise<void> {
		if (!connectionId || !contextMenu?.entry || contextMenu.entry.isDirectory) return;
		const entry = contextMenu.entry;
		closeContextMenu();
		try {
			const content = await sftpReadFile(connectionId, entry.path);
			previewFile = { name: entry.name, path: entry.path, content, size: entry.size };
		} catch (err) {
			addToast(t('explorer.open_file_error', { error: String(err) }), 'error');
		}
	}

	let pathInput = $state('');
	let editingPath = $state(false);

	function startPathEdit(): void {
		pathInput = currentPath;
		editingPath = true;
	}

	function commitPathEdit(): void {
		const p = pathInput.trim();
		editingPath = false;
		if (p && p !== currentPath) {
			loadDirectory(p.startsWith('/') ? p : '/' + p);
		}
	}

	function handlePathKeydown(e: KeyboardEvent): void {
		if (e.key === 'Enter') { e.preventDefault(); commitPathEdit(); }
		else if (e.key === 'Escape') { editingPath = false; }
	}

	async function copyToClipboard(text: string): Promise<void> {
		try {
			await navigator.clipboard.writeText(text);
			addToast(t('explorer.copied'), 'success');
		} catch {
			addToast('Copy failed', 'error');
		}
	}

	function handleCopyPath(): void {
		if (!contextMenu?.entry) return;
		copyToClipboard(contextMenu.entry.path);
		closeContextMenu();
	}

	function handleCopyFilename(): void {
		if (!contextMenu?.entry) return;
		copyToClipboard(contextMenu.entry.name);
		closeContextMenu();
	}

	async function handleQuickDownload(entry: FileEntry): Promise<void> {
		if (!connectionId) return;

		const localPath = await save({ defaultPath: entry.name });
		if (!localPath) return;

		let transferId: string | undefined;
		let progressUnlisten: UnlistenFn | undefined;
		let completeUnlisten: UnlistenFn | undefined;
		let errorUnlisten: UnlistenFn | undefined;

		try {
			transferId = await sftpDownload(connectionId, entry.path, localPath);
			addTransfer(transferId, entry.name, entry.size, 'downloading');

			const tid = transferId;

			progressUnlisten = await listen<{ id: string; filename: string; bytesTransferred: number; totalBytes: number; percent: number; }>(`transfer-progress-${tid}`, (event) => {
				updateTransferProgress(event.payload.id, event.payload.bytesTransferred, event.payload.totalBytes, event.payload.percent);
			});

			completeUnlisten = await listen(`transfer-complete-${tid}`, () => { completeTransfer(tid); });
			errorUnlisten = await listen<string>(`transfer-error-${tid}`, (event) => {
				failTransfer(tid, typeof event.payload === 'string' ? event.payload : 'Download failed');
			});

			await new Promise<void>((resolve, reject) => {
				const checkInterval = setInterval(() => {
					const transfers = getTransfers();
					const tr = transfers.find((x: Transfer) => x.id === tid);
					if (!tr || tr.status === 'completed') { clearInterval(checkInterval); resolve(); }
					else if (tr.status === 'error') { clearInterval(checkInterval); reject(new Error(tr.error ?? 'Download failed')); }
				}, 200);
			});
		} catch (err) {
			if (transferId) failTransfer(transferId, String(err));
		} finally {
			progressUnlisten?.();
			completeUnlisten?.();
			errorUnlisten?.();
		}
	}

	let creatingFolder = $state<string | undefined>();
	let creatingFile = $state<string | undefined>();

	function handleNewFolder(): void {
		closeContextMenu();
		creatingFile = undefined;
		creatingFolder = '';
	}

	function handleNewFile(): void {
		closeContextMenu();
		creatingFolder = undefined;
		creatingFile = '';
	}

	function handleNewFolderKeydown(e: KeyboardEvent): void {
		if (e.key === 'Enter') {
			e.preventDefault();
			commitNewFolder();
		} else if (e.key === 'Escape') {
			creatingFolder = undefined;
		}
	}

	function handleNewFileKeydown(e: KeyboardEvent): void {
		if (e.key === 'Enter') {
			e.preventDefault();
			commitNewFile();
		} else if (e.key === 'Escape') {
			creatingFile = undefined;
		}
	}

	async function commitNewFolder(): Promise<void> {
		if (!connectionId || creatingFolder === undefined) return;
		const name = creatingFolder.trim();
		creatingFolder = undefined;

		if (!name) return;

		const path = currentPath === '/' ? `/${name}` : `${currentPath}/${name}`;

		try {
			await sftpMkdir(connectionId, path);
			refresh();
		} catch (err) {
			error = `Create folder failed: ${err}`;
		}
	}

	async function commitNewFile(): Promise<void> {
		if (!connectionId || creatingFile === undefined) return;
		const name = creatingFile.trim();
		creatingFile = undefined;

		if (!name) return;

		const path = currentPath === '/' ? `/${name}` : `${currentPath}/${name}`;

		try {
			await sftpTouch(connectionId, path);
			refresh();
		} catch (err) {
			error = `Create file failed: ${err}`;
		}
	}

	async function loadDirectory(path: string): Promise<void> {
		if (!connectionId) return;
		loading = true;
		error = undefined;
		fileSearch = '';
		try {
			const result = await sftpListDir(connectionId, path);
			setCurrentPath(connectionId, path);
			setEntries(
				connectionId,
				result.map((e) => ({
					name: e.name,
					path: e.path,
					isDirectory: e.isDirectory,
					size: e.size,
					modified: e.modified,
					permissions: e.permissions
				}))
			);
		} catch (err) {
			error = String(err);
		} finally {
			loading = false;
		}
	}

	function navigateTo(path: string): void {
		loadDirectory(path);
	}

	function navigateUp(): void {
		const parent = currentPath.replace(/\/[^/]+\/?$/, '') || '/';
		loadDirectory(parent);
	}

	function handleNodeClick(entry: FileEntry): void {
		if (entry.isDirectory) {
			loadDirectory(entry.path);
		}
	}

	function refresh(): void {
		loadDirectory(currentPath);
	}

	function extractFilename(filePath: string): string {
		const segments = filePath.replace(/\\/g, '/').split('/');
		return segments[segments.length - 1] || filePath;
	}

	function buildRemotePath(remoteDirPath: string, filename: string): string {
		if (remoteDirPath.endsWith('/')) {
			return remoteDirPath + filename;
		}
		return remoteDirPath + '/' + filename;
	}

	async function uploadFile(localPath: string): Promise<void> {
		if (!connectionId) return;

		const filename = extractFilename(localPath);
		const remotePath = buildRemotePath(currentPath, filename);

		let transferId: string | undefined;
		let progressUnlisten: UnlistenFn | undefined;
		let completeUnlisten: UnlistenFn | undefined;
		let errorUnlisten: UnlistenFn | undefined;

		try {
			transferId = await sftpUpload(connectionId, localPath, remotePath);
			addTransfer(transferId, filename, 0);

			const tid = transferId;

			progressUnlisten = await listen<{
				id: string;
				filename: string;
				bytesTransferred: number;
				totalBytes: number;
				percent: number;
			}>(`transfer-progress-${tid}`, (event) => {
				updateTransferProgress(
					event.payload.id,
					event.payload.bytesTransferred,
					event.payload.totalBytes,
					event.payload.percent
				);
			});

			completeUnlisten = await listen(`transfer-complete-${tid}`, () => {
				completeTransfer(tid);
			});

			errorUnlisten = await listen<string>(`transfer-error-${tid}`, (event) => {
				const errMsg = typeof event.payload === 'string' ? event.payload : 'Upload failed';
				failTransfer(tid, errMsg);
			});

			await new Promise<void>((resolve, reject) => {
				const checkInterval = setInterval(() => {
					const transfers = getTransfers();
					const tr = transfers.find((x: Transfer) => x.id === tid);
					if (!tr || tr.status === 'completed') {
						clearInterval(checkInterval);
						resolve();
					} else if (tr.status === 'error') {
						clearInterval(checkInterval);
						reject(new Error(tr.error ?? 'Upload failed'));
					}
				}, 200);
			});
		} catch (err) {
			if (transferId) {
				failTransfer(transferId, String(err));
			}
			addToast(t('explorer.upload_failed', { error: String(err) }), 'error');
		} finally {
			progressUnlisten?.();
			completeUnlisten?.();
			errorUnlisten?.();
		}
	}

	async function handleDrop(paths: string[]): Promise<void> {
		if (!connectionId) return;

		if (paths.length === 0) {
			addToast(t('explorer.drop_no_files'), 'error');
			return;
		}

		if (uploading) return;
		uploading = true;

		try {
			for (const localPath of paths) {
				await uploadFile(localPath);
			}
		} finally {
			uploading = false;
			refresh();
		}
	}

	// Load initial directory only when connectionId changes.
	// This $effect intentionally sets prevConnectionId as a guard to avoid
	// redundant network calls -- it cannot be replaced with $derived.
	$effect(() => {
		const connId = connectionId;
		if (connId && connId !== prevConnectionId) {
			prevConnectionId = connId;
			const path = getCurrentPath(connId);
			loadDirectory(path);
		}
		if (!connId) {
			prevConnectionId = undefined;
		}
	});

	// Set up Tauri native drag-and-drop listener
	onMount(() => {
		let unlistenDragDrop: (() => void) | undefined;

		getCurrentWebview()
			.onDragDropEvent((event) => {
				if (event.payload.type === 'over') {
					dragging = true;
				} else if (event.payload.type === 'drop') {
					dragging = false;
					handleDrop(event.payload.paths);
				} else if (event.payload.type === 'leave') {
					dragging = false;
				}
			})
			.then((unlisten) => {
				unlistenDragDrop = unlisten;
			});

		return () => {
			unlistenDragDrop?.();
		};
	});

	function formatTransferProgress(transfer: Transfer): string {
		if (transfer.totalBytes > 0) {
			const mbTransferred = (transfer.bytesTransferred / (1024 * 1024)).toFixed(1);
			const mbTotal = (transfer.totalBytes / (1024 * 1024)).toFixed(1);
			return `${mbTransferred} / ${mbTotal} MB`;
		}
		return `${transfer.percent}%`;
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	bind:this={explorerEl}
	class="explorer-root"
	onclick={closeContextMenu}
	onkeydown={() => {}}
>
{#if !connectionId}
	<div class="empty-state">
		<svg width="24" height="24" viewBox="0 0 24 24" fill="none" class="empty-icon">
			<path
				d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"
				stroke="currentColor"
				stroke-width="1.5"
				stroke-linecap="round"
				stroke-linejoin="round"
			/>
		</svg>
		<span class="empty-text">{t('explorer.connect_to_browse')}</span>
	</div>
{:else}
	<div class="explorer">
		<div class="toolbar">
			<button
				class="tool-btn"
				onclick={navigateUp}
				disabled={currentPath === '/'}
				aria-label={t('explorer.go_parent')}
				type="button"
			>
				<svg width="14" height="14" viewBox="0 0 24 24" fill="none">
					<path
						d="M15 18l-6-6 6-6"
						stroke="currentColor"
						stroke-width="1.8"
						stroke-linecap="round"
						stroke-linejoin="round"
					/>
				</svg>
			</button>

			<button class="tool-btn" onclick={refresh} aria-label={t('explorer.refresh_dir')} type="button">
				<svg width="14" height="14" viewBox="0 0 24 24" fill="none">
					<path
						d="M23 4v6h-6M1 20v-6h6"
						stroke="currentColor"
						stroke-width="1.8"
						stroke-linecap="round"
						stroke-linejoin="round"
					/>
					<path
						d="M3.51 9a9 9 0 0114.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0020.49 15"
						stroke="currentColor"
						stroke-width="1.8"
						stroke-linecap="round"
						stroke-linejoin="round"
					/>
				</svg>
			</button>

			<div class="explorer-search">
				<svg width="11" height="11" viewBox="0 0 24 24" fill="none" class="explorer-search-icon">
					<circle cx="11" cy="11" r="8" stroke="currentColor" stroke-width="2"/>
					<path d="M21 21l-4.35-4.35" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
				</svg>
				<input
					class="explorer-search-input"
					type="text"
					placeholder={t('explorer.search_placeholder')}
					bind:value={fileSearch}
				/>
				{#if fileSearch}
					<button class="explorer-search-clear" onclick={() => (fileSearch = '')} type="button" aria-label="Clear search">
						<svg width="8" height="8" viewBox="0 0 10 10" fill="none">
							<path d="M1 1l8 8M9 1L1 9" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>
						</svg>
					</button>
				{/if}
			</div>
		</div>

		<div class="breadcrumb-bar">
			{#if editingPath}
				<input
					class="path-input"
					type="text"
					bind:value={pathInput}
					onkeydown={handlePathKeydown}
					onblur={commitPathEdit}
					use:autoFocus
				/>
			{:else}
				<!-- svelte-ignore a11y_no_static_element_interactions -->
				<div class="breadcrumb-segments" ondblclick={startPathEdit}>
					{#each pathSegments as segment, i (segment.path)}
						{#if i > 0}
							<span class="breadcrumb-sep">/</span>
						{/if}
						<button
							class="breadcrumb-segment"
							class:active={i === pathSegments.length - 1}
							onclick={() => navigateTo(segment.path)}
							type="button"
						>
							{segment.name}
						</button>
					{/each}
				</div>
			{/if}
		</div>

		{#if error}
			<div class="error-message">
				<span class="error-text">{error}</span>
				<button class="retry-btn" onclick={refresh} type="button">{t('explorer.retry')}</button>
			</div>
		{/if}

		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div class="file-list" class:drag-over={dragging} oncontextmenu={openBackgroundContextMenu}>
			{#if dragging}
				<div class="drop-overlay">
					<div class="drop-overlay-content">
						<svg width="24" height="24" viewBox="0 0 24 24" fill="none" class="drop-icon">
							<path
								d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4"
								stroke="currentColor"
								stroke-width="1.5"
								stroke-linecap="round"
								stroke-linejoin="round"
							/>
							<polyline
								points="17 8 12 3 7 8"
								stroke="currentColor"
								stroke-width="1.5"
								stroke-linecap="round"
								stroke-linejoin="round"
							/>
							<line
								x1="12"
								y1="3"
								x2="12"
								y2="15"
								stroke="currentColor"
								stroke-width="1.5"
								stroke-linecap="round"
								stroke-linejoin="round"
							/>
						</svg>
						<span class="drop-text">{t('explorer.drop_to_upload')}</span>
					</div>
				</div>
			{/if}

			{#if activeTransfers.length > 0}
				<div class="transfer-list">
					{#each activeTransfers as transfer (transfer.id)}
						<div class="transfer-item">
							<div class="transfer-info">
								{#if transfer.status === 'downloading'}
									<svg width="12" height="12" viewBox="0 0 24 24" fill="none" class="transfer-icon download">
										<path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
										<polyline points="7 10 12 15 17 10" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
										<line x1="12" y1="15" x2="12" y2="3" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
									</svg>
								{:else}
									<svg width="12" height="12" viewBox="0 0 24 24" fill="none" class="transfer-icon">
										<path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
										<polyline points="17 8 12 3 7 8" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
										<line x1="12" y1="3" x2="12" y2="15" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
									</svg>
								{/if}
								<span class="transfer-filename">{transfer.filename}</span>
								<span class="transfer-progress-text">{formatTransferProgress(transfer)}</span>
							</div>
							<div class="transfer-bar-track">
								<div class="transfer-bar-fill" style="width: {transfer.percent}%"></div>
							</div>
						</div>
					{/each}
				</div>
			{/if}

			{#if loading}
				<div class="loading-overlay">
					<div class="spinner"></div>
				</div>
			{/if}

			{#if deleteConfirm}
				<div class="delete-confirm-bar">
					<span class="delete-confirm-text">{t('explorer.delete_confirm', { name: deleteConfirm.entry.name })}</span>
					<div class="delete-confirm-actions">
						<button class="delete-confirm-btn cancel" onclick={cancelDelete} type="button">{t('common.cancel')}</button>
						<button class="delete-confirm-btn confirm" onclick={confirmDelete} type="button">{t('common.delete')}</button>
					</div>
				</div>
			{/if}

			{#if creatingFolder !== undefined}
				<div class="rename-row">
					<svg width="16" height="16" viewBox="0 0 24 24" fill="none" class="new-folder-icon">
						<path d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
					</svg>
					<input
						class="rename-input"
						type="text"
						placeholder={t('explorer.folder_name')}
						value={creatingFolder}
						oninput={(e) => { creatingFolder = e.currentTarget.value; }}
						onkeydown={handleNewFolderKeydown}
						onblur={commitNewFolder}
						use:autoFocus
					/>
				</div>
			{/if}

			{#if creatingFile !== undefined}
				<div class="rename-row">
					<svg width="16" height="16" viewBox="0 0 24 24" fill="none" class="new-folder-icon">
						<path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
						<polyline points="14 2 14 8 20 8" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
					</svg>
					<input
						class="rename-input"
						type="text"
						placeholder={t('explorer.file_name')}
						value={creatingFile}
						oninput={(e) => { creatingFile = e.currentTarget.value; }}
						onkeydown={handleNewFileKeydown}
						onblur={commitNewFile}
						use:autoFocus
					/>
				</div>
			{/if}

			{#if sortedEntries.length === 0 && !loading && creatingFolder === undefined}
				<div class="empty-dir">
					<span class="empty-dir-text">{t('explorer.empty')}</span>
				</div>
			{:else}
				{#each sortedEntries as entry (entry.path)}
					{#if renaming && renaming.entry.path === entry.path}
						<div class="rename-row">
							<input
								class="rename-input"
								type="text"
								value={renaming.newName}
								oninput={(e) => { if (renaming) renaming.newName = e.currentTarget.value; }}
								onkeydown={handleRenameKeydown}
								onblur={commitRename}
								use:autoFocus
							/>
						</div>
					{:else}
						<FileNode {entry} onclick={() => handleNodeClick(entry)} oncontextmenu={(e) => openContextMenu(e, entry)} ondownload={!entry.isDirectory ? () => handleQuickDownload(entry) : undefined} />
					{/if}
				{/each}
			{/if}
		</div>
	</div>
{/if}

{#if previewFile}
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div class="preview-overlay" onclick={() => (previewFile = undefined)} onkeydown={(e) => { if (e.key === 'Escape') previewFile = undefined; }}>
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div class="preview-box" onclick={(e) => e.stopPropagation()} onkeydown={() => {}}>
			<div class="preview-header">
				<span class="preview-filename">{previewFile.name}</span>
				<span class="preview-path">{previewFile.path}</span>
				<button class="preview-close" onclick={() => (previewFile = undefined)} type="button" aria-label="Close preview">
					<svg width="12" height="12" viewBox="0 0 10 10" fill="none">
						<path d="M1 1l8 8M9 1L1 9" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>
					</svg>
				</button>
			</div>
			<pre class="preview-content">{previewFile.content}</pre>
		</div>
	</div>
{/if}

{#if contextMenu}
	<div
		class="context-menu"
		style="left: {contextMenu.x}px; top: {contextMenu.y}px;"
	>
		{#if contextMenu.entry}
			{#if !contextMenu.entry.isDirectory}
				<button class="context-item" onclick={handlePreview} type="button">
					<svg width="14" height="14" viewBox="0 0 24 24" fill="none">
						<path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
						<circle cx="12" cy="12" r="3" stroke="currentColor" stroke-width="1.5"/>
					</svg>
					{t('explorer.preview')}
				</button>
				<button class="context-item" onclick={handleEdit} type="button">
					<svg width="14" height="14" viewBox="0 0 24 24" fill="none">
						<path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
						<polyline points="14 2 14 8 20 8" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
						<line x1="16" y1="13" x2="8" y2="13" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
						<line x1="16" y1="17" x2="8" y2="17" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
					</svg>
					{t('explorer.edit')}
				</button>
				<button class="context-item" onclick={handleDownload} type="button">
					<svg width="14" height="14" viewBox="0 0 24 24" fill="none">
						<path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
						<polyline points="7 10 12 15 17 10" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
						<line x1="12" y1="15" x2="12" y2="3" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
					</svg>
					{t('explorer.download')}
				</button>
			{/if}
			{#if contextMenu.entry.isDirectory}
				<button class="context-item" onclick={() => { if (contextMenu?.entry) cdToFolder(contextMenu.entry); closeContextMenu(); }} type="button">
					<svg width="14" height="14" viewBox="0 0 24 24" fill="none">
						<polyline points="4 17 10 11 4 5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
						<line x1="12" y1="19" x2="20" y2="19" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
					</svg>
					{t('explorer.cd_here')}
				</button>
			{/if}
			<button class="context-item" onclick={handleCopyPath} type="button">
				<svg width="14" height="14" viewBox="0 0 24 24" fill="none">
					<rect x="9" y="9" width="13" height="13" rx="2" stroke="currentColor" stroke-width="1.5"/>
					<path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1" stroke="currentColor" stroke-width="1.5"/>
				</svg>
				{t('explorer.copy_path')}
			</button>
			<button class="context-item" onclick={handleCopyFilename} type="button">
				<svg width="14" height="14" viewBox="0 0 24 24" fill="none">
					<rect x="9" y="9" width="13" height="13" rx="2" stroke="currentColor" stroke-width="1.5"/>
					<path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1" stroke="currentColor" stroke-width="1.5"/>
				</svg>
				{t('explorer.copy_filename')}
			</button>
			<button class="context-item" onclick={startRename} type="button">
				<svg width="14" height="14" viewBox="0 0 24 24" fill="none">
					<path d="M17 3a2.83 2.83 0 114 4L7.5 20.5 2 22l1.5-5.5L17 3z" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
				</svg>
				{t('explorer.rename')}
			</button>
			<button class="context-item danger" onclick={handleDelete} type="button">
				<svg width="14" height="14" viewBox="0 0 24 24" fill="none">
					<polyline points="3 6 5 6 21 6" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
					<path d="M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
				</svg>
				{t('explorer.delete')}
			</button>
			<div class="context-sep"></div>
		{/if}
		<button class="context-item" onclick={handleNewFile} type="button">
			<svg width="14" height="14" viewBox="0 0 24 24" fill="none">
				<path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
				<polyline points="14 2 14 8 20 8" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
				<line x1="12" y1="11" x2="12" y2="17" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
				<line x1="9" y1="14" x2="15" y2="14" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
			</svg>
			{t('explorer.new_file')}
		</button>
		<button class="context-item" onclick={handleNewFolder} type="button">
			<svg width="14" height="14" viewBox="0 0 24 24" fill="none">
				<path d="M22 19a2 2 0 01-2 2H4a2 2 0 01-2-2V5a2 2 0 012-2h5l2 3h9a2 2 0 012 2z" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
				<line x1="12" y1="11" x2="12" y2="17" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
				<line x1="9" y1="14" x2="15" y2="14" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
			</svg>
			{t('explorer.new_folder')}
		</button>
		<div class="context-sep"></div>
		<button class="context-item" onclick={() => { closeContextMenu(); refresh(); }} type="button">
			<svg width="14" height="14" viewBox="0 0 24 24" fill="none">
				<path d="M23 4v6h-6M1 20v-6h6" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
				<path d="M3.51 9a9 9 0 0114.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0020.49 15" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
			</svg>
			{t('explorer.refresh')}
		</button>
	</div>
{/if}
</div>

<style>
	.empty-state {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 8px;
		padding: 24px 12px;
	}

	.empty-icon {
		color: var(--color-text-secondary);
		opacity: 0.4;
	}

	.empty-text {
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
		opacity: 0.6;
	}

	.explorer {
		display: flex;
		flex-direction: column;
		height: 100%;
		overflow: hidden;
	}

	.toolbar {
		display: flex;
		align-items: center;
		gap: 2px;
		padding: 4px 6px;
		border-bottom: 1px solid var(--color-border);
	}

	.tool-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 26px;
		height: 26px;
		border: none;
		border-radius: var(--radius-btn);
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
		transition: background-color var(--duration-default) var(--ease-default),
			color var(--duration-default) var(--ease-default);
	}

	.tool-btn:hover {
		background-color: rgba(255, 255, 255, 0.06);
		color: var(--color-text-primary);
	}

	.tool-btn:disabled {
		opacity: 0.3;
		cursor: default;
	}

	.tool-btn:disabled:hover {
		background-color: transparent;
		color: var(--color-text-secondary);
	}

	.explorer-search {
		display: flex;
		align-items: center;
		gap: 4px;
		flex: 1;
		min-width: 0;
		padding: 3px 6px;
		background: var(--color-bg-primary);
		border: 1px solid var(--color-border);
		border-radius: 5px;
		margin-left: 4px;
	}

	.explorer-search:focus-within {
		border-color: var(--color-accent);
	}

	.explorer-search-icon {
		flex-shrink: 0;
		color: var(--color-text-secondary);
		opacity: 0.4;
	}

	.explorer-search-input {
		flex: 1;
		min-width: 0;
		padding: 0;
		border: none;
		background: transparent;
		color: var(--color-text-primary);
		font-family: var(--font-sans);
		font-size: 0.625rem;
		outline: none;
	}

	.explorer-search-input::placeholder {
		color: var(--color-text-secondary);
		opacity: 0.5;
	}

	.explorer-search-clear {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 14px;
		height: 14px;
		border: none;
		border-radius: 3px;
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
		flex-shrink: 0;
	}

	.explorer-search-clear:hover {
		background: rgba(255, 255, 255, 0.08);
	}

	.breadcrumb-bar {
		display: flex;
		align-items: center;
		padding: 4px 8px;
		border-bottom: 1px solid var(--color-border);
		overflow-x: auto;
		scrollbar-width: none;
		flex-shrink: 0;
	}

	.breadcrumb-segments {
		display: flex;
		align-items: center;
		flex: 1;
		min-width: 0;
	}

	.path-input {
		width: 100%;
		padding: 2px 4px;
		border: 1px solid var(--color-accent);
		border-radius: 4px;
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		font-family: var(--font-mono, monospace);
		font-size: 0.6875rem;
		outline: none;
	}

	.breadcrumb-bar::-webkit-scrollbar {
		display: none;
	}

	.breadcrumb-sep {
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
		opacity: 0.4;
		padding: 0 1px;
		flex-shrink: 0;
	}

	.breadcrumb-segment {
		border: none;
		background: transparent;
		color: var(--color-text-secondary);
		font-family: var(--font-sans);
		font-size: 0.6875rem;
		cursor: pointer;
		padding: 2px 4px;
		border-radius: var(--radius-btn);
		white-space: nowrap;
		flex-shrink: 0;
		transition: background-color var(--duration-default) var(--ease-default),
			color var(--duration-default) var(--ease-default);
	}

	.breadcrumb-segment:hover {
		background-color: rgba(255, 255, 255, 0.06);
		color: var(--color-text-primary);
	}

	.breadcrumb-segment.active {
		color: var(--color-text-primary);
	}

	.error-message {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 6px 8px;
		background-color: rgba(255, 69, 58, 0.1);
		border-bottom: 1px solid rgba(255, 69, 58, 0.2);
	}

	.error-text {
		font-size: 0.6875rem;
		color: #ff453a;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		flex: 1;
		min-width: 0;
	}

	.retry-btn {
		border: none;
		background: transparent;
		color: #ff453a;
		font-family: var(--font-sans);
		font-size: 0.6875rem;
		font-weight: 500;
		cursor: pointer;
		padding: 2px 6px;
		border-radius: var(--radius-btn);
		flex-shrink: 0;
		transition: background-color var(--duration-default) var(--ease-default);
	}

	.retry-btn:hover {
		background-color: rgba(255, 69, 58, 0.15);
	}

	.file-list {
		flex: 1;
		overflow-y: auto;
		position: relative;
		scrollbar-width: thin;
		scrollbar-color: rgba(255, 255, 255, 0.15) transparent;
		transition: border-color 0.15s ease;
	}

	.file-list::-webkit-scrollbar {
		width: 6px;
	}

	.file-list::-webkit-scrollbar-track {
		background: transparent;
	}

	.file-list::-webkit-scrollbar-thumb {
		background-color: rgba(255, 255, 255, 0.15);
		border-radius: 3px;
	}

	.file-list.drag-over {
		outline: 2px dashed rgba(100, 160, 255, 0.5);
		outline-offset: -2px;
		background-color: rgba(100, 160, 255, 0.04);
	}

	.drop-overlay {
		position: absolute;
		inset: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		background-color: rgba(100, 160, 255, 0.06);
		z-index: 10;
		pointer-events: none;
	}

	.drop-overlay-content {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 8px;
	}

	.drop-icon {
		color: rgba(100, 160, 255, 0.7);
	}

	.drop-text {
		font-size: 0.6875rem;
		font-weight: 500;
		color: rgba(100, 160, 255, 0.8);
	}

	.transfer-list {
		border-bottom: 1px solid var(--color-border);
	}

	.transfer-item {
		padding: 6px 8px;
		border-bottom: 1px solid rgba(255, 255, 255, 0.04);
	}

	.transfer-item:last-child {
		border-bottom: none;
	}

	.transfer-info {
		display: flex;
		align-items: center;
		gap: 6px;
		margin-bottom: 4px;
	}

	.transfer-icon {
		flex-shrink: 0;
		color: var(--color-accent, rgba(100, 160, 255, 0.8));
	}

	.transfer-filename {
		flex: 1;
		font-size: 0.6875rem;
		color: var(--color-text-primary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		min-width: 0;
	}

	.transfer-progress-text {
		flex-shrink: 0;
		font-size: 0.625rem;
		color: var(--color-text-secondary);
		font-variant-numeric: tabular-nums;
	}

	.transfer-bar-track {
		width: 100%;
		height: 2px;
		background-color: rgba(255, 255, 255, 0.06);
		border-radius: 1px;
		overflow: hidden;
	}

	.transfer-bar-fill {
		height: 100%;
		background-color: var(--color-accent, rgba(100, 160, 255, 0.8));
		border-radius: 1px;
		transition: width 0.2s ease;
	}

	.loading-overlay {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 24px;
	}

	.spinner {
		width: 18px;
		height: 18px;
		border: 2px solid var(--color-border);
		border-top-color: var(--color-text-secondary);
		border-radius: 50%;
		animation: spin 0.6s linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}

	.empty-dir {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 24px 12px;
	}

	.empty-dir-text {
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
		opacity: 0.6;
	}

	.explorer-root {
		display: contents;
	}

	.preview-overlay {
		position: fixed;
		inset: 0;
		z-index: 500;
		display: flex;
		align-items: center;
		justify-content: center;
		background: rgba(0, 0, 0, 0.6);
		backdrop-filter: blur(4px);
	}

	.preview-box {
		width: min(90vw, 700px);
		max-height: 80vh;
		display: flex;
		flex-direction: column;
		background: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
		border-radius: 10px;
		box-shadow: 0 16px 48px rgba(0, 0, 0, 0.4);
		overflow: hidden;
	}

	.preview-header {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 10px 14px;
		border-bottom: 1px solid var(--color-border);
		flex-shrink: 0;
	}

	.preview-filename {
		font-size: 0.8125rem;
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.preview-path {
		flex: 1;
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
		opacity: 0.6;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.preview-close {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 24px;
		height: 24px;
		border: none;
		border-radius: 6px;
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
		flex-shrink: 0;
	}

	.preview-close:hover {
		background: rgba(255, 255, 255, 0.08);
		color: var(--color-text-primary);
	}

	.preview-content {
		flex: 1;
		overflow: auto;
		padding: 12px 16px;
		margin: 0;
		font-family: var(--font-mono, monospace);
		font-size: 0.75rem;
		line-height: 1.5;
		color: var(--color-text-primary);
		white-space: pre-wrap;
		word-break: break-all;
		tab-size: 4;
	}

	.context-menu {
		position: fixed;
		min-width: 160px;
		padding: 4px 0;
		background-color: var(--color-bg-elevated, #1c1c1e);
		border: 1px solid var(--color-border);
		border-radius: 8px;
		box-shadow: 0 8px 32px rgba(0, 0, 0, 0.35);
		z-index: 1000;
	}

	.context-item {
		display: flex;
		align-items: center;
		gap: 8px;
		width: 100%;
		padding: 6px 12px;
		border: none;
		background: transparent;
		color: var(--color-text-primary);
		font-family: var(--font-sans);
		font-size: 0.75rem;
		cursor: pointer;
		text-align: left;
		transition: background-color 0.1s ease;
	}

	.context-item:hover {
		background-color: rgba(255, 255, 255, 0.08);
	}

	.context-item.danger {
		color: var(--color-danger, #ff453a);
	}

	.context-item.danger:hover {
		background-color: rgba(255, 69, 58, 0.12);
	}

	.context-sep {
		height: 1px;
		margin: 4px 0;
		background-color: var(--color-border);
	}

	.rename-row {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 4px 8px;
		border-bottom: 1px solid var(--color-border);
	}

	.new-folder-icon {
		flex-shrink: 0;
		color: var(--color-text-secondary);
	}

	.transfer-icon.download {
		color: var(--color-success, #30d158);
	}

	.rename-input {
		width: 100%;
		padding: 4px 6px;
		border: 1px solid var(--color-accent, #0a84ff);
		border-radius: 4px;
		background-color: var(--color-bg-primary);
		color: var(--color-text-primary);
		font-family: var(--font-sans);
		font-size: 0.75rem;
		outline: none;
	}

	.delete-confirm-bar {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
		padding: 8px 10px;
		background-color: rgba(255, 69, 58, 0.08);
		border-bottom: 1px solid rgba(255, 69, 58, 0.2);
	}

	.delete-confirm-text {
		font-size: 0.6875rem;
		color: var(--color-text-primary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		min-width: 0;
	}

	.delete-confirm-actions {
		display: flex;
		gap: 4px;
		flex-shrink: 0;
	}

	.delete-confirm-btn {
		padding: 3px 10px;
		font-family: var(--font-sans);
		font-size: 0.6875rem;
		font-weight: 500;
		border-radius: 4px;
		cursor: pointer;
		border: none;
		transition: background-color var(--duration-default) var(--ease-default);
	}

	.delete-confirm-btn.cancel {
		background: transparent;
		color: var(--color-text-secondary);
		border: 1px solid var(--color-border);
	}

	.delete-confirm-btn.cancel:hover {
		background-color: rgba(255, 255, 255, 0.06);
		color: var(--color-text-primary);
	}

	.delete-confirm-btn.confirm {
		background-color: var(--color-danger, #ff453a);
		color: #fff;
	}

	.delete-confirm-btn.confirm:hover {
		background-color: #ff6961;
	}
</style>
