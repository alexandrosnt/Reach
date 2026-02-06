<script lang="ts">
	import Button from '$lib/components/shared/Button.svelte';
	import Input from '$lib/components/shared/Input.svelte';
	import Modal from '$lib/components/shared/Modal.svelte';
	import { inviteMember, listMembers, removeMember, type MemberInfo, type InviteInfo } from '$lib/ipc/vault';
	import { vaultState } from '$lib/state/vault.svelte';
	import { addToast } from '$lib/state/toasts.svelte';

	interface Props {
		open: boolean;
		vaultId: string;
		vaultName: string;
		onclose: () => void;
	}

	let { open, vaultId, vaultName, onclose }: Props = $props();

	// Tab state
	let activeTab = $state<'invite' | 'members'>('invite');

	// Invite form state
	let inviteeUuid = $state('');
	let inviteePublicKey = $state('');
	let inviteeRole = $state<'admin' | 'member' | 'readonly'>('member');
	let inviting = $state(false);
	let error = $state('');

	// Invite result
	let inviteResult = $state<InviteInfo | null>(null);

	// Members list
	let members = $state<MemberInfo[]>([]);
	let loadingMembers = $state(false);

	// Current user info for reference
	let userUuid = $derived(vaultState.userUuid);
	let publicKey = $derived(vaultState.publicKey);

	$effect(() => {
		if (open && activeTab === 'members') {
			loadMembers();
		}
	});

	$effect(() => {
		if (!open) {
			// Reset state when closing
			inviteeUuid = '';
			inviteePublicKey = '';
			inviteeRole = 'member';
			error = '';
			inviteResult = null;
		}
	});

	async function loadMembers() {
		loadingMembers = true;
		try {
			members = await listMembers(vaultId);
		} catch (e) {
			addToast(`Failed to load members: ${e}`, 'error');
		} finally {
			loadingMembers = false;
		}
	}

	async function handleInvite() {
		if (!inviteeUuid.trim()) {
			error = 'Recipient UUID is required';
			return;
		}
		if (!inviteePublicKey.trim()) {
			error = 'Recipient public key is required';
			return;
		}

		inviting = true;
		error = '';

		try {
			const result = await inviteMember(vaultId, inviteePublicKey.trim(), inviteeUuid.trim(), inviteeRole);
			inviteResult = result;
			addToast('Member invited successfully', 'success');
			// Refresh members list
			await loadMembers();
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			inviting = false;
		}
	}

	async function handleRemoveMember(memberUuid: string) {
		if (memberUuid === userUuid) {
			addToast('Cannot remove yourself', 'error');
			return;
		}

		try {
			await removeMember(vaultId, memberUuid);
			addToast('Member removed', 'success');
			await loadMembers();
		} catch (e) {
			addToast(`Failed to remove member: ${e}`, 'error');
		}
	}

	function copyToClipboard(text: string, label: string) {
		navigator.clipboard.writeText(text);
		addToast(`${label} copied to clipboard`, 'success');
	}

	function copyInviteInfo() {
		if (!inviteResult) return;
		const info = `Vault Invite for "${vaultName}"

Sync URL: ${inviteResult.syncUrl}
Token: ${inviteResult.token}

To accept this invite, go to Settings > Vault > Accept Invite and paste the sync URL and token.`;
		navigator.clipboard.writeText(info);
		addToast('Invite info copied to clipboard', 'success');
	}

	function formatRole(role: string): string {
		switch (role.toLowerCase()) {
			case 'owner': return 'Owner';
			case 'admin': return 'Admin';
			case 'member': return 'Member';
			case 'readonly': return 'Read Only';
			default: return role;
		}
	}
</script>

<Modal {open} {onclose} title="Manage Members - {vaultName}">
	<div class="dialog-content">
		<!-- Tabs -->
		<div class="tabs">
			<button
				class="tab"
				class:active={activeTab === 'invite'}
				onclick={() => (activeTab = 'invite')}
			>
				Invite
			</button>
			<button
				class="tab"
				class:active={activeTab === 'members'}
				onclick={() => { activeTab = 'members'; loadMembers(); }}
			>
				Members
			</button>
		</div>

		{#if activeTab === 'invite'}
			<!-- Invite Tab -->
			<div class="tab-content">
				{#if inviteResult}
					<!-- Show invite result -->
					<div class="invite-result">
						<div class="result-header">
							<svg width="20" height="20" viewBox="0 0 24 24" fill="none">
								<path d="M22 11.08V12a10 10 0 11-5.93-9.14" stroke="var(--color-success)" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
								<polyline points="22,4 12,14.01 9,11.01" stroke="var(--color-success)" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
							</svg>
							<span>Invite Created!</span>
						</div>
						<p class="result-description">
							Share the following information with the invitee. They'll need to accept the invite in Settings.
						</p>

						<div class="info-field">
							<span class="info-label">Sync URL</span>
							<div class="info-value-row">
								<code class="info-value">{inviteResult.syncUrl}</code>
								<button class="copy-btn" onclick={() => copyToClipboard(inviteResult!.syncUrl, 'Sync URL')}>Copy</button>
							</div>
						</div>

						<div class="info-field">
							<span class="info-label">Token</span>
							<div class="info-value-row">
								<code class="info-value truncate">{inviteResult.token}</code>
								<button class="copy-btn" onclick={() => copyToClipboard(inviteResult!.token, 'Token')}>Copy</button>
							</div>
						</div>

						<div class="result-actions">
							<Button variant="primary" size="sm" onclick={copyInviteInfo}>
								Copy All Invite Info
							</Button>
							<Button variant="ghost" size="sm" onclick={() => { inviteResult = null; inviteeUuid = ''; inviteePublicKey = ''; }}>
								Invite Another
							</Button>
						</div>
					</div>
				{:else}
					<!-- Show invite form -->
					<div class="section">
						<h4 class="section-title">Your Identity (Share with invitee)</h4>
						<div class="info-field">
							<span class="info-label">Your UUID</span>
							<div class="info-value-row">
								<code class="info-value">{userUuid ?? 'Not available'}</code>
								{#if userUuid}
									<button class="copy-btn" onclick={() => copyToClipboard(userUuid!, 'UUID')}>Copy</button>
								{/if}
							</div>
						</div>
						<div class="info-field">
							<span class="info-label">Your Public Key</span>
							<div class="info-value-row">
								<code class="info-value truncate">{publicKey ?? 'Not available'}</code>
								{#if publicKey}
									<button class="copy-btn" onclick={() => copyToClipboard(publicKey!, 'Public Key')}>Copy</button>
								{/if}
							</div>
						</div>
					</div>

					<div class="divider"></div>

					<div class="section">
						<h4 class="section-title">Invitee Information (Get from them)</h4>
						<div class="form-field">
							<Input
								label="Invitee UUID"
								placeholder="xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
								bind:value={inviteeUuid}
								disabled={inviting}
							/>
						</div>

						<div class="form-field">
							<Input
								label="Invitee Public Key"
								placeholder="Base64 encoded public key"
								bind:value={inviteePublicKey}
								disabled={inviting}
							/>
						</div>

						<div class="form-field">
							<span class="field-label">Role</span>
							<div class="role-options">
								<button
									class="role-option"
									class:active={inviteeRole === 'admin'}
									onclick={() => (inviteeRole = 'admin')}
									disabled={inviting}
								>
									<span class="role-name">Admin</span>
									<span class="role-desc">Can invite/remove members</span>
								</button>
								<button
									class="role-option"
									class:active={inviteeRole === 'member'}
									onclick={() => (inviteeRole = 'member')}
									disabled={inviting}
								>
									<span class="role-name">Member</span>
									<span class="role-desc">Read & write secrets</span>
								</button>
								<button
									class="role-option"
									class:active={inviteeRole === 'readonly'}
									onclick={() => (inviteeRole = 'readonly')}
									disabled={inviting}
								>
									<span class="role-name">Read Only</span>
									<span class="role-desc">View secrets only</span>
								</button>
							</div>
						</div>
					</div>

					{#if error}
						<div class="error-message">{error}</div>
					{/if}
				{/if}
			</div>
		{:else}
			<!-- Members Tab -->
			<div class="tab-content">
				{#if loadingMembers}
					<div class="loading">Loading members...</div>
				{:else if members.length === 0}
					<div class="empty-state">
						<p>No members yet. Invite someone to share this vault.</p>
					</div>
				{:else}
					<div class="members-list">
						{#each members as member (member.userUuid)}
							<div class="member-card">
								<div class="member-info">
									<span class="member-uuid">{member.userUuid}</span>
									<div class="member-meta">
										<span class="member-role">{formatRole(member.role)}</span>
										{#if member.userUuid === userUuid}
											<span class="member-you">(You)</span>
										{/if}
									</div>
								</div>
								{#if member.userUuid !== userUuid && member.role !== 'owner'}
									<button
										class="remove-btn"
										onclick={() => handleRemoveMember(member.userUuid)}
										title="Remove member"
									>
										<svg width="14" height="14" viewBox="0 0 24 24" fill="none">
											<path d="M18 6L6 18M6 6l12 12" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
										</svg>
									</button>
								{/if}
							</div>
						{/each}
					</div>
				{/if}
			</div>
		{/if}
	</div>

	{#snippet actions()}
		{#if activeTab === 'invite' && !inviteResult}
			<Button variant="ghost" onclick={onclose} disabled={inviting}>
				Cancel
			</Button>
			<Button
				variant="primary"
				onclick={handleInvite}
				disabled={inviting || !inviteeUuid.trim() || !inviteePublicKey.trim()}
			>
				{#if inviting}Inviting...{:else}Send Invite{/if}
			</Button>
		{:else}
			<Button variant="ghost" onclick={onclose}>
				Close
			</Button>
		{/if}
	{/snippet}
</Modal>

<style>
	.dialog-content {
		display: flex;
		flex-direction: column;
		gap: 16px;
		min-width: 380px;
	}

	.tabs {
		display: flex;
		gap: 4px;
		padding: 4px;
		background-color: rgba(255, 255, 255, 0.04);
		border-radius: 8px;
	}

	.tab {
		flex: 1;
		padding: 8px 16px;
		font-family: var(--font-sans);
		font-size: 0.75rem;
		font-weight: 500;
		color: var(--color-text-secondary);
		background: transparent;
		border: none;
		border-radius: 6px;
		cursor: pointer;
		transition: all var(--duration-default) var(--ease-default);
	}

	.tab:hover {
		color: var(--color-text-primary);
	}

	.tab.active {
		color: var(--color-text-primary);
		background-color: rgba(255, 255, 255, 0.08);
	}

	.tab-content {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.section {
		display: flex;
		flex-direction: column;
		gap: 10px;
	}

	.section-title {
		margin: 0;
		font-size: 0.6875rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--color-text-secondary);
	}

	.divider {
		height: 1px;
		background-color: var(--color-border);
		margin: 4px 0;
	}

	.info-field {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.info-label {
		font-size: 0.6875rem;
		font-weight: 500;
		color: var(--color-text-secondary);
	}

	.info-value-row {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.info-value {
		flex: 1;
		padding: 6px 8px;
		font-family: var(--font-mono);
		font-size: 0.6875rem;
		color: var(--color-text-primary);
		background-color: rgba(255, 255, 255, 0.04);
		border-radius: 4px;
		word-break: break-all;
	}

	.info-value.truncate {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.copy-btn {
		padding: 4px 8px;
		font-family: var(--font-sans);
		font-size: 0.625rem;
		font-weight: 500;
		color: var(--color-accent);
		background: transparent;
		border: 1px solid var(--color-accent);
		border-radius: 4px;
		cursor: pointer;
		flex-shrink: 0;
		transition: all var(--duration-default) var(--ease-default);
	}

	.copy-btn:hover {
		background-color: rgba(10, 132, 255, 0.1);
	}

	.form-field {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.field-label {
		font-size: 0.75rem;
		font-weight: 500;
		color: var(--color-text-secondary);
	}

	.role-options {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.role-option {
		display: flex;
		flex-direction: column;
		gap: 2px;
		padding: 10px 12px;
		text-align: left;
		background-color: rgba(255, 255, 255, 0.02);
		border: 1px solid var(--color-border);
		border-radius: 6px;
		cursor: pointer;
		transition: all var(--duration-default) var(--ease-default);
	}

	.role-option:hover:not(:disabled) {
		background-color: rgba(255, 255, 255, 0.05);
		border-color: var(--color-text-secondary);
	}

	.role-option.active {
		background-color: rgba(10, 132, 255, 0.08);
		border-color: var(--color-accent);
	}

	.role-option:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.role-name {
		font-size: 0.75rem;
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.role-desc {
		font-size: 0.625rem;
		color: var(--color-text-secondary);
	}

	.error-message {
		padding: 8px 10px;
		font-size: 0.6875rem;
		color: var(--color-danger);
		background-color: rgba(255, 69, 58, 0.08);
		border-radius: 6px;
	}

	/* Invite Result */
	.invite-result {
		display: flex;
		flex-direction: column;
		gap: 12px;
		padding: 16px;
		background-color: rgba(48, 209, 88, 0.06);
		border: 1px solid rgba(48, 209, 88, 0.2);
		border-radius: 8px;
	}

	.result-header {
		display: flex;
		align-items: center;
		gap: 8px;
		font-size: 0.875rem;
		font-weight: 600;
		color: var(--color-success);
	}

	.result-description {
		margin: 0;
		font-size: 0.75rem;
		color: var(--color-text-secondary);
		line-height: 1.5;
	}

	.result-actions {
		display: flex;
		gap: 8px;
		margin-top: 8px;
	}

	/* Members List */
	.loading {
		padding: 20px;
		text-align: center;
		color: var(--color-text-secondary);
		font-size: 0.75rem;
	}

	.empty-state {
		padding: 20px;
		text-align: center;
	}

	.empty-state p {
		margin: 0;
		font-size: 0.75rem;
		color: var(--color-text-secondary);
	}

	.members-list {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.member-card {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 10px 12px;
		background-color: rgba(255, 255, 255, 0.02);
		border: 1px solid var(--color-border);
		border-radius: 6px;
	}

	.member-info {
		display: flex;
		flex-direction: column;
		gap: 4px;
		overflow: hidden;
	}

	.member-uuid {
		font-family: var(--font-mono);
		font-size: 0.6875rem;
		color: var(--color-text-primary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.member-meta {
		display: flex;
		align-items: center;
		gap: 6px;
	}

	.member-role {
		font-size: 0.625rem;
		font-weight: 500;
		color: var(--color-text-secondary);
	}

	.member-you {
		font-size: 0.625rem;
		color: var(--color-accent);
	}

	.remove-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 24px;
		height: 24px;
		padding: 0;
		border: none;
		border-radius: 4px;
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
		transition: all var(--duration-default) var(--ease-default);
	}

	.remove-btn:hover {
		background-color: rgba(255, 69, 58, 0.1);
		color: var(--color-danger);
	}
</style>
