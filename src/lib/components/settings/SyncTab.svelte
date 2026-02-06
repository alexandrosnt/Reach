<script lang="ts">
	import Button from '$lib/components/shared/Button.svelte';
	import Input from '$lib/components/shared/Input.svelte';
	import { getSettings, saveSettings, exportIdentity } from '$lib/state/vault.svelte';
	import type { AppSettings } from '$lib/ipc/vault';
	import { createTursoDatabase, createTursoDatabaseToken, setPersonalSync, acceptInvite } from '$lib/ipc/vault';
	import { vaultState } from '$lib/state/vault.svelte';
	import { addToast } from '$lib/state/toasts.svelte';

	let loading = $state(true);
	let saving = $state(false);
	let tursoOrg = $state('');
	let tursoApiToken = $state('');
	let tursoGroup = $state('default');
	let personalDbUrl = $state('');
	let personalDbToken = $state('');
	let syncEnabled = $state(false);
	let error = $state('');

	// Identity info
	let publicKey = $derived(vaultState.publicKey);
	let userUuid = $derived(vaultState.userUuid);

	// Export key
	let showExportKey = $state(false);
	let exportedKey = $state('');

	// Accept invite
	let inviteSyncUrl = $state('');
	let inviteToken = $state('');
	let acceptingInvite = $state(false);

	$effect(() => {
		loadSettings();
	});

	async function loadSettings() {
		loading = true;
		try {
			const settings = await getSettings();
			tursoOrg = settings.tursoOrg ?? '';
			tursoApiToken = settings.tursoApiToken ?? '';
			tursoGroup = settings.tursoGroup ?? 'default';
			personalDbUrl = settings.personalDbUrl ?? '';
			personalDbToken = settings.personalDbToken ?? '';
			syncEnabled = settings.syncEnabled ?? false;
		} catch {
			// Default values
		}
		loading = false;
	}

	async function handleSave() {
		saving = true;
		error = '';

		try {
			let newPersonalDbUrl = personalDbUrl;
			let newPersonalDbToken = personalDbToken;

			// If org/token set but no personal DB yet, we need to:
			// 1. FIRST save the Turso credentials so backend can use them
			// 2. THEN create the database using those credentials
			// 3. THEN update settings with the new DB URL/token
			if (tursoOrg && tursoApiToken && !personalDbUrl) {
				// Step 1: Save Turso credentials FIRST (without DB URL)
				addToast('Saving Turso credentials...', 'info');
				const initialSettings: AppSettings = {
					tursoOrg: tursoOrg,
					tursoApiToken: tursoApiToken,
					tursoGroup: tursoGroup || 'default',
					syncEnabled: false
				};
				await saveSettings(initialSettings);

				// Step 2: Now create the database (backend will read saved credentials)
				addToast('Creating personal database...', 'info');
				const dbName = `reach-personal-${userUuid?.substring(0, 8) || Date.now()}`;
				const dbInfo = await createTursoDatabase(dbName);
				const token = await createTursoDatabaseToken(dbInfo.name);

				newPersonalDbUrl = `libsql://${dbInfo.hostname}`;
				newPersonalDbToken = token;
				personalDbUrl = newPersonalDbUrl;
				personalDbToken = token;

				addToast('Personal database created', 'success');
			}

			// Step 3: Save complete settings (with DB URL if created)
			const settings: AppSettings = {
				tursoOrg: tursoOrg || undefined,
				tursoApiToken: tursoApiToken || undefined,
				tursoGroup: tursoGroup || 'default',
				personalDbUrl: newPersonalDbUrl || undefined,
				personalDbToken: newPersonalDbToken || undefined,
				syncEnabled: !!(tursoOrg && tursoApiToken && newPersonalDbUrl)
			};
			await saveSettings(settings);
			syncEnabled = settings.syncEnabled;

			// Step 4: Set personal sync config in identity file so ALL data syncs to cloud
			if (newPersonalDbUrl && newPersonalDbToken) {
				await setPersonalSync(newPersonalDbUrl, newPersonalDbToken);
				addToast('Cloud sync enabled for all data', 'success');
			} else {
				addToast('Sync settings saved', 'success');
			}
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to save';
		}
		saving = false;
	}

	async function handleExportKey() {
		try {
			exportedKey = await exportIdentity();
			showExportKey = true;
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to export';
		}
	}

	function copyToClipboard(text: string) {
		navigator.clipboard.writeText(text);
		addToast('Copied to clipboard', 'success');
	}

	async function handleAcceptInvite() {
		if (!inviteSyncUrl.trim() || !inviteToken.trim()) {
			error = 'Both Sync URL and Token are required';
			return;
		}

		acceptingInvite = true;
		error = '';

		try {
			const vaultInfo = await acceptInvite(inviteSyncUrl.trim(), inviteToken.trim());
			addToast(`Joined vault: ${vaultInfo.name}`, 'success');
			inviteSyncUrl = '';
			inviteToken = '';
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to accept invite';
		} finally {
			acceptingInvite = false;
		}
	}
</script>

<div class="tab-content">
	{#if loading}
		<div class="loading">Loading...</div>
	{:else}
		<!-- Identity Section -->
		<div class="section">
			<h3 class="section-title">Identity</h3>
			<div class="setting-row">
				<div class="setting-info">
					<span class="setting-label">User UUID</span>
					<span class="setting-value mono">{userUuid ?? 'Not initialized'}</span>
				</div>
				{#if userUuid}
					<button class="copy-btn" onclick={() => copyToClipboard(userUuid!)}>Copy</button>
				{/if}
			</div>
			<div class="setting-row">
				<div class="setting-info">
					<span class="setting-label">Public Key</span>
					<span class="setting-value mono truncate">{publicKey ?? 'Not initialized'}</span>
				</div>
				{#if publicKey}
					<button class="copy-btn" onclick={() => copyToClipboard(publicKey!)}>Copy</button>
				{/if}
			</div>
			<div class="action-row">
				<Button variant="secondary" size="sm" onclick={handleExportKey}>
					Export Backup Key
				</Button>
			</div>
		</div>

		{#if showExportKey}
			<div class="export-box">
				<p class="export-warning">Keep this key safe! Anyone with it can decrypt your data.</p>
				<div class="export-key">
					<code>{exportedKey}</code>
					<button class="copy-btn" onclick={() => copyToClipboard(exportedKey)}>Copy</button>
				</div>
				<Button variant="ghost" size="sm" onclick={() => { showExportKey = false; exportedKey = ''; }}>
					Close
				</Button>
			</div>
		{/if}

		<!-- Turso Sync Section -->
		<div class="section">
			<h3 class="section-title">Cloud Sync</h3>
			<p class="section-desc">Sync your data to Turso for backup and multi-device access. Get credentials from <a href="https://turso.tech/app" target="_blank" rel="noopener">turso.tech</a>.</p>

			<div class="form-field">
				<Input
					label="Organization"
					placeholder="my-org"
					bind:value={tursoOrg}
				/>
			</div>

			<div class="form-field">
				<Input
					label="Platform API Token"
					type="password"
					placeholder="eyJhbG..."
					bind:value={tursoApiToken}
				/>
			</div>

			<div class="form-field">
				<Input
					label="Group"
					placeholder="default"
					bind:value={tursoGroup}
				/>
			</div>

			<div class="setting-row">
				<div class="setting-info">
					<span class="setting-label">Personal Database</span>
					<span class="setting-description">
						{#if personalDbUrl}
							{personalDbUrl}
						{:else}
							Will be created on save
						{/if}
					</span>
				</div>
				<div class="status-badge" class:enabled={!!personalDbUrl}>
					{personalDbUrl ? 'Ready' : 'Pending'}
				</div>
			</div>

			<div class="setting-row">
				<div class="setting-info">
					<span class="setting-label">Sync Status</span>
					<span class="setting-description">
						{#if syncEnabled}
							All your data syncs to cloud
						{:else}
							Data stored locally only
						{/if}
					</span>
				</div>
				<div class="status-badge" class:enabled={syncEnabled}>
					{syncEnabled ? 'Enabled' : 'Disabled'}
				</div>
			</div>

			{#if error}
				<div class="form-error">{error}</div>
			{/if}

			<div class="action-row">
				<Button variant="primary" size="sm" onclick={handleSave} disabled={saving}>
					{saving ? 'Setting up...' : 'Save & Setup'}
				</Button>
			</div>
		</div>

		<!-- Accept Invite Section -->
		<div class="section">
			<h3 class="section-title">Accept Vault Invite</h3>
			<p class="section-desc">Join a shared vault using an invite from another user.</p>

			<div class="form-field">
				<Input
					label="Sync URL"
					placeholder="libsql://vault-xxx.turso.io"
					bind:value={inviteSyncUrl}
					disabled={acceptingInvite}
				/>
			</div>

			<div class="form-field">
				<Input
					label="Token"
					type="password"
					placeholder="eyJhbG..."
					bind:value={inviteToken}
					disabled={acceptingInvite}
				/>
			</div>

			<div class="action-row">
				<Button
					variant="secondary"
					size="sm"
					onclick={handleAcceptInvite}
					disabled={acceptingInvite || !inviteSyncUrl.trim() || !inviteToken.trim()}
				>
					{acceptingInvite ? 'Joining...' : 'Accept Invite'}
				</Button>
			</div>
		</div>
	{/if}
</div>

<style>
	.tab-content {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.loading {
		padding: 20px;
		text-align: center;
		color: var(--color-text-secondary);
	}

	.section {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.section-title {
		margin: 0;
		font-size: 0.8125rem;
		font-weight: 600;
		color: var(--color-text-primary);
		text-transform: uppercase;
		letter-spacing: 0.05em;
	}

	.section-desc {
		margin: 0;
		font-size: 0.75rem;
		color: var(--color-text-secondary);
	}

	.setting-row {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 8px 0;
		border-bottom: 1px solid var(--color-border);
		gap: 16px;
	}

	.setting-info {
		display: flex;
		flex-direction: column;
		gap: 2px;
		min-width: 0;
		flex: 1;
	}

	.setting-label {
		font-size: 0.8125rem;
		font-weight: 500;
		color: var(--color-text-primary);
	}

	.setting-value {
		font-size: 0.75rem;
		color: var(--color-text-secondary);
	}

	.setting-value.mono {
		font-family: var(--font-mono);
	}

	.setting-value.truncate {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		max-width: 200px;
	}

	.setting-description {
		font-size: 0.75rem;
		color: var(--color-text-secondary);
	}

	.copy-btn {
		padding: 4px 8px;
		font-size: 0.6875rem;
		font-weight: 500;
		color: var(--color-accent);
		background: transparent;
		border: 1px solid var(--color-accent);
		border-radius: 4px;
		cursor: pointer;
	}

	.copy-btn:hover {
		background: rgba(0, 122, 255, 0.1);
	}

	.status-badge {
		padding: 4px 10px;
		font-size: 0.75rem;
		font-weight: 500;
		border-radius: 6px;
		background-color: rgba(255, 69, 58, 0.12);
		color: var(--color-danger);
	}

	.status-badge.enabled {
		background-color: rgba(48, 209, 88, 0.12);
		color: var(--color-success);
	}

	.form-field {
		width: 100%;
	}

	.form-error {
		font-size: 0.75rem;
		color: var(--color-danger);
		padding: 4px 0;
	}

	.action-row {
		display: flex;
		gap: 8px;
		padding-top: 8px;
	}

	.export-box {
		display: flex;
		flex-direction: column;
		gap: 10px;
		padding: 12px;
		background: rgba(255, 214, 10, 0.08);
		border: 1px solid rgba(255, 214, 10, 0.3);
		border-radius: 8px;
	}

	.export-warning {
		margin: 0;
		font-size: 0.75rem;
		font-weight: 500;
		color: var(--color-warning);
	}

	.export-key {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 8px;
		background: var(--color-bg-primary);
		border-radius: 4px;
		overflow: hidden;
	}

	.export-key code {
		flex: 1;
		font-size: 0.6875rem;
		font-family: var(--font-mono);
		color: var(--color-text-primary);
		word-break: break-all;
	}
</style>
