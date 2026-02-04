<script lang="ts">
	import QuickConnect from './QuickConnect.svelte';
	import SessionEditor from './SessionEditor.svelte';
	import SessionCard from './SessionCard.svelte';
	import { sessionList, sessionDelete, sessionUpdate, type SessionConfig } from '$lib/ipc/sessions';
	import { sshConnect, sshDetectOs } from '$lib/ipc/ssh';
	import { getCachedPassword, setCachedPassword, hasCachedPassword } from '$lib/state/passwords.svelte';
	import { createTab } from '$lib/state/tabs.svelte';
	import { addToast } from '$lib/state/toasts.svelte';
	import { untrack } from 'svelte';

	let showQuickConnect = $state(false);
	let showEditor = $state(false);
	let editingSession = $state<SessionConfig | undefined>();
	let sessions = $state<SessionConfig[]>([]);
	let loading = $state(true);
	let deleteConfirm = $state<string | null>(null);

	// Connect prompt state
	let connectSession = $state<SessionConfig | undefined>();
	let connectPassword = $state('');
	let connectKeyPassphrase = $state('');
	let connecting = $state(false);
	let connectError = $state<string | undefined>();
	let rememberPassword = $state(false);
	let hasSavedPassword = $state(false);

	let showConnectPrompt = $derived(!!connectSession);

	async function loadSessions(): Promise<void> {
		try {
			sessions = await sessionList();
		} catch (err) {
			console.error('Failed to load sessions:', err);
		} finally {
			loading = false;
		}
	}

	async function handleConnect(session: SessionConfig): Promise<void> {
		const saved = getCachedPassword(session.id);

		// Auto-connect if we have a saved password
		if (saved) {
			connectSession = session;
			connectError = undefined;
			rememberPassword = true;
			hasSavedPassword = true;

			if (session.auth_method.type === 'Password') {
				connectPassword = saved;
			} else if (session.auth_method.type === 'Key') {
				connectKeyPassphrase = saved;
			}

			await doConnect();
			return;
		}

		// Otherwise show the prompt
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
		const authType = session.auth_method.type;

		const passwordToSave = authType === 'Password' ? connectPassword : connectKeyPassphrase;

		try {
			await sshConnect({
				id,
				host: session.host,
				port: session.port,
				username: session.username,
				authMethod: authType === 'Key' ? 'key' : authType.toLowerCase(),
				password: authType === 'Password' ? connectPassword : undefined,
				keyPath: authType === 'Key' ? session.auth_method.path : undefined,
				keyPassphrase: authType === 'Key' && connectKeyPassphrase ? connectKeyPassphrase : undefined,
				cols: 80,
				rows: 24,
			});

			// Cache password for future connections in this session
			if (rememberPassword && passwordToSave) {
				setCachedPassword(session.id, passwordToSave);
			}

			createTab('ssh', `${session.username}@${session.host}`, id);
			addToast(`Connected to ${session.name}`, 'success');
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
		if (!connecting) {
			connectSession = undefined;
		}
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

	// Load sessions on mount
	$effect(() => {
		untrack(() => loadSessions());
	});
</script>

<div class="session-list">
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
			Quick Connect
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
			Save Session
		</button>
	</div>

	{#if loading}
		<div class="loading-state">
			<span class="spinner"></span>
			<span class="loading-text">Loading sessions...</span>
		</div>
	{:else if sessions.length === 0}
		<p class="empty-state">No saved sessions yet. Create one to get started.</p>
	{:else}
		<div class="divider"></div>
		<div class="sessions-scroll">
			{#each sessions as session (session.id)}
				{#if deleteConfirm === session.id}
					<div class="delete-confirm">
						<span class="delete-confirm-text">Delete "{session.name}"?</span>
						<button class="delete-confirm-btn" onclick={() => handleDelete(session)}>
							Confirm
						</button>
						<button class="delete-cancel-btn" onclick={() => (deleteConfirm = null)}>
							Cancel
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
			{/each}
		</div>
	{/if}
</div>

<QuickConnect bind:open={showQuickConnect} />
<SessionEditor bind:open={showEditor} editSession={editingSession} onsave={handleEditorSave} />

{#if showConnectPrompt && connectSession}
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div class="prompt-overlay" onkeydown={(e) => { if (e.key === 'Escape') cancelConnect(); }} onclick={cancelConnect}>
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div class="prompt-box" onclick={(e) => e.stopPropagation()} onkeydown={() => {}}>
			<div class="prompt-header">
				<span class="prompt-title">Connect to {connectSession.name}</span>
				<span class="prompt-detail">{connectSession.username}@{connectSession.host}:{connectSession.port}</span>
			</div>

			<form class="prompt-form" onsubmit={(e) => { e.preventDefault(); doConnect(); }}>
				{#if connectSession.auth_method.type === 'Password'}
					<input
						class="prompt-input"
						type="password"
						placeholder="Password"
						bind:value={connectPassword}
						disabled={connecting}
					/>
				{:else if connectSession.auth_method.type === 'Key'}
					<input
						class="prompt-input"
						type="password"
						placeholder="Key passphrase (optional)"
						bind:value={connectKeyPassphrase}
						disabled={connecting}
					/>
				{/if}

				{#if connectSession.auth_method.type !== 'Agent'}
					<label class="remember-label">
						<input type="checkbox" class="remember-check" bind:checked={rememberPassword} disabled={connecting} />
						<span class="remember-text">{hasSavedPassword ? 'Password saved' : 'Remember password'}</span>
					</label>
				{/if}

				{#if connectError}
					<div class="prompt-error">{connectError}</div>
				{/if}

				<div class="prompt-actions">
					<button type="button" class="prompt-btn prompt-cancel" onclick={cancelConnect} disabled={connecting}>Cancel</button>
					<button type="submit" class="prompt-btn prompt-connect" disabled={connecting}>
						{#if connecting}Connecting...{:else}Connect{/if}
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
</style>
