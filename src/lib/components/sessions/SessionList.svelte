<script lang="ts">
	import QuickConnect from './QuickConnect.svelte';
	import SessionEditor from './SessionEditor.svelte';
	import SshConfigImport from './SshConfigImport.svelte';
	import SessionCard from './SessionCard.svelte';
	import VaultSelector from '$lib/components/vault/VaultSelector.svelte';
	import { sessionList, sessionDelete, sessionUpdate, sessionListFolders, sessionCreateFolder, sessionDeleteFolder, type SessionConfig, type Folder } from '$lib/ipc/sessions';
	import { sshConnect, sshDisconnect, sshDetectOs, type JumpHostConnectParams } from '$lib/ipc/ssh';
	// Passwords are now stored encrypted in vault, not in memory cache
	import { createTab, updateTabOs } from '$lib/state/tabs.svelte';
	import { addToast } from '$lib/state/toasts.svelte';
	import { t } from '$lib/state/i18n.svelte';
	import { untrack } from 'svelte';
	import { vaultState, checkState, initIdentity, refreshVaults, importIdentity } from '$lib/state/vault.svelte';

	let showQuickConnect = $state(false);
	let showEditor = $state(false);
	let showImport = $state(false);
	let editingSession = $state<SessionConfig | undefined>();
	let sessions = $state<SessionConfig[]>([]);
	let loading = $state(true);
	let deleteConfirm = $state<string | null>(null);

	// Selected vault filter (null = private/default vault)
	let selectedVaultId = $state<string | null>(null);

	// Search query
	let searchQuery = $state('');

	// Folders
	let folders = $state<Folder[]>([]);
	let collapsedFolders = $state<Set<string>>(new Set(JSON.parse(localStorage.getItem('collapsedFolders') ?? '[]')));
	let creatingFolder = $state(false);
	let newFolderName = $state('');

	// Filter sessions by selected vault, then by search query
	let filteredSessions = $derived.by(() => {
		let result = sessions.filter(s => {
			if (selectedVaultId === null) return !s.vault_id;
			return s.vault_id === selectedVaultId;
		});
		const q = searchQuery.trim().toLowerCase();
		if (q) {
			result = result.filter(s =>
				s.name.toLowerCase().includes(q) ||
				s.host.toLowerCase().includes(q) ||
				s.username.toLowerCase().includes(q) ||
				s.tags.some(tag => tag.toLowerCase().includes(q))
			);
		}
		return result;
	});


	// Group sessions by folder
	let groupedSessions = $derived.by(() => {
		const groups: { folder: Folder | null; sessions: SessionConfig[] }[] = [];
		const folderIds = new Set(folders.map(f => f.id));
		const folderMap = new Map<string, SessionConfig[]>();
		const ungrouped: SessionConfig[] = [];

		for (const s of filteredSessions) {
			// Treat sessions with deleted/orphaned folder_id as ungrouped
			if (s.folder_id && folderIds.has(s.folder_id)) {
				const arr = folderMap.get(s.folder_id) ?? [];
				arr.push(s);
				folderMap.set(s.folder_id, arr);
			} else {
				ungrouped.push(s);
			}
		}

		for (const folder of folders) {
			// Only show folders that belong to the current vault (or have no vault_id for legacy folders)
			if (folder.vault_id !== undefined && folder.vault_id !== null && folder.vault_id !== (selectedVaultId ?? null)) {
				// This folder belongs to a different vault — skip unless it has sessions here
				const folderSessions = folderMap.get(folder.id) ?? [];
				if (folderSessions.length > 0) {
					groups.push({ folder, sessions: folderSessions });
				}
				continue;
			}
			const folderSessions = folderMap.get(folder.id) ?? [];
			groups.push({ folder, sessions: folderSessions });
		}

		if (ungrouped.length > 0 || groups.length === 0) {
			groups.push({ folder: null, sessions: ungrouped });
		}

		return groups;
	});

	function toggleFolder(folderId: string): void {
		const next = new Set(collapsedFolders);
		if (next.has(folderId)) next.delete(folderId);
		else next.add(folderId);
		collapsedFolders = next;
		localStorage.setItem('collapsedFolders', JSON.stringify([...next]));
	}

	async function handleCreateFolder(): Promise<void> {
		const name = newFolderName.trim();
		if (!name) return;
		try {
			await sessionCreateFolder(name, null, selectedVaultId);
			newFolderName = '';
			creatingFolder = false;
			folders = await sessionListFolders();
		} catch (err) {
			addToast(String(err), 'error');
		}
	}

	let deleteFolderConfirm = $state<string | null>(null);

	function handleDeleteFolder(folderId: string): void {
		if (deleteFolderConfirm !== folderId) {
			deleteFolderConfirm = folderId;
			setTimeout(() => { deleteFolderConfirm = null; }, 4000);
			return;
		}
		confirmDeleteFolder(folderId);
	}

	async function confirmDeleteFolder(folderId: string): Promise<void> {
		deleteFolderConfirm = null;
		try {
			const affected = sessions.filter(s => s.folder_id === folderId);
			for (const s of affected) {
				await sessionUpdate({ ...s, folder_id: null });
			}
			await sessionDeleteFolder(folderId);
			folders = await sessionListFolders();
			await loadSessions();
		} catch (err) {
			addToast(String(err), 'error');
		}
	}

	// Drag & drop via pointer events (HTML5 DnD doesn't work in Tauri WebView2 on Windows)
	let dragSession = $state<SessionConfig | undefined>();
	let dropTarget = $state<string | null | undefined>();
	let dragging = $state(false);

	function handleDragStart(e: PointerEvent, session: SessionConfig): void {
		dragSession = session;
		dragging = true;
		const onMove = (me: PointerEvent) => {
			if (!dragSession) return;
			// Find drop target by checking which folder header we're over
			const el = document.elementFromPoint(me.clientX, me.clientY);
			if (el) {
				const folderEl = el.closest('[data-folder-id]') as HTMLElement | null;
				if (folderEl) {
					dropTarget = folderEl.dataset.folderId === '__ungrouped__' ? null : folderEl.dataset.folderId!;
				} else {
					dropTarget = undefined;
				}
			}
		};
		const onUp = async () => {
			window.removeEventListener('pointermove', onMove);
			window.removeEventListener('pointerup', onUp);
			const session = dragSession;
			const target = dropTarget;
			dragSession = undefined;
			dropTarget = undefined;
			dragging = false;
			if (session && target !== undefined && session.folder_id !== target) {
				try {
					await sessionUpdate({ ...session, folder_id: target });
					await loadSessions();
				} catch (err) {
					addToast(String(err), 'error');
				}
			}
		};
		window.addEventListener('pointermove', onMove);
		window.addEventListener('pointerup', onUp);
	}

	// Right-click context menu
	let contextMenu = $state<{ x: number; y: number; session?: SessionConfig } | undefined>();

	function openSessionContextMenu(e: MouseEvent, session: SessionConfig): void {
		e.preventDefault();
		e.stopPropagation();
		contextMenu = { x: e.clientX, y: e.clientY, session };
	}

	function openBackgroundContextMenu(e: MouseEvent): void {
		e.preventDefault();
		contextMenu = { x: e.clientX, y: e.clientY };
	}

	function closeContextMenu(): void {
		contextMenu = undefined;
	}

	async function moveToFolder(session: SessionConfig, folderId: string | null): Promise<void> {
		closeContextMenu();
		try {
			await sessionUpdate({ ...session, folder_id: folderId });
			await loadSessions();
		} catch (err) {
			addToast(String(err), 'error');
		}
	}

	function contextNewFolder(): void {
		closeContextMenu();
		creatingFolder = true;
	}

	// Vault state (TLS-style: auto-unlock, no password needed)
	let locked = $derived(vaultState.locked);
	let hasIdentity = $derived(vaultState.hasIdentity);
	let keychainError = $derived(vaultState.keychainError);
	let initializing = $state(false);
	let initError = $state('');
	let importKey = $state('');
	let importing = $state(false);

	// Connect prompt state
	let connectSession = $state<SessionConfig | undefined>();
	let connectPassword = $state('');
	let connectKeyPassphrase = $state('');
	let connecting = $state(false);
	let connectingId = $state<string | undefined>();
	let connectError = $state<string | undefined>();
	let rememberPassword = $state(false);
	let hasSavedPassword = $state(false);

	let showConnectPrompt = $derived(!!connectSession);

	async function loadSessions(): Promise<void> {
		try {
			[sessions, folders] = await Promise.all([sessionList(), sessionListFolders()]);
		} catch (err) {
			console.error('Failed to load sessions:', err);
		} finally {
			loading = false;
		}
	}

	async function handleConnect(session: SessionConfig): Promise<void> {
		// Check if credentials are stored in the session (from vault)
		const storedPassword = session.auth_method.type === 'Password' ? session.auth_method.password : undefined;
		const storedPassphrase = session.auth_method.type === 'Key' ? session.auth_method.passphrase : undefined;

		// Auto-connect if we have stored credentials OR Agent auth
		if (storedPassword || storedPassphrase || session.auth_method.type === 'Agent') {
			connectSession = session;
			connectError = undefined;
			rememberPassword = true;
			hasSavedPassword = true;

			if (session.auth_method.type === 'Password') {
				connectPassword = storedPassword ?? '';
			} else if (session.auth_method.type === 'Key') {
				connectKeyPassphrase = storedPassphrase ?? '';
			}

			await doConnect();
			return;
		}

		// Otherwise show the prompt for credentials
		connectSession = session;
		connectPassword = '';
		connectKeyPassphrase = '';
		connectError = undefined;
		rememberPassword = false;
		hasSavedPassword = false;
	}

	async function doConnect(): Promise<void> {
		if (!connectSession) return;
		connecting = true;
		connectError = undefined;

		const session = connectSession;
		const id = crypto.randomUUID();
		connectingId = id;
		const authType = session.auth_method.type;

		const passwordToSave = authType === 'Password' ? connectPassword : connectKeyPassphrase;

		// Build jump chain from session config if present
		const jumpChain: JumpHostConnectParams[] | undefined = session.jump_chain && session.jump_chain.length > 0
			? session.jump_chain.map(j => ({
				host: j.host,
				port: j.port,
				username: j.username,
				authMethod: j.auth_method.type === 'Key' ? 'key' : j.auth_method.type.toLowerCase(),
				password: j.auth_method.type === 'Password' ? j.auth_method.password : undefined,
				keyPath: j.auth_method.type === 'Key' ? j.auth_method.path : undefined,
				keyPassphrase: j.auth_method.type === 'Key' ? j.auth_method.passphrase : undefined,
			}))
			: undefined;

		try {
			await sshConnect({
				id,
				host: session.host,
				port: session.port,
				username: session.username,
				authMethod: authType === 'Key' ? 'key' : authType.toLowerCase(),
				password: authType === 'Password' ? connectPassword : undefined,
				keyPath: authType === 'Key' && session.auth_method.type === 'Key' ? session.auth_method.path : undefined,
				keyPassphrase: authType === 'Key' && connectKeyPassphrase ? connectKeyPassphrase : undefined,
				cols: 80,
				rows: 24,
				jumpChain,
			proxy: session.proxy ? {
				proxy_type: session.proxy.proxy_type,
				host: session.proxy.host,
				port: session.proxy.port,
				username: session.proxy.username ?? undefined,
				password: session.proxy.password ?? undefined,
			} : undefined,
			});

			createTab('ssh', `${session.username}@${session.host}`, id, session.name, session.detected_os);
			addToast(t('session.connected_toast', { name: session.name }), 'success');
			connectSession = undefined;

			// Detect OS in background and persist to session
			if (!session.detected_os) {
				sshDetectOs(id).then(async (osId) => {
					if (osId) {
						updateTabOs(id, osId);
						const updated = { ...session, detected_os: osId };
						try {
							await sessionUpdate(updated);
							await loadSessions();
						} catch {
							// Non-critical: icon will show next time
						}
					}
				}).catch(() => {});
			}
		} catch (err) {
			connectError = String(err);
		} finally {
			connecting = false;
		}
	}

	function cancelConnect(): void {
		if (connecting && connectingId) {
			// Try to clean up the in-flight connection on the backend
			sshDisconnect(connectingId).catch(() => {});
		}
		connecting = false;
		connectingId = undefined;
		connectSession = undefined;
	}

	function handleEdit(session: SessionConfig): void {
		editingSession = session;
		showEditor = true;
	}

	async function handleDelete(session: SessionConfig): Promise<void> {
		if (deleteConfirm !== session.id) {
			deleteConfirm = session.id;
			// Auto-clear confirmation after 3 seconds
			setTimeout(() => {
				deleteConfirm = null;
			}, 3000);
			return;
		}

		try {
			await sessionDelete(session.id);
			deleteConfirm = null;
			await loadSessions();
		} catch (err) {
			console.error('Delete failed:', err);
		}
	}

	function handleNewSession(): void {
		editingSession = undefined;
		showEditor = true;
	}

	function handleEditorSave(): void {
		loadSessions();
	}

	// TLS-style: initialize identity (generates X25519 keypair, stores in OS keychain)
	async function handleInitialize(): Promise<void> {
		initializing = true;
		initError = '';
		try {
			await initIdentity(''); // No password needed - TLS-style
			await loadSessions();
			addToast(t('session.identity_created_toast'), 'success');
		} catch (err) {
			initError = String(err);
		} finally {
			initializing = false;
		}
	}

	// Import identity from backup key
	async function handleImport(): Promise<void> {
		if (!importKey.trim()) {
			initError = t('session.enter_backup_key');
			return;
		}
		importing = true;
		initError = '';
		try {
			await importIdentity(importKey.trim());
			await loadSessions();
			importKey = '';
			addToast(t('session.identity_restored_toast'), 'success');
		} catch (err) {
			initError = String(err);
		} finally {
			importing = false;
		}
	}

	// Reset - delete existing data and start fresh
	async function handleReset(): Promise<void> {
		// Delete identity file and vaults to start fresh
		try {
			const { invoke } = await import('@tauri-apps/api/core');
			await invoke('vault_reset');
			await checkState();
			addToast(t('session.data_cleared_toast'), 'info');
		} catch (err) {
			initError = String(err);
		}
	}

	// Load sessions and vaults on mount (auto-unlock via OS keychain)
	$effect(() => {
		untrack(() => {
			checkState().then(async () => {
				if (!vaultState.locked) {
					await refreshVaults();
					await loadSessions();
				} else {
					loading = false;
				}
			});
		});
	});
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="session-list" oncontextmenu={(e) => e.preventDefault()}>
	{#if !hasIdentity}
		<!-- First run: Initialize identity (TLS-style, no password) -->
		<div class="init-section">
			<div class="init-icon">
				<svg width="32" height="32" viewBox="0 0 24 24" fill="none">
					<path d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
				</svg>
			</div>
			<p class="init-title">{t('session.secure_sessions')}</p>
			<p class="init-desc">{t('session.secure_sessions_desc')}</p>
			{#if initError}
				<p class="init-error">{initError}</p>
			{/if}
			<button class="init-btn" onclick={handleInitialize} disabled={initializing}>
				{#if initializing}{t('session.initializing')}{:else}{t('session.initialize')}{/if}
			</button>
		</div>
	{:else if keychainError}
		<!-- Keychain error: data exists but can't access key -->
		<div class="init-section">
			<div class="init-icon error">
				<svg width="32" height="32" viewBox="0 0 24 24" fill="none">
					<path d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
				</svg>
			</div>
			<p class="init-title">{t('session.keychain_error')}</p>
			<p class="init-desc">{t('session.keychain_error_desc')}</p>
			{#if initError}
				<p class="init-error">{initError}</p>
			{/if}
			<input
				class="import-input"
				type="password"
				placeholder={t('session.paste_backup_key')}
				bind:value={importKey}
				disabled={importing}
			/>
			<div class="recovery-buttons">
				<button class="init-btn" onclick={handleImport} disabled={importing || !importKey.trim()}>
					{#if importing}{t('session.restoring')}{:else}{t('session.restore_identity')}{/if}
				</button>
				<button class="reset-btn" onclick={handleReset} disabled={importing}>
					{t('session.start_fresh')}
				</button>
			</div>
		</div>
	{:else if locked}
		<!-- Locked but has identity - keychain access failed -->
		<div class="init-section">
			<p class="init-desc">{t('session.keychain_locked_desc')}</p>
		</div>
	{:else}
		<div class="actions-row">
			<button class="quick-connect-btn" onclick={() => (showQuickConnect = true)}>
				<svg width="11" height="11" viewBox="0 0 24 24" fill="none">
					<path
						d="M13 10V3L4 14h7v7l9-11h-7z"
						stroke="currentColor"
						stroke-width="2"
						stroke-linecap="round"
						stroke-linejoin="round"
					/>
				</svg>
				{t('session.quick_connect')}
			</button>
			<button class="save-session-btn" onclick={handleNewSession}>
				<svg width="11" height="11" viewBox="0 0 24 24" fill="none">
					<path
						d="M12 5v14M5 12h14"
						stroke="currentColor"
						stroke-width="2"
						stroke-linecap="round"
					/>
				</svg>
				{t('session.save_session')}
			</button>
			<button class="save-session-btn" onclick={() => (showImport = true)} title={t('session.import_ssh_config')}>
				<svg width="11" height="11" viewBox="0 0 24 24" fill="none">
					<path
						d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4M7 10l5 5 5-5M12 15V3"
						stroke="currentColor"
						stroke-width="2"
						stroke-linecap="round"
						stroke-linejoin="round"
					/>
				</svg>
				{t('session.import_ssh_config')}
				<span class="beta-badge">BETA</span>
			</button>
		</div>

		<VaultSelector onvaultselect={(id) => { selectedVaultId = id; creatingFolder = false; newFolderName = ''; }} onrefresh={() => loadSessions()} />

		<div class="search-row">
			<svg class="search-icon" width="12" height="12" viewBox="0 0 24 24" fill="none">
				<circle cx="11" cy="11" r="8" stroke="currentColor" stroke-width="2"/>
				<path d="M21 21l-4.35-4.35" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
			</svg>
			<input
				class="search-input"
				type="text"
				placeholder={t('session.search_placeholder')}
				bind:value={searchQuery}
			/>
			{#if searchQuery}
				<button class="search-clear" onclick={() => (searchQuery = '')} aria-label={t('common.clear')}>
					<svg width="10" height="10" viewBox="0 0 10 10" fill="none">
						<path d="M1 1l8 8M9 1L1 9" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>
					</svg>
				</button>
			{/if}
		</div>

		{#if loading}
			<div class="loading-state">
				<span class="spinner"></span>
				<span class="loading-text">{t('session.loading')}</span>
			</div>
		{:else if filteredSessions.length === 0}
			<!-- svelte-ignore a11y_no_static_element_interactions -->
			<div class="empty-state-area" oncontextmenu={openBackgroundContextMenu}>
				{#if creatingFolder}
					<div class="new-folder-row">
						<svg width="14" height="14" viewBox="0 0 24 24" fill="none" class="new-folder-icon">
							<path d="M22 19a2 2 0 01-2 2H4a2 2 0 01-2-2V5a2 2 0 012-2h5l2 3h9a2 2 0 012 2z" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
						</svg>
						<form class="new-folder-form" onsubmit={(e) => { e.preventDefault(); handleCreateFolder(); }}>
							<input class="new-folder-input" type="text" placeholder={t('session.folder_name')} bind:value={newFolderName} />
							<button class="new-folder-save" type="submit" disabled={!newFolderName.trim()}>{t('common.save')}</button>
							<button class="new-folder-cancel" type="button" onclick={() => { creatingFolder = false; newFolderName = ''; }}>{t('common.cancel')}</button>
						</form>
					</div>
				{:else}
					<p class="empty-state">{t('session.no_sessions_vault')}</p>
				{/if}
			</div>
		{:else}
		{#if creatingFolder}
			<div class="new-folder-row">
				<svg width="14" height="14" viewBox="0 0 24 24" fill="none" class="new-folder-icon">
					<path d="M22 19a2 2 0 01-2 2H4a2 2 0 01-2-2V5a2 2 0 012-2h5l2 3h9a2 2 0 012 2z" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
				</svg>
				<form class="new-folder-form" onsubmit={(e) => { e.preventDefault(); handleCreateFolder(); }}>
					<input class="new-folder-input" type="text" placeholder={t('session.folder_name')} bind:value={newFolderName} />
					<button class="new-folder-save" type="submit" disabled={!newFolderName.trim()}>{t('common.save')}</button>
					<button class="new-folder-cancel" type="button" onclick={() => { creatingFolder = false; newFolderName = ''; }}>{t('common.cancel')}</button>
				</form>
			</div>
		{/if}
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<!-- svelte-ignore a11y_click_events_have_key_events -->
		<div
			class="sessions-scroll"
			oncontextmenu={openBackgroundContextMenu}
			onclick={closeContextMenu}
			data-folder-id="__ungrouped__"
			class:drop-active={dropTarget === null && dragging}
		>
			{#each groupedSessions as group (group.folder?.id ?? '__ungrouped__')}
				{#if group.folder}
					<!-- svelte-ignore a11y_no_static_element_interactions -->
					<div
						class="folder-header"
						class:drop-active={dropTarget === group.folder.id && dragging}
						data-folder-id={group.folder.id}
					>
						<button class="folder-toggle" onclick={() => toggleFolder(group.folder!.id)}>
							<svg width="10" height="10" viewBox="0 0 10 10" fill="none" class="folder-chevron" class:collapsed={collapsedFolders.has(group.folder!.id)}>
								<path d="M3 2l4 3-4 3" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>
							</svg>
							<svg width="12" height="12" viewBox="0 0 24 24" fill="none" class="folder-icon">
								<path d="M22 19a2 2 0 01-2 2H4a2 2 0 01-2-2V5a2 2 0 012-2h5l2 3h9a2 2 0 012 2z" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
							</svg>
							<span class="folder-name">{group.folder.name}</span>
							<span class="folder-count">{group.sessions.length}</span>
						</button>
						<button
							class="folder-delete-btn"
							class:confirm={deleteFolderConfirm === group.folder!.id}
							onclick={() => handleDeleteFolder(group.folder!.id)}
							title={deleteFolderConfirm === group.folder!.id ? t('common.confirm') : t('common.delete')}
						>
							{#if deleteFolderConfirm === group.folder!.id}
								<span class="folder-delete-confirm-text">{t('common.confirm')}</span>
							{:else}
								<svg width="10" height="10" viewBox="0 0 10 10" fill="none">
									<path d="M1 1l8 8M9 1L1 9" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>
								</svg>
							{/if}
						</button>
					</div>
					{#if !collapsedFolders.has(group.folder.id)}
						{#each group.sessions as session (session.id)}
							{#if deleteConfirm === session.id}
								<div class="delete-confirm">
									<span class="delete-confirm-text">{t('session.delete_confirm', { name: session.name })}</span>
									<button class="delete-confirm-btn" onclick={() => handleDelete(session)}>{t('common.confirm')}</button>
									<button class="delete-cancel-btn" onclick={() => (deleteConfirm = null)}>{t('common.cancel')}</button>
								</div>
							{:else}
								<div class="folder-session">
									<SessionCard {session} onconnect={() => handleConnect(session)} onedit={() => handleEdit(session)} ondelete={() => handleDelete(session)} oncontextmenu={(e) => openSessionContextMenu(e, session)} ondragstart={(e) => handleDragStart(e, session)} ondragend={() => {}} />
								</div>
							{/if}
						{/each}
					{/if}
				{:else}
					{#each group.sessions as session (session.id)}
						{#if deleteConfirm === session.id}
							<div class="delete-confirm">
								<span class="delete-confirm-text">{t('session.delete_confirm', { name: session.name })}</span>
								<button class="delete-confirm-btn" onclick={() => handleDelete(session)}>{t('common.confirm')}</button>
								<button class="delete-cancel-btn" onclick={() => (deleteConfirm = null)}>{t('common.cancel')}</button>
							</div>
						{:else}
							<SessionCard {session} onconnect={() => handleConnect(session)} onedit={() => handleEdit(session)} ondelete={() => handleDelete(session)} oncontextmenu={(e) => openSessionContextMenu(e, session)} ondragstart={(e) => handleDragStart(e, session)} ondragend={() => {}} />
						{/if}
					{/each}
				{/if}
			{/each}
		</div>
		{/if}
	{/if}

	{#if contextMenu}
		<div class="context-menu" style="left: {contextMenu.x}px; top: {contextMenu.y}px;">
			{#if contextMenu.session}
				<button class="context-item" onclick={() => { if (contextMenu?.session) handleConnect(contextMenu.session); closeContextMenu(); }} type="button">
					{t('session.connect')}
				</button>
				<button class="context-item" onclick={() => { if (contextMenu?.session) handleEdit(contextMenu.session); closeContextMenu(); }} type="button">
					{t('session.edit')}
				</button>
				<div class="context-sep"></div>
				<div class="context-label">{t('session.move_to_folder')}</div>
				{#each folders as folder (folder.id)}
					<button class="context-item context-folder-item" onclick={() => { if (contextMenu?.session) moveToFolder(contextMenu.session, folder.id); }} type="button">
						<svg width="12" height="12" viewBox="0 0 24 24" fill="none" class="ctx-folder-icon">
							<path d="M22 19a2 2 0 01-2 2H4a2 2 0 01-2-2V5a2 2 0 012-2h5l2 3h9a2 2 0 012 2z" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
						</svg>
						{folder.name}
					</button>
				{/each}
				<button class="context-item context-folder-item" onclick={contextNewFolder} type="button">
					<svg width="12" height="12" viewBox="0 0 24 24" fill="none" class="ctx-folder-icon">
						<path d="M12 5v14M5 12h14" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
					</svg>
					{t('session.new_folder')}
				</button>
				{#if contextMenu.session.folder_id}
					<button class="context-item" onclick={() => { if (contextMenu?.session) moveToFolder(contextMenu.session, null); }} type="button">
						{t('session.remove_from_folder')}
					</button>
				{/if}
				<div class="context-sep"></div>
				<button class="context-item context-danger" onclick={() => { if (contextMenu?.session) handleDelete(contextMenu.session); closeContextMenu(); }} type="button">
					{t('common.delete')}
				</button>
			{:else}
				<button class="context-item" onclick={contextNewFolder} type="button">
					{t('session.new_folder')}
				</button>
			{/if}
		</div>
	{/if}
</div>

<QuickConnect bind:open={showQuickConnect} />
<SessionEditor bind:open={showEditor} editSession={editingSession} vaultId={selectedVaultId} {folders} onsave={handleEditorSave} />
<SshConfigImport bind:open={showImport} onsave={handleEditorSave} />

{#if showConnectPrompt && connectSession}
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div class="prompt-overlay" onkeydown={(e) => { if (e.key === 'Escape') cancelConnect(); }} onclick={cancelConnect}>
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div class="prompt-box" onclick={(e) => e.stopPropagation()} onkeydown={() => {}}>
			<div class="prompt-header">
				<span class="prompt-title">{t('session.connect_to', { name: connectSession.name })}</span>
				<span class="prompt-detail">{connectSession.username}@{connectSession.host}:{connectSession.port}</span>
			</div>

			<form class="prompt-form" onsubmit={(e) => { e.preventDefault(); doConnect(); }}>
				{#if connectSession.auth_method.type === 'Password'}
					<input
						class="prompt-input"
						type="password"
						placeholder={t('session.password')}
						bind:value={connectPassword}
						disabled={connecting}
					/>
				{:else if connectSession.auth_method.type === 'Key'}
					<input
						class="prompt-input"
						type="password"
						placeholder={t('session.key_passphrase_optional')}
						bind:value={connectKeyPassphrase}
						disabled={connecting}
					/>
				{/if}

				{#if connectSession.auth_method.type !== 'Agent'}
					<label class="remember-label">
						<input type="checkbox" class="remember-check" bind:checked={rememberPassword} disabled={connecting} />
						<span class="remember-text">{hasSavedPassword ? t('session.password_saved') : t('session.remember_password')}</span>
					</label>
				{/if}

				{#if connectError}
					<div class="prompt-error">{connectError}</div>
				{/if}

				<div class="prompt-actions">
					<button type="button" class="prompt-btn prompt-cancel" onclick={cancelConnect}>{t('common.cancel')}</button>
					<button type="submit" class="prompt-btn prompt-connect" disabled={connecting}>
						{#if connecting}{t('session.connecting')}{:else}{t('session.connect')}{/if}
					</button>
				</div>
			</form>
		</div>
	</div>
{/if}


<style>
	.session-list {
		display: flex;
		flex-direction: column;
		gap: 8px;
		padding: 4px 0;
		height: 100%;
		overflow: hidden;
	}

	.actions-row {
		display: flex;
		gap: 4px;
	}

	.beta-badge {
		padding: 1px 4px;
		font-size: 0.45rem;
		font-weight: 700;
		letter-spacing: 0.05em;
		color: #fff;
		background: linear-gradient(135deg, #ff6b35, #f7c948);
		border-radius: 3px;
		line-height: 1.4;
	}

	.quick-connect-btn,
	.save-session-btn {
		display: flex;
		align-items: center;
		gap: 4px;
		flex: 1;
		padding: 5px 8px;
		font-family: var(--font-sans);
		font-size: 0.6875rem;
		font-weight: 500;
		border-radius: 6px;
		cursor: pointer;
		transition:
			background-color var(--duration-default) var(--ease-default),
			color var(--duration-default) var(--ease-default);
	}

	.quick-connect-btn {
		color: var(--color-accent);
		background: transparent;
		border: 1px solid var(--color-accent);
	}

	.quick-connect-btn:hover {
		background-color: rgba(0, 122, 255, 0.1);
	}

	.save-session-btn {
		color: var(--color-text-secondary);
		background: transparent;
		border: 1px solid var(--color-border);
	}

	.save-session-btn:hover {
		background-color: rgba(255, 255, 255, 0.06);
		color: var(--color-text-primary);
	}

	.quick-connect-btn:active,
	.save-session-btn:active {
		transform: scale(0.98);
	}

	.search-row {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 4px 8px;
		background: var(--color-bg-primary);
		border: 1px solid var(--color-border);
		border-radius: 6px;
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
		font-size: 0.6875rem;
		outline: none;
	}

	.search-input::placeholder {
		color: var(--color-text-secondary);
		opacity: 0.5;
	}

	.search-clear {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 18px;
		height: 18px;
		border: none;
		border-radius: 4px;
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
		flex-shrink: 0;
	}

	.search-clear:hover {
		background: rgba(255, 255, 255, 0.08);
		color: var(--color-text-primary);
	}


	.new-folder-row {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 6px 8px;
		background: rgba(255, 255, 255, 0.02);
		border-radius: 6px;
		border: 1px solid var(--color-border);
	}

	.new-folder-icon {
		flex-shrink: 0;
		color: var(--color-warning, #ffd60a);
		opacity: 0.7;
	}

	.new-folder-form {
		display: flex;
		align-items: center;
		gap: 4px;
		flex: 1;
	}

	.new-folder-input {
		flex: 1;
		min-width: 0;
		padding: 5px 10px;
		border: 1px solid var(--color-border);
		border-radius: 5px;
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		font-family: var(--font-sans);
		font-size: 0.6875rem;
		outline: none;
	}

	.new-folder-input:focus {
		border-color: var(--color-accent);
	}

	.new-folder-save,
	.new-folder-cancel {
		padding: 4px 8px;
		font-family: var(--font-sans);
		font-size: 0.625rem;
		font-weight: 500;
		border: none;
		border-radius: 4px;
		cursor: pointer;
	}

	.new-folder-save {
		background: var(--color-accent);
		color: #fff;
	}

	.new-folder-save:disabled {
		opacity: 0.4;
		cursor: default;
	}

	.new-folder-cancel {
		background: transparent;
		color: var(--color-text-secondary);
	}

	.folder-header {
		display: flex;
		align-items: center;
		gap: 2px;
		padding: 5px 6px;
		border-radius: 6px;
		transition: background-color 0.15s ease, outline 0.15s ease;
	}

	.folder-toggle {
		flex: 1;
		display: flex;
		align-items: center;
		gap: 5px;
		padding: 2px 4px;
		border: none;
		border-radius: 4px;
		background: transparent;
		color: var(--color-text-secondary);
		font-family: var(--font-sans);
		font-size: 0.6875rem;
		cursor: pointer;
		pointer-events: auto;
		transition: background-color var(--duration-default) var(--ease-default);
	}

	.folder-toggle:hover {
		background-color: rgba(255, 255, 255, 0.04);
	}

	.folder-chevron {
		transition: transform 0.15s ease;
		transform: rotate(90deg);
	}

	.folder-chevron.collapsed {
		transform: rotate(0deg);
	}

	.folder-icon {
		color: var(--color-warning, #ffd60a);
		opacity: 0.7;
	}

	.folder-name {
		font-weight: 500;
		color: var(--color-text-primary);
	}

	.folder-count {
		font-size: 0.5625rem;
		color: var(--color-text-secondary);
		opacity: 0.6;
	}

	.folder-delete-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 20px;
		height: 20px;
		border: none;
		border-radius: 4px;
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
		opacity: 0;
		transition: opacity var(--duration-default) var(--ease-default), background-color var(--duration-default) var(--ease-default);
	}

	.folder-header:hover .folder-delete-btn {
		opacity: 1;
	}

	.folder-delete-btn:hover {
		background: rgba(255, 69, 58, 0.12);
		color: var(--color-danger);
	}

	.folder-delete-btn.confirm {
		opacity: 1;
		width: auto;
		padding: 2px 8px;
		background: var(--color-danger, #ff453a);
		color: #fff;
		border-radius: 4px;
	}

	.folder-delete-btn.confirm:hover {
		background: #ff6961;
		color: #fff;
	}

	.folder-delete-confirm-text {
		font-size: 0.5625rem;
		font-weight: 600;
		white-space: nowrap;
	}

	.folder-session {
		padding-left: 12px;
	}

	.context-menu {
		position: fixed;
		min-width: 180px;
		padding: 4px 0;
		background-color: var(--color-bg-elevated, #1c1c1e);
		border: 1px solid var(--color-border);
		border-radius: 8px;
		box-shadow: 0 8px 32px rgba(0, 0, 0, 0.4);
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

	.context-folder-item {
		padding-left: 20px;
	}

	.context-danger {
		color: var(--color-danger, #ff453a);
	}

	.context-danger:hover {
		background-color: rgba(255, 69, 58, 0.12);
	}

	.context-label {
		padding: 4px 12px 2px;
		font-size: 0.625rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--color-text-secondary);
		opacity: 0.6;
	}

	.ctx-folder-icon {
		color: var(--color-warning, #ffd60a);
		opacity: 0.7;
		flex-shrink: 0;
	}

	.context-sep {
		height: 1px;
		margin: 4px 0;
		background-color: var(--color-border);
	}


	.sessions-scroll {
		display: flex;
		flex-direction: column;
		gap: 2px;
		overflow-y: auto;
		flex: 1;
		min-height: 60px;
	}

	.drop-active {
		outline: 2px dashed rgba(100, 160, 255, 0.5);
		outline-offset: -2px;
		background-color: rgba(100, 160, 255, 0.04);
		border-radius: 4px;
	}

	.loading-state {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 8px;
		padding: 16px 0;
	}

	.loading-text {
		font-size: 0.75rem;
		color: var(--color-text-secondary);
	}

	.empty-state-area {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 10px;
		padding: 16px 4px;
		flex: 1;
	}

	.empty-state {
		margin: 0;
		padding: 0;
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
		opacity: 0.7;
		text-align: center;
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
		font-size: 0.75rem;
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
		font-size: 0.6875rem;
		font-weight: 500;
		border: none;
		border-radius: 4px;
		cursor: pointer;
		transition:
			background-color var(--duration-default) var(--ease-default),
			opacity var(--duration-default) var(--ease-default);
	}

	.delete-confirm-btn {
		background-color: var(--color-danger);
		color: #fff;
	}

	.delete-confirm-btn:hover {
		opacity: 0.85;
	}

	.delete-cancel-btn {
		background-color: rgba(255, 255, 255, 0.08);
		color: var(--color-text-secondary);
	}

	.delete-cancel-btn:hover {
		background-color: rgba(255, 255, 255, 0.12);
	}

	.spinner {
		display: inline-block;
		width: 14px;
		height: 14px;
		border: 2px solid rgba(255, 255, 255, 0.15);
		border-top-color: var(--color-accent);
		border-radius: 50%;
		animation: spin 0.6s linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}

	/* Connect prompt overlay */
	.prompt-overlay {
		position: fixed;
		inset: 0;
		z-index: 200;
		display: flex;
		align-items: center;
		justify-content: center;
		background: rgba(0, 0, 0, 0.5);
		backdrop-filter: blur(4px);
	}

	.prompt-box {
		width: 320px;
		background-color: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-card);
		box-shadow: var(--shadow-elevated);
		overflow: hidden;
	}

	.prompt-header {
		display: flex;
		flex-direction: column;
		gap: 2px;
		padding: 14px 16px 10px;
		border-bottom: 1px solid var(--color-border);
	}

	.prompt-title {
		font-size: 0.8125rem;
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.prompt-detail {
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
		font-family: var(--font-mono);
	}

	.prompt-form {
		display: flex;
		flex-direction: column;
		gap: 10px;
		padding: 12px 16px 14px;
	}

	.prompt-input {
		width: 100%;
		padding: 8px 10px;
		font-family: var(--font-sans);
		font-size: 0.8125rem;
		color: var(--color-text-primary);
		background-color: var(--color-bg-primary);
		border: 1px solid var(--color-border);
		border-radius: 6px;
		outline: none;
		box-sizing: border-box;
		transition: border-color var(--duration-default) var(--ease-default);
	}

	.prompt-input:focus {
		border-color: var(--color-accent);
	}

	.remember-label {
		display: flex;
		align-items: center;
		gap: 6px;
		cursor: pointer;
	}

	.remember-check {
		width: 14px;
		height: 14px;
		accent-color: var(--color-accent);
		cursor: pointer;
	}

	.remember-check:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.remember-text {
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
	}

	.prompt-error {
		padding: 6px 10px;
		font-size: 0.6875rem;
		color: var(--color-danger);
		background-color: rgba(255, 69, 58, 0.08);
		border-radius: 4px;
	}

	.prompt-actions {
		display: flex;
		justify-content: flex-end;
		gap: 6px;
	}

	.prompt-btn {
		padding: 6px 14px;
		font-family: var(--font-sans);
		font-size: 0.75rem;
		font-weight: 500;
		border: none;
		border-radius: 6px;
		cursor: pointer;
		transition: background-color var(--duration-default) var(--ease-default);
	}

	.prompt-btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.prompt-cancel {
		background: transparent;
		color: var(--color-text-secondary);
	}

	.prompt-cancel:hover:not(:disabled) {
		background-color: rgba(255, 255, 255, 0.06);
	}

	.prompt-connect {
		background-color: var(--color-accent);
		color: #fff;
	}

	.prompt-connect:hover:not(:disabled) {
		background-color: var(--color-accent-hover);
	}

	/* Init section (first run) */
	.init-section {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 12px;
		padding: 24px 16px;
		text-align: center;
	}

	.init-icon {
		color: var(--color-text-secondary);
		opacity: 0.6;
	}

	.init-icon.error {
		color: var(--color-danger);
		opacity: 1;
	}

	.import-input {
		width: 100%;
		max-width: 240px;
		padding: 8px 12px;
		font-family: var(--font-mono);
		font-size: 0.75rem;
		color: var(--color-text-primary);
		background: var(--color-bg-primary);
		border: 1px solid var(--color-border);
		border-radius: 6px;
		outline: none;
	}

	.import-input:focus {
		border-color: var(--color-accent);
	}

	.recovery-buttons {
		display: flex;
		gap: 8px;
	}

	.reset-btn {
		padding: 8px 16px;
		font-family: var(--font-sans);
		font-size: 0.75rem;
		font-weight: 500;
		color: var(--color-text-secondary);
		background: transparent;
		border: 1px solid var(--color-border);
		border-radius: 6px;
		cursor: pointer;
		transition: background-color var(--duration-default) var(--ease-default);
	}

	.reset-btn:hover:not(:disabled) {
		background-color: rgba(255, 255, 255, 0.05);
	}

	.reset-btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.init-title {
		margin: 0;
		font-size: 0.875rem;
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.init-desc {
		margin: 0;
		font-size: 0.75rem;
		color: var(--color-text-secondary);
		max-width: 200px;
		line-height: 1.4;
	}

	.init-error {
		margin: 0;
		font-size: 0.6875rem;
		color: var(--color-danger);
	}

	.init-btn {
		padding: 8px 20px;
		font-family: var(--font-sans);
		font-size: 0.75rem;
		font-weight: 500;
		color: #fff;
		background-color: var(--color-accent);
		border: none;
		border-radius: 6px;
		cursor: pointer;
		transition: background-color var(--duration-default) var(--ease-default);
	}

	.init-btn:hover:not(:disabled) {
		background-color: var(--color-accent-hover);
	}

	.init-btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}
</style>
