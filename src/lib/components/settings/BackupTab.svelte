<script lang="ts">
	import Button from '$lib/components/shared/Button.svelte';
	import Input from '$lib/components/shared/Input.svelte';
	import { exportBackup, previewBackup, importBackup } from '$lib/state/vault.svelte';
	import { addToast } from '$lib/state/toasts.svelte';
	import { getSettings, syncTraySettings } from '$lib/state/settings.svelte';
	import * as vaultIpc from '$lib/ipc/vault';
	import { save, open, message } from '@tauri-apps/plugin-dialog';
	import { relaunch } from '@tauri-apps/plugin-process';
	import type { BackupPreview } from '$lib/ipc/vault';

	// Export state
	let exportPassword = $state('');
	let exportConfirmPassword = $state('');
	let exporting = $state(false);
	let exportError = $state('');

	// Import state
	let importFilePath = $state('');
	let importExportPassword = $state('');
	let importMasterPassword = $state('');
	let importing = $state(false);
	let importError = $state('');
	let preview: BackupPreview | null = $state(null);
	let verifying = $state(false);

	let canExport = $derived(
		exportPassword.length >= 8 &&
		exportPassword === exportConfirmPassword &&
		!exporting
	);

	let canVerify = $derived(
		importFilePath.length > 0 &&
		importExportPassword.length >= 8 &&
		!verifying
	);

	let canImport = $derived(
		preview !== null &&
		!importing
	);

	function formatDate(timestamp: number): string {
		return new Date(timestamp * 1000).toLocaleString();
	}

	async function handleExport() {
		exportError = '';

		if (exportPassword.length < 8) {
			exportError = 'Password must be at least 8 characters';
			return;
		}

		if (exportPassword !== exportConfirmPassword) {
			exportError = 'Passwords do not match';
			return;
		}

		exporting = true;

		try {
			// Save local settings into vault AppSettings so they're included in backup
			const localSettings = getSettings();
			const currentAppSettings = await vaultIpc.getSettings();
			await vaultIpc.saveSettings({
				...currentAppSettings,
				minimizeToTray: localSettings.minimizeToTray,
				startWithSystem: localSettings.startWithSystem,
				defaultShell: localSettings.defaultShell,
				openLastSession: localSettings.openLastSession,
				fontSize: localSettings.fontSize,
				fontFamily: localSettings.fontFamily,
				locale: localSettings.locale
			});

			const filePath = await save({
				filters: [{ name: 'Reach Backup', extensions: ['reachbackup'] }],
				defaultPath: 'reach-backup.reachbackup'
			});

			if (!filePath) {
				exporting = false;
				return;
			}

			await exportBackup(exportPassword, filePath);
			addToast('Backup exported successfully', 'success');
			exportPassword = '';
			exportConfirmPassword = '';
		} catch (e) {
			exportError = e instanceof Error ? e.message : 'Failed to export backup';
			addToast('Export failed', 'error');
		} finally {
			exporting = false;
		}
	}

	async function handleSelectFile() {
		importError = '';
		preview = null;

		try {
			const selected = await open({
				filters: [{ name: 'Reach Backup', extensions: ['reachbackup'] }],
				multiple: false
			});

			if (selected) {
				importFilePath = selected as string;
			}
		} catch (e) {
			importError = e instanceof Error ? e.message : 'Failed to open file dialog';
		}
	}

	async function handleVerify() {
		importError = '';
		preview = null;
		verifying = true;

		try {
			preview = await previewBackup(importFilePath, importExportPassword);
			addToast('Backup verified successfully', 'success');
		} catch (e) {
			importError = e instanceof Error ? e.message : 'Failed to verify backup';
		} finally {
			verifying = false;
		}
	}

	async function handleImport() {
		if (!preview) return;

		importError = '';
		importing = true;

		try {
			await importBackup(importFilePath, importExportPassword, importMasterPassword);
			addToast('Backup imported successfully — restarting app', 'success');
			await message('Backup restored successfully. The app will now restart to apply all settings.', { title: 'Import Complete', kind: 'info' });
			await relaunch();
		} catch (e) {
			importError = e instanceof Error ? e.message : 'Failed to import backup';
			addToast('Import failed', 'error');
		} finally {
			importing = false;
		}
	}
</script>

<div class="tab-content">
	<!-- Export Section -->
	<div class="section">
		<h3 class="section-title">Export Backup</h3>
		<p class="section-desc">
			Create an encrypted backup of your entire Reach state — identity, vaults, sessions, credentials, and settings.
		</p>

		<div class="form-field">
			<Input
				label="Export Password"
				type="password"
				placeholder="Minimum 8 characters"
				bind:value={exportPassword}
				disabled={exporting}
			/>
		</div>

		<div class="form-field">
			<Input
				label="Confirm Password"
				type="password"
				placeholder="Re-enter password"
				bind:value={exportConfirmPassword}
				disabled={exporting}
			/>
		</div>

		{#if exportError}
			<div class="form-error">{exportError}</div>
		{/if}

		<div class="action-row">
			<Button variant="primary" size="sm" onclick={handleExport} disabled={!canExport}>
				{exporting ? 'Exporting...' : 'Export Backup'}
			</Button>
		</div>
	</div>

	<!-- Import Section -->
	<div class="section">
		<h3 class="section-title">Import Backup</h3>
		<p class="section-desc">
			Restore from a previously exported backup file. This will replace your current data.
		</p>

		<div class="action-row">
			<Button variant="secondary" size="sm" onclick={handleSelectFile}>
				Select Backup File
			</Button>
		</div>

		{#if importFilePath}
			<div class="setting-row">
				<div class="setting-info">
					<span class="setting-label">Selected File</span>
					<span class="setting-value mono truncate">{importFilePath}</span>
				</div>
			</div>

			<div class="form-field">
				<Input
					label="Export Password"
					type="password"
					placeholder="Password used during export"
					bind:value={importExportPassword}
					disabled={verifying || importing}
				/>
			</div>

			<div class="action-row">
				<Button variant="secondary" size="sm" onclick={handleVerify} disabled={!canVerify}>
					{verifying ? 'Verifying...' : 'Verify'}
				</Button>
			</div>
		{/if}

		{#if preview}
			<div class="preview-box">
				<h4 class="preview-title">Backup Preview</h4>

				<div class="setting-row">
					<div class="setting-info">
						<span class="setting-label">Exported</span>
						<span class="setting-description">{formatDate(preview.exportedAt)}</span>
					</div>
				</div>

				<div class="setting-row">
					<div class="setting-info">
						<span class="setting-label">Vaults</span>
						<span class="setting-description">{preview.vaultCount}</span>
					</div>
				</div>

				<div class="setting-row">
					<div class="setting-info">
						<span class="setting-label">Secrets</span>
						<span class="setting-description">{preview.secretCount}</span>
					</div>
				</div>

				<div class="setting-row">
					<div class="setting-info">
						<span class="setting-label">Sync Config</span>
						<span class="setting-description">{preview.hasSyncConfig ? 'Included' : 'None'}</span>
					</div>
					<div class="status-badge" class:enabled={preview.hasSyncConfig}>
						{preview.hasSyncConfig ? 'Yes' : 'No'}
					</div>
				</div>
			</div>

			<div class="form-field">
				<Input
					label="Master Password (optional)"
					type="password"
					placeholder="Leave blank if you used TLS-style init"
					bind:value={importMasterPassword}
					disabled={importing}
				/>
			</div>

			<div class="import-warning">
				This will reset your current data and restore from the backup.
			</div>

			{#if importError}
				<div class="form-error">{importError}</div>
			{/if}

			<div class="action-row">
				<Button variant="danger" size="sm" onclick={handleImport} disabled={!canImport}>
					{importing ? 'Importing...' : 'Import Backup'}
				</Button>
			</div>
		{:else if importError}
			<div class="form-error">{importError}</div>
		{/if}
	</div>
</div>

<style>
	.tab-content {
		display: flex;
		flex-direction: column;
		gap: 16px;
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
		max-width: 300px;
	}

	.setting-description {
		font-size: 0.75rem;
		color: var(--color-text-secondary);
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

	.preview-box {
		display: flex;
		flex-direction: column;
		gap: 4px;
		padding: 12px;
		background: var(--color-bg-secondary);
		border: 1px solid var(--color-border);
		border-radius: 8px;
	}

	.preview-title {
		margin: 0 0 4px 0;
		font-size: 0.8125rem;
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.preview-box .setting-row:last-child {
		border-bottom: none;
	}

	.import-warning {
		font-size: 0.75rem;
		font-weight: 500;
		color: var(--color-warning);
		padding: 8px 12px;
		background: rgba(255, 214, 10, 0.08);
		border: 1px solid rgba(255, 214, 10, 0.3);
		border-radius: 8px;
	}
</style>
