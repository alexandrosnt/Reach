<script lang="ts">
	import QuickConnect from './QuickConnect.svelte';
	import SessionEditor from './SessionEditor.svelte';
	import SshConfigImport from './SshConfigImport.svelte';
	import SessionCard from './SessionCard.svelte';
	import Modal from '$lib/components/shared/Modal.svelte';
	import Input from '$lib/components/shared/Input.svelte';
	import Button from '$lib/components/shared/Button.svelte';
	import VaultSelector from '$lib/components/vault/VaultSelector.svelte';
	import { sessionList, sessionDelete, sessionUpdate, sessionListFolders, sessionCreateFolder, sessionUpdateFolder, type SessionConfig, type Folder } from '$lib/ipc/sessions';
	import { sshConnect, sshDisconnect, sshDetectOs, type JumpHostConnectParams } from '$lib/ipc/ssh';
	// Passwords are now stored encrypted in vault, not in memory cache
	import { createTab } from '$lib/state/tabs.svelte';
	import { addToast } from '$lib/state/toasts.svelte';
	import { t } from '$lib/state/i18n.svelte';
	import { untrack, onMount } from 'svelte';
	import { flip } from 'svelte/animate';
	import { vaultState, checkState, initIdentity, refreshVaults, importIdentity } from '$lib/state/vault.svelte';

	let showQuickConnect = $state(false);
	let showEditor = $state(false);
	let showImport = $state(false);
	let editingSession = $state<SessionConfig | undefined>();
	let sessions = $state<SessionConfig[]>([]);
	let loading = $state(true);
	let deleteConfirm = $state<string | null>(null);

	let folders = $state<Folder[]>([]);
	let folderCollapse = $state<Record<string, boolean>>({});
	let showFolderModal = $state(false);
	let newFolderName = $state('');
	let newFolderIcon = $state('📁');
	let newFolderColor = $state('#3b82f6');
	let folderEditing = $state<Folder | null>(null);

	let draggingSessionId = $state<string | null>(null);
	let draggingFolderId = $state<string | null>(null);

	// Selected vault filter (null = private/default vault)
	let selectedVaultId = $state<string | null>(null);

	// Filter sessions by selected vault
	let filteredSessions = $derived(
		sessions.filter(s => {
			if (selectedVaultId === null) {
				// Show sessions without vault_id (private vault)
				return !s.vault_id;
			}
			return s.vault_id === selectedVaultId;
		})
	);

	let sortedFolders = $derived(
		[...folders].sort((a, b) => (a.order - b.order) || a.name.localeCompare(b.name))
	);

	function sortedSessions(list: SessionConfig[]): SessionConfig[] {
		return [...list].sort((a, b) => (a.order - b.order) || a.name.localeCompare(b.name));
	}

	let ungroupedSessions = $derived(
		sortedSessions(filteredSessions.filter(s => !s.folder_id))
	);

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

	async function loadFolders(): Promise<void> {
		try {
			folders = await sessionListFolders();
		} catch (err) {
			console.error('Failed to load folders:', err);
		}
	}

	async function loadSessions(): Promise<void> {
		try {
			sessions = await sessionList();
			await loadFolders();
		} catch (err) {
			console.error('Failed to load sessions:', err);
		} finally {
			loading = false;
		}
	}

	onMount(() => {
		try {
			const raw = localStorage.getItem('reach.folderCollapse');
			if (raw) {
				folderCollapse = JSON.parse(raw);
			}
		} catch {
			// ignore
		}
	});

	$effect(() => {
		try {
			localStorage.setItem('reach.folderCollapse', JSON.stringify(folderCollapse));
		} catch {
			// ignore
		}
	});

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
			});

			createTab('ssh', `${session.username}@${session.host}`, id);
			addToast(t('session.connected_toast', { name: session.name }), 'success');
			connectSession = undefined;

			// Detect OS in background and persist to session
			if (!session.detected_os) {
				sshDetectOs(id).then(async (osId) => {
					if (osId) {
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

	function openCreateFolder(): void {
		newFolderName = '';
		newFolderIcon = '📁';
		newFolderColor = '#3b82f6';
		folderEditing = null;
		showFolderModal = true;
	}

	function openEditFolder(folder: Folder): void {
		newFolderName = folder.name;
		newFolderIcon = folder.icon ?? '📁';
		newFolderColor = folder.color ?? '#3b82f6';
		folderEditing = folder;
		showFolderModal = true;
	}

	async function saveFolder(): Promise<void> {
		const name = newFolderName.trim();
		if (!name) return;

		if (folderEditing) {
			const updated: Folder = {
				...folderEditing,
				name,
				icon: newFolderIcon.trim() || null,
				color: newFolderColor || null,
			};
			const saved = await sessionUpdateFolder(updated);
			folders = folders.map(f => (f.id === saved.id ? saved : f));
		} else {
			const order = folders.length ? Math.max(...folders.map(f => f.order ?? 0)) + 1 : 0;
			const created = await sessionCreateFolder({
				name,
				parentId: null,
				icon: newFolderIcon.trim() || null,
				color: newFolderColor || null,
				order,
			});
			folders = [...folders, created];
		}

		showFolderModal = false;
		newFolderName = '';
		newFolderIcon = '📁';
		newFolderColor = '#3b82f6';
		addToast('Saved', 'success');
	}

	function toggleFolder(folderId: string): void {
		folderCollapse = { ...folderCollapse, [folderId]: !folderCollapse[folderId] };
	}

	function handleFolderDragStart(folder: Folder, event: DragEvent): void {
		if (!event.dataTransfer) return;
		draggingFolderId = folder.id;
		event.dataTransfer.setData('folderId', folder.id);
		event.dataTransfer.effectAllowed = 'move';
	}

	function handleFolderDragEnd(): void {
		draggingFolderId = null;
	}

	async function handleFolderDrop(targetFolderId: string, event: DragEvent): Promise<void> {
		const sourceId = event.dataTransfer?.getData('folderId');
		if (!sourceId || sourceId === targetFolderId) return;

		const list = [...sortedFolders];
		const sourceIndex = list.findIndex(f => f.id === sourceId);
		const targetIndex = list.findIndex(f => f.id === targetFolderId);
		if (sourceIndex === -1 || targetIndex === -1) return;

		const [moved] = list.splice(sourceIndex, 1);
		list.splice(targetIndex, 0, moved);

		const updates: Folder[] = [];
		list.forEach((f, idx) => {
			if (f.order !== idx) {
				updates.push({ ...f, order: idx });
			}
		});

		if (updates.length) {
			const updateMap = new Map(updates.map(u => [u.id, u]));
			folders = folders.map(f => updateMap.get(f.id) ?? f);
			await Promise.all(updates.map(sessionUpdateFolder));
		}
	}

	function handleSessionDragStart(session: SessionConfig, event: DragEvent): void {
		if (!event.dataTransfer) return;
		draggingSessionId = session.id;
		event.dataTransfer.setData('sessionId', session.id);
		event.dataTransfer.effectAllowed = 'move';
	}

	function handleSessionDragEnd(): void {
		draggingSessionId = null;
	}

	async function moveSessionTo(folderId: string | null, targetIndex?: number): Promise<void> {
		if (!draggingSessionId) return;
		const session = sessions.find(s => s.id === draggingSessionId);
		if (!session) return;

		const sourceFolderId = session.folder_id ?? null;
		const targetFolderId = folderId;

		const targetSessions = sortedSessions(
			filteredSessions.filter(s => (s.folder_id ?? null) === targetFolderId && s.id !== session.id)
		);
		const insertIndex = targetIndex === undefined
			? targetSessions.length
			: Math.max(0, Math.min(targetIndex, targetSessions.length));

		const newTargetSessions = [...targetSessions];
		newTargetSessions.splice(insertIndex, 0, { ...session, folder_id: targetFolderId, order: insertIndex });

		const updates = new Map<string, SessionConfig>();
		newTargetSessions.forEach((s, idx) => {
			if (s.order !== idx || (s.folder_id ?? null) !== targetFolderId) {
				updates.set(s.id, { ...s, order: idx, folder_id: targetFolderId });
			}
		});

		if (sourceFolderId !== targetFolderId) {
			const sourceSessions = sortedSessions(
				filteredSessions.filter(s => (s.folder_id ?? null) === sourceFolderId && s.id !== session.id)
			);
			sourceSessions.forEach((s, idx) => {
				if (s.order !== idx) {
					updates.set(s.id, { ...s, order: idx });
				}
			});
		}

		if (updates.size) {
			const updatedSessions = new Map(updates);
			sessions = sessions.map(s => updatedSessions.get(s.id) ?? s);
			await Promise.all([...updates.values()].map(sessionUpdate));
		}
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

<div class="session-list">
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
			<button class="save-session-btn" onclick={openCreateFolder} title="New Folder">
				<svg width="11" height="11" viewBox="0 0 24 24" fill="none">
					<path
						d="M3 7h6l2 2h10v9a2 2 0 01-2 2H5a2 2 0 01-2-2V7z"
						stroke="currentColor"
						stroke-width="2"
						stroke-linecap="round"
						stroke-linejoin="round"
					/>
				</svg>
				New Folder
			</button>
		</div>

		<VaultSelector onvaultselect={(id) => (selectedVaultId = id)} onrefresh={() => loadSessions()} />

		{#if loading}
			<div class="loading-state">
				<span class="spinner"></span>
				<span class="loading-text">{t('session.loading')}</span>
			</div>
		{:else if filteredSessions.length === 0}
			<p class="empty-state">{t('session.no_sessions_vault')}</p>
		{:else}
		<div class="divider"></div>
		<div class="sessions-scroll">
			{#if ungroupedSessions.length > 0}
				<div
					class="folder-block ungrouped"
					style="--folder-color: #374151;"
					on:dragover|preventDefault
					on:drop={(e) => moveSessionTo(null)}
				>
					<div class="folder-header">
						<div class="folder-title">
							<span class="folder-icon">🧷</span>
							<span class="folder-name">Ungrouped</span>
							<span class="folder-count">{ungroupedSessions.length}</span>
						</div>
					</div>
					<div class="folder-content" on:dragover|preventDefault on:drop={(e) => moveSessionTo(null)}
					>
						{#each ungroupedSessions as session, index (session.id)}
							<div
								class="session-card-wrapper"
								draggable="true"
								on:dragstart={(e) => handleSessionDragStart(session, e)}
								on:dragend={handleSessionDragEnd}
								on:dragover|preventDefault
								on:drop={(e) => moveSessionTo(null, index)}
								animate:flip
							>
								{#if deleteConfirm === session.id}
									<div class="delete-confirm">
										<span class="delete-confirm-text">{t('session.delete_confirm', { name: session.name })}</span>
										<button class="delete-confirm-btn" onclick={() => handleDelete(session)}>
											{t('common.confirm')}
										</button>
										<button class="delete-cancel-btn" onclick={() => (deleteConfirm = null)}>
											{t('common.cancel')}
										</button>
									</div>
								{:else}
									<SessionCard
										{session}
										onconnect={() => handleConnect(session)}
										onedit={() => handleEdit(session)}
										ondelete={() => handleDelete(session)}
									/>
								{/if}
							</div>
						{/each}
					</div>
				</div>
			{/if}

			{#each sortedFolders as folder (folder.id)}
				{@const folderSessions = sortedSessions(filteredSessions.filter(s => s.folder_id === folder.id))}
				<div
					class="folder-block"
					style="--folder-color: {folder.color ?? '#3b82f6'};"
					on:dragover|preventDefault
					on:drop={(e) => handleFolderDrop(folder.id, e)}
					animate:flip
				>
					<div
						class="folder-header"
						draggable="true"
						on:dragstart={(e) => handleFolderDragStart(folder, e)}
						on:dragend={handleFolderDragEnd}
					>
						<div class="folder-title" onclick={() => toggleFolder(folder.id)}>
							<span class="folder-icon">{folder.icon ?? '📁'}</span>
							<span class="folder-name">{folder.name}</span>
							<span class="folder-count">{folderSessions.length}</span>
						</div>
						<div class="folder-actions">
							<button class="folder-btn" onclick={() => openEditFolder(folder)} title="Edit">✏️</button>
							<button class="folder-btn" onclick={() => toggleFolder(folder.id)} title="Toggle">{folderCollapse[folder.id] ? '▸' : '▾'}</button>
						</div>
					</div>

					{#if !folderCollapse[folder.id]}
						<div class="folder-content" on:dragover|preventDefault on:drop={(e) => moveSessionTo(folder.id)}
						>
							{#if folderSessions.length === 0}
								<div class="folder-empty">Drop sessions here</div>
							{/if}
							{#each folderSessions as session, index (session.id)}
								<div
									class="session-card-wrapper"
									draggable="true"
									on:dragstart={(e) => handleSessionDragStart(session, e)}
									on:dragend={handleSessionDragEnd}
									on:dragover|preventDefault
									on:drop={(e) => moveSessionTo(folder.id, index)}
									animate:flip
								>
									{#if deleteConfirm === session.id}
										<div class="delete-confirm">
											<span class="delete-confirm-text">{t('session.delete_confirm', { name: session.name })}</span>
											<button class="delete-confirm-btn" onclick={() => handleDelete(session)}>
												{t('common.confirm')}
											</button>
											<button class="delete-cancel-btn" onclick={() => (deleteConfirm = null)}>
												{t('common.cancel')}
											</button>
										</div>
									{:else}
										<SessionCard
											{session}
											onconnect={() => handleConnect(session)}
											onedit={() => handleEdit(session)}
											ondelete={() => handleDelete(session)}
										/>
									{/if}
								</div>
							{/each}
						</div>
					{/if}
				</div>
			{/each}
		</div>
		{/if}
	{/if}
</div>

<QuickConnect bind:open={showQuickConnect} />
<SessionEditor bind:open={showEditor} editSession={editingSession} vaultId={selectedVaultId} onsave={handleEditorSave} />
<SshConfigImport bind:open={showImport} onsave={handleEditorSave} />

<Modal open={showFolderModal} onclose={() => (showFolderModal = false)} title={folderEditing ? 'Edit Folder' : 'New Folder'}>
	<div class="folder-modal">
		<Input label="Name" bind:value={newFolderName} placeholder="Prod servers" />
		<Input label="Icon (emoji)" bind:value={newFolderIcon} placeholder="📁" />
		<label class="color-label">
			<span>Color</span>
			<input type="color" bind:value={newFolderColor} />
		</label>
		<div class="modal-actions">
			<Button variant="ghost" onclick={() => (showFolderModal = false)}>Cancel</Button>
			<Button variant="primary" onclick={saveFolder}>Save</Button>
		</div>
	</div>
</Modal>

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

	.divider {
		height: 1px;
		background-color: var(--color-border);
		opacity: 0.5;
		margin: 2px 0;
	}

	.sessions-scroll {
		display: flex;
		flex-direction: column;
		gap: 2px;
		overflow-y: auto;
	}

	.folder-block {
		border: 1px solid rgba(255, 255, 255, 0.08);
		border-radius: 10px;
		background: linear-gradient(180deg, rgba(255,255,255,0.02), rgba(255,255,255,0.01));
		box-shadow: 0 1px 0 rgba(0,0,0,0.25);
		margin-bottom: 6px;
		transition: border-color 0.2s ease, transform 0.2s ease;
	}

	.folder-block:hover {
		border-color: rgba(255, 255, 255, 0.16);
	}

	.folder-block.ungrouped {
		background: rgba(255,255,255,0.02);
	}

	.folder-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 8px 10px;
		cursor: grab;
		border-bottom: 1px solid rgba(255, 255, 255, 0.06);
	}

	.folder-title {
		display: flex;
		align-items: center;
		gap: 8px;
		font-size: 0.75rem;
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.folder-icon {
		width: 18px;
		text-align: center;
	}

	.folder-name {
		max-width: 170px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.folder-count {
		padding: 2px 6px;
		font-size: 0.625rem;
		background: rgba(255,255,255,0.06);
		border-radius: 999px;
		color: var(--color-text-secondary);
	}

	.folder-actions {
		display: flex;
		gap: 4px;
	}

	.folder-btn {
		background: rgba(255,255,255,0.04);
		border: 1px solid rgba(255,255,255,0.08);
		border-radius: 6px;
		padding: 2px 6px;
		font-size: 0.65rem;
		color: var(--color-text-secondary);
		cursor: pointer;
	}

	.folder-btn:hover {
		color: var(--color-text-primary);
	}

	.folder-content {
		padding: 8px;
		border-top: 2px solid rgba(255,255,255,0.02);
		background: rgba(0,0,0,0.12);
	}

	.folder-content:empty {
		padding: 6px;
	}

	.folder-block {
		border-left: 3px solid var(--folder-color, #3b82f6);
	}

	.folder-empty {
		padding: 8px;
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
		opacity: 0.7;
	}

	.session-card-wrapper {
		margin-bottom: 6px;
		transition: transform 0.15s ease;
	}

	.session-card-wrapper:active {
		transform: scale(0.995);
	}

	.folder-modal {
		display: flex;
		flex-direction: column;
		gap: 10px;
	}

	.color-label {
		display: flex;
		align-items: center;
		justify-content: space-between;
		font-size: 0.75rem;
		color: var(--color-text-secondary);
	}

	.color-label input[type='color'] {
		width: 44px;
		height: 28px;
		border: none;
		background: transparent;
		padding: 0;
	}

	.modal-actions {
		display: flex;
		justify-content: flex-end;
		gap: 8px;
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

	.empty-state {
		margin: 0;
		padding: 12px 4px;
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
