<script lang="ts">
	import Button from '$lib/components/shared/Button.svelte';
	import Input from '$lib/components/shared/Input.svelte';
	import {
		hasMasterPassword,
		setMasterPassword,
		lock as lockCredentials,
		isLocked as checkIsLocked
	} from '$lib/ipc/credentials';

	let hasPassword = $state(false);
	let locked = $state(false);
	let loading = $state(true);

	let showPasswordForm = $state(false);
	let newPassword = $state('');
	let confirmPassword = $state('');
	let error = $state('');
	let saving = $state(false);

	$effect(() => {
		loadStatus();
	});

	async function loadStatus() {
		loading = true;
		try {
			hasPassword = await hasMasterPassword();
			locked = await checkIsLocked();
		} catch {
			// IPC not available in dev, set safe defaults
			hasPassword = false;
			locked = false;
		}
		loading = false;
	}

	function openPasswordForm() {
		showPasswordForm = true;
		newPassword = '';
		confirmPassword = '';
		error = '';
	}

	function cancelPasswordForm() {
		showPasswordForm = false;
		newPassword = '';
		confirmPassword = '';
		error = '';
	}

	async function savePassword() {
		error = '';

		if (newPassword.length < 8) {
			error = 'Password must be at least 8 characters';
			return;
		}

		if (newPassword !== confirmPassword) {
			error = "Passwords don't match";
			return;
		}

		saving = true;
		try {
			await setMasterPassword(newPassword);
			hasPassword = true;
			showPasswordForm = false;
			newPassword = '';
			confirmPassword = '';
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to set password';
		}
		saving = false;
	}

	async function handleLock() {
		try {
			await lockCredentials();
			locked = true;
		} catch {
			// Lock failed
		}
	}
</script>

<div class="tab-content">
	<div class="setting-row">
		<div class="setting-info">
			<span class="setting-label">Master Password Status</span>
			<span class="setting-description">
				{#if loading}
					Checking...
				{:else if hasPassword}
					Password is set
				{:else}
					No password configured
				{/if}
			</span>
		</div>
		<div class="setting-control">
			<div class="status-badge" class:set={hasPassword && !loading}>
				{#if loading}
					...
				{:else if hasPassword}
					<svg width="14" height="14" viewBox="0 0 14 14" fill="none">
						<path d="M2 7L5.5 10.5L12 3.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
					</svg>
					Set
				{:else}
					<svg width="14" height="14" viewBox="0 0 14 14" fill="none">
						<path d="M1 1L13 13M13 1L1 13" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" />
					</svg>
					Not set
				{/if}
			</div>
		</div>
	</div>

	<div class="setting-row">
		<div class="setting-info">
			<span class="setting-label">Lock Status</span>
			<span class="setting-description">
				{#if locked}
					Credentials are locked and encrypted
				{:else}
					Credentials are currently accessible
				{/if}
			</span>
		</div>
		<div class="setting-control">
			<div class="lock-badge" class:locked>
				{#if locked}
					<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
						<rect x="3" y="11" width="18" height="11" rx="2" ry="2" />
						<path d="M7 11V7a5 5 0 0 1 10 0v4" />
					</svg>
					Locked
				{:else}
					<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
						<rect x="3" y="11" width="18" height="11" rx="2" ry="2" />
						<path d="M7 11V7a5 5 0 0 1 9.9-1" />
					</svg>
					Unlocked
				{/if}
			</div>
		</div>
	</div>

	<div class="action-row">
		{#if !showPasswordForm}
			<Button
				variant="secondary"
				size="sm"
				onclick={openPasswordForm}
			>
				{hasPassword ? 'Change Password' : 'Set Password'}
			</Button>

			{#if hasPassword && !locked}
				<Button
					variant="danger"
					size="sm"
					onclick={handleLock}
				>
					Lock Now
				</Button>
			{/if}
		{/if}
	</div>

	{#if showPasswordForm}
		<div class="password-form">
			<div class="form-field">
				<Input
					label="New Password"
					type="password"
					placeholder="Enter new password"
					bind:value={newPassword}
				/>
			</div>

			<div class="form-field">
				<Input
					label="Confirm Password"
					type="password"
					placeholder="Re-enter password"
					bind:value={confirmPassword}
				/>
			</div>

			{#if error}
				<div class="form-error">{error}</div>
			{/if}

			<div class="form-actions">
				<Button
					variant="ghost"
					size="sm"
					onclick={cancelPasswordForm}
				>
					Cancel
				</Button>
				<Button
					variant="primary"
					size="sm"
					disabled={saving || newPassword.length === 0}
					onclick={savePassword}
				>
					{saving ? 'Saving...' : 'Save Password'}
				</Button>
			</div>
		</div>
	{/if}
</div>

<style>
	.tab-content {
		display: flex;
		flex-direction: column;
	}

	.setting-row {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 12px 0;
		border-bottom: 1px solid var(--color-border);
		gap: 24px;
	}

	.setting-info {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.setting-label {
		font-size: 0.875rem;
		font-weight: 500;
		color: var(--color-text-primary);
	}

	.setting-description {
		font-size: 0.75rem;
		color: var(--color-text-secondary);
	}

	.setting-control {
		flex-shrink: 0;
	}

	.status-badge {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		padding: 4px 10px;
		border-radius: 6px;
		font-size: 0.75rem;
		font-weight: 500;
		background-color: rgba(255, 69, 58, 0.12);
		color: var(--color-danger);
	}

	.status-badge.set {
		background-color: rgba(48, 209, 88, 0.12);
		color: var(--color-success);
	}

	.lock-badge {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		padding: 4px 10px;
		border-radius: 6px;
		font-size: 0.75rem;
		font-weight: 500;
		background-color: rgba(255, 214, 10, 0.12);
		color: var(--color-warning);
	}

	.lock-badge.locked {
		background-color: rgba(48, 209, 88, 0.12);
		color: var(--color-success);
	}

	.action-row {
		display: flex;
		gap: 8px;
		padding: 16px 0 8px;
	}

	.password-form {
		display: flex;
		flex-direction: column;
		gap: 12px;
		padding: 16px;
		margin-top: 8px;
		background-color: var(--color-bg-secondary);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-card);
	}

	.form-field {
		width: 100%;
	}

	.form-error {
		font-size: 0.75rem;
		color: var(--color-danger);
		padding: 4px 0;
	}

	.form-actions {
		display: flex;
		justify-content: flex-end;
		gap: 8px;
		padding-top: 4px;
	}
</style>
