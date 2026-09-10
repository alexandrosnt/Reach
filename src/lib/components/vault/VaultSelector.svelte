<script lang="ts">
	import { onMount, tick } from 'svelte';
	import {
		vaultState,
		createVault,
		deleteVault,
		refreshVaults,
		ALL_VAULTS,
		loadVaultFilter,
		saveVaultFilter,
		type VaultFilter,
		type VaultInfo
	} from '$lib/state/vault.svelte';
	import { inviteMember, listMembers, removeMember, type MemberInfo, type InviteInfo } from '$lib/ipc/vault';
	import { addToast } from '$lib/state/toasts.svelte';
	import { t } from '$lib/state/i18n.svelte';

	interface Props {
		onvaultselect?: (filter: VaultFilter) => void;
		onrefresh?: () => void;
	}

	let { onvaultselect, onrefresh }: Props = $props();

	let showCreateDialog = $state(false);
	let newVaultName = $state('');
	let newVaultType = $state<'private' | 'shared'>('private');
	let creating = $state(false);
	let refreshing = $state(false);
	let error = $state('');

	async function handleRefresh() {
		refreshing = true;
		try {
			await refreshVaults();
			onrefresh?.();
		} catch (e) {
			addToast(`Failed to refresh: ${e}`, 'error');
		} finally {
			refreshing = false;
		}
	}

	// Delete confirmation
	let showDeleteDialog = $state(false);
	let vaultToDelete = $state<VaultInfo | null>(null);
	let deleting = $state(false);

	// Invite dialog state
	let showInviteDialog = $state(false);
	let inviteVault = $state<VaultInfo | null>(null);
	let inviteeUuid = $state('');
	let inviteePublicKey = $state('');
	let inviteeRole = $state<'admin' | 'member' | 'readonly'>('member');
	let inviting = $state(false);
	let inviteResult = $state<InviteInfo | null>(null);
	let members = $state<MemberInfo[]>([]);
	let loadingMembers = $state(false);

	// User vaults (excludes internal __xxx__ vaults)
	let userVaults = $derived(vaultState.vaultList.filter(v => !v.name.startsWith('__')));

	// ---- The switcher -------------------------------------------------------
	//
	// One control instead of a list. The list scaled with the number of vaults
	// and starved the sessions it was there to filter; this scales with what
	// you are doing. Searchable, grouped Private / Shared, keyboard-driven.

	let selected = $state<VaultFilter>(loadVaultFilter());
	let open = $state(false);
	let query = $state('');
	let active = $state(0);
	let searchEl = $state<HTMLInputElement | undefined>();
	let rootEl = $state<HTMLDivElement | undefined>();

	const byName = (a: VaultInfo, b: VaultInfo) => a.name.localeCompare(b.name);
	let privateVaults = $derived(userVaults.filter((v) => v.vaultType !== 'shared').sort(byName));
	let sharedVaults = $derived(userVaults.filter((v) => v.vaultType === 'shared').sort(byName));
	let current = $derived(
		selected === ALL_VAULTS || selected === null ? null : (vaultState.vaults.get(selected) ?? null)
	);

	interface Option {
		key: string;
		filter: VaultFilter;
		label: string;
		group?: string;
		vault?: VaultInfo;
	}

	let options = $derived.by((): Option[] => {
		const q = query.trim().toLowerCase();
		const hit = (label: string) => !q || label.toLowerCase().includes(q);
		const out: Option[] = [];
		const all = t('vault.all_vaults');
		const mine = t('vault.private_this_device');
		if (hit(all)) out.push({ key: 'all', filter: ALL_VAULTS, label: all });
		if (hit(mine)) out.push({ key: 'device', filter: null, label: mine });
		for (const v of privateVaults) {
			if (hit(v.name)) out.push({ key: v.id, filter: v.id, label: v.name, group: t('vault.private'), vault: v });
		}
		for (const v of sharedVaults) {
			if (hit(v.name)) out.push({ key: v.id, filter: v.id, label: v.name, group: t('vault.shared'), vault: v });
		}
		return out;
	});

	/** What the closed control says. */
	let triggerName = $derived(
		selected === ALL_VAULTS ? t('vault.all_vaults') : current ? current.name : t('vault.private')
	);
	let triggerMeta = $derived.by(() => {
		if (selected === ALL_VAULTS) {
			return t('vault.vaults_summary', { count: userVaults.length, shared: sharedVaults.length });
		}
		if (!current) return t('vault.this_device');
		if (current.unreachable) return t('vault.unreachable');
		if (current.vaultType === 'shared') {
			return `${t('vault.shared')} · ${t('vault.n_members', { count: current.memberCount ?? 0 })}`;
		}
		return `${t('vault.private')} · ${t('vault.n_secrets', { count: current.secretCount })}`;
	});

	function choose(filter: VaultFilter) {
		selected = filter;
		saveVaultFilter(filter);
		onvaultselect?.(filter);
		open = false;
		query = '';
	}

	async function toggle() {
		open = !open;
		if (!open) return;
		query = '';
		active = Math.max(0, options.findIndex((o) => o.filter === selected));
		await tick();
		searchEl?.focus();
	}

	function onKey(e: KeyboardEvent) {
		if (!open) {
			if (e.key === 'ArrowDown' || e.key === 'Enter' || e.key === ' ') {
				e.preventDefault();
				toggle();
			}
			return;
		}
		if (e.key === 'Escape') {
			e.preventDefault();
			open = false;
		} else if (e.key === 'ArrowDown') {
			e.preventDefault();
			active = Math.min(options.length - 1, active + 1);
		} else if (e.key === 'ArrowUp') {
			e.preventDefault();
			active = Math.max(0, active - 1);
		} else if (e.key === 'Enter') {
			e.preventDefault();
			const o = options[active];
			if (o) choose(o.filter);
		}
	}

	$effect(() => {
		if (!open) return;
		function outside(e: MouseEvent) {
			if (rootEl && !rootEl.contains(e.target as Node)) open = false;
		}
		document.addEventListener('mousedown', outside, true);
		return () => document.removeEventListener('mousedown', outside, true);
	});

	// A remembered vault that has since been deleted (or belongs to another
	// identity) falls back to everything rather than to an empty list.
	$effect(() => {
		if (selected === ALL_VAULTS || selected === null) return;
		if (vaultState.vaults.size > 0 && !vaultState.vaults.has(selected)) choose(ALL_VAULTS);
	});

	// The parent needs the restored filter before it renders a session.
	onMount(() => onvaultselect?.(selected));

	async function handleCreateVault() {
		if (!newVaultName.trim()) {
			error = 'Vault name is required';
			return;
		}

		creating = true;
		error = '';

		try {
			// Shared vaults auto-create Turso database via Platform API
			const vault = await createVault(newVaultName.trim(), newVaultType);
			await refreshVaults();
			choose(vault.id);
			showCreateDialog = false;
			newVaultName = '';
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			creating = false;
		}
	}

	function confirmDelete(vault: VaultInfo, e: Event) {
		e.stopPropagation();
		vaultToDelete = vault;
		showDeleteDialog = true;
	}

	function openInviteDialog(vault: VaultInfo, e: Event) {
		e.stopPropagation();
		inviteVault = vault;
		inviteeUuid = '';
		inviteePublicKey = '';
		inviteeRole = 'member';
		inviteResult = null;
		error = '';
		showInviteDialog = true;
		loadMembers(vault.id);
	}

	async function loadMembers(vaultId: string) {
		loadingMembers = true;
		try {
			members = await listMembers(vaultId);
		} catch (e) {
			console.error('Failed to load members:', e);
		} finally {
			loadingMembers = false;
		}
	}

	async function handleInvite() {
		if (!inviteVault) return;
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
			const result = await inviteMember(inviteVault.id, inviteePublicKey.trim(), inviteeUuid.trim(), inviteeRole);
			inviteResult = result;
			addToast(t('vault.invited_toast'), 'success');
			await loadMembers(inviteVault.id);
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			inviting = false;
		}
	}

	function copyToClipboard(text: string, label: string) {
		navigator.clipboard.writeText(text);
		addToast(`${label} copied`, 'success');
	}

	function copyInviteInfo() {
		if (!inviteResult || !inviteVault) return;
		const info = `Vault Invite: ${inviteVault.name}

Sync URL: ${inviteResult.syncUrl}
Token: ${inviteResult.token}

Go to Settings > Sync > Accept Vault Invite`;
		navigator.clipboard.writeText(info);
		addToast(t('vault.invite_copied_toast'), 'success');
	}

	async function handleDelete() {
		if (!vaultToDelete) return;
		deleting = true;
		try {
			await deleteVault(vaultToDelete.id);
			if (selected === vaultToDelete.id) choose(ALL_VAULTS);
			showDeleteDialog = false;
			vaultToDelete = null;
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			deleting = false;
		}
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="vault-switcher" bind:this={rootEl} onkeydown={onKey}>
	<button
		class="trigger"
		class:open
		class:shared={current?.vaultType === 'shared'}
		aria-haspopup="listbox"
		aria-expanded={open}
		title={t('vault.switch_vault')}
		onclick={toggle}
	>
		{#if selected === ALL_VAULTS}
			<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
				<rect x="3" y="4" width="18" height="6" rx="1.5"/>
				<rect x="3" y="14" width="18" height="6" rx="1.5"/>
			</svg>
		{:else if current?.vaultType === 'shared'}
			<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
				<path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2"/>
				<circle cx="9" cy="7" r="4"/>
				<path d="M23 21v-2a4 4 0 0 0-3-3.87"/>
				<path d="M16 3.13a4 4 0 0 1 0 7.75"/>
			</svg>
		{:else}
			<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
				<rect x="3" y="11" width="18" height="11" rx="2"/>
				<path d="M7 11V7a5 5 0 0 1 10 0v4"/>
			</svg>
		{/if}
		<span class="trigger-text">
			<span class="trigger-name">{triggerName}</span>
			<span class="trigger-meta" class:unreachable={current?.unreachable}>{triggerMeta}</span>
		</span>
		<svg class="chevron" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
			<path d="M6 9l6 6 6-6"/>
		</svg>
	</button>

	{#if open}
		<div class="pop" role="listbox" aria-label={t('vault.vaults')}>
			<input
				class="pop-search"
				type="text"
				bind:this={searchEl}
				bind:value={query}
				placeholder={t('vault.search_vaults')}
				oninput={() => (active = 0)}
				aria-label={t('vault.search_vaults')}
			/>
			<div class="pop-list">
				{#each options as o, i (o.key)}
					{#if o.group && (i === 0 || options[i - 1].group !== o.group)}
						<div class="group-label">{o.group}</div>
					{/if}
					<div class="opt-row">
						<button
							class="opt"
							class:active={i === active}
							class:selected={o.filter === selected}
							class:shared={o.vault?.vaultType === 'shared'}
							role="option"
							aria-selected={o.filter === selected}
							onmouseenter={() => (active = i)}
							onclick={() => choose(o.filter)}
						>
							{#if o.vault?.unreachable}
								<span class="dot" title={[t('vault.unreachable'), o.vault.syncError].filter(Boolean).join(' — ')}></span>
							{:else if o.key === 'all'}
								<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
									<rect x="3" y="4" width="18" height="6" rx="1.5"/>
									<rect x="3" y="14" width="18" height="6" rx="1.5"/>
								</svg>
							{:else if o.vault?.vaultType === 'shared'}
								<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
									<path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2"/>
									<circle cx="9" cy="7" r="4"/>
									<path d="M23 21v-2a4 4 0 0 0-3-3.87"/>
									<path d="M16 3.13a4 4 0 0 1 0 7.75"/>
								</svg>
							{:else}
								<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
									<rect x="3" y="11" width="18" height="11" rx="2"/>
									<path d="M7 11V7a5 5 0 0 1 10 0v4"/>
								</svg>
							{/if}
							<span class="opt-name">{o.label}</span>
							{#if o.vault?.vaultType === 'shared' && o.vault.memberCount}
								<span class="member-count">{o.vault.memberCount}</span>
							{/if}
							{#if o.vault}
								<span class="opt-n">{o.vault.secretCount}</span>
							{/if}
						</button>
						{#if o.vault}
							{#if o.vault.vaultType === 'shared'}
								<button class="opt-action" onclick={(e) => { open = false; openInviteDialog(o.vault!, e); }} title={t('vault.invite_members_short')}>
									<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
										<path d="M16 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2"/>
										<circle cx="8.5" cy="7" r="4"/>
										<path d="M20 8v6M23 11h-6"/>
									</svg>
								</button>
							{/if}
							<button class="opt-action danger" onclick={(e) => { open = false; confirmDelete(o.vault!, e); }} title={t('vault.delete_vault_short')}>
								<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
									<path d="M3 6h18M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>
								</svg>
							</button>
						{/if}
					</div>
				{/each}
				{#if options.length === 0}
					<p class="no-match">{t('vault.no_vault_matches')}</p>
				{/if}
			</div>
			<div class="pop-foot">
				<button class="foot-btn" onclick={() => { open = false; showCreateDialog = true; }}>
					<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M12 5v14M5 12h14"/></svg>
					{t('vault.new_vault')}
				</button>
				<button class="foot-btn" onclick={handleRefresh} disabled={refreshing} title={t('vault.refresh_vaults')}>
					<svg class:spinning={refreshing} width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
						<path d="M1 4v6h6"/>
						<path d="M3.51 15a9 9 0 105.64-9.94L1 10"/>
					</svg>
				</button>
			</div>
		</div>
	{/if}
</div>

<!-- Create Vault Dialog -->
{#if showCreateDialog}
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div class="dialog-overlay" onclick={() => (showCreateDialog = false)} onkeydown={(e) => { if (e.key === 'Escape') showCreateDialog = false; }}>
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div class="dialog" onclick={(e) => e.stopPropagation()} onkeydown={() => {}}>
			<h3>{t('vault.create_vault_short')}</h3>

			<div class="form-group">
				<label for="vault-name">{t('vault.secret_name')}</label>
				<input
					id="vault-name"
					type="text"
					bind:value={newVaultName}
					placeholder="DevOps Team"
					disabled={creating}
				/>
			</div>

			<div class="form-group" role="group" aria-labelledby="vault-type-label">
				<span id="vault-type-label" class="field-label">{t('vault.type')}</span>
				<div class="type-toggle">
					<button
						type="button"
						class="type-btn"
						class:active={newVaultType === 'private'}
						disabled={creating}
						onclick={() => (newVaultType = 'private')}
					>
						{t('vault.private')}
					</button>
					<button
						type="button"
						class="type-btn"
						class:active={newVaultType === 'shared'}
						disabled={creating}
						onclick={() => (newVaultType = 'shared')}
					>
						{t('vault.shared')}
					</button>
				</div>
				<p class="type-hint">
					{#if newVaultType === 'private'}
						{t('vault.type_private_hint')}
					{:else}
						{t('vault.type_shared_hint')}
					{/if}
				</p>
			</div>

			{#if error}
				<p class="error">{error}</p>
			{/if}

			<div class="dialog-actions">
				<button class="btn-secondary" onclick={() => (showCreateDialog = false)} disabled={creating}>
					{t('common.cancel')}
				</button>
				<button class="btn-primary" onclick={handleCreateVault} disabled={creating || !newVaultName.trim()}>
					{#if creating}{t('vault.creating')}{:else}{t('vault.create_short')}{/if}
				</button>
			</div>
		</div>
	</div>
{/if}

<!-- Invite Member Dialog -->
{#if showInviteDialog && inviteVault}
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div class="dialog-overlay" onclick={() => (showInviteDialog = false)} onkeydown={(e) => { if (e.key === 'Escape') showInviteDialog = false; }}>
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div class="dialog invite-dialog" onclick={(e) => e.stopPropagation()} onkeydown={() => {}}>
			<h3>{t('vault.invite_to', { name: inviteVault.name })}</h3>

			{#if inviteResult}
				<!-- Show result -->
				<div class="invite-success">
					<p class="success-msg">{t('vault.invite_created_msg')}</p>
					<div class="invite-field">
						<span class="field-label">{t('vault.sync_url')}</span>
						<div class="field-row">
							<code class="field-value">{inviteResult.syncUrl}</code>
							<button class="copy-btn" onclick={() => copyToClipboard(inviteResult!.syncUrl, 'URL')}>{t('vault.copy')}</button>
						</div>
					</div>
					<div class="invite-field">
						<span class="field-label">{t('vault.token')}</span>
						<div class="field-row">
							<code class="field-value truncate">{inviteResult.token}</code>
							<button class="copy-btn" onclick={() => copyToClipboard(inviteResult!.token, 'Token')}>{t('vault.copy')}</button>
						</div>
					</div>
					<div class="dialog-actions">
						<button class="btn-primary" onclick={copyInviteInfo}>{t('vault.copy_all')}</button>
						<button class="btn-secondary" onclick={() => { inviteResult = null; inviteeUuid = ''; inviteePublicKey = ''; }}>{t('vault.invite_another')}</button>
					</div>
				</div>
			{:else}
				<!-- Show form -->
				<div class="invite-section">
					<p class="section-label">{t('vault.your_info_share')}</p>
					<div class="info-row">
						<span class="info-label">UUID:</span>
						<code class="info-value">{vaultState.userUuid ?? 'N/A'}</code>
						{#if vaultState.userUuid}
							<button class="copy-btn-sm" onclick={() => copyToClipboard(vaultState.userUuid!, 'UUID')}>{t('vault.copy')}</button>
						{/if}
					</div>
					<div class="info-row">
						<span class="info-label">Key:</span>
						<code class="info-value truncate">{vaultState.publicKey ?? 'N/A'}</code>
						{#if vaultState.publicKey}
							<button class="copy-btn-sm" onclick={() => copyToClipboard(vaultState.publicKey!, 'Key')}>{t('vault.copy')}</button>
						{/if}
					</div>
				</div>

				<div class="invite-section">
					<p class="section-label">{t('vault.invitee_info_get')}</p>
					<div class="form-group">
						<input type="text" placeholder={t('vault.invitee_uuid')} bind:value={inviteeUuid} disabled={inviting} />
					</div>
					<div class="form-group">
						<input type="text" placeholder={t('vault.invitee_public_key')} bind:value={inviteePublicKey} disabled={inviting} />
					</div>
					<div class="form-group">
						<select bind:value={inviteeRole} disabled={inviting}>
							<option value="member">{t('vault.member_rw')}</option>
							<option value="admin">{t('vault.admin_can_invite')}</option>
							<option value="readonly">{t('vault.role_read_only')}</option>
						</select>
					</div>
				</div>

				{#if error}
					<p class="error">{error}</p>
				{/if}

				<div class="dialog-actions">
					<button class="btn-secondary" onclick={() => (showInviteDialog = false)} disabled={inviting}>{t('common.cancel')}</button>
					<button class="btn-primary" onclick={handleInvite} disabled={inviting || !inviteeUuid.trim() || !inviteePublicKey.trim()}>
						{#if inviting}{t('vault.inviting')}{:else}{t('vault.send_invite')}{/if}
					</button>
				</div>
			{/if}
		</div>
	</div>
{/if}

<!-- Delete Vault Confirmation -->
{#if showDeleteDialog && vaultToDelete}
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div class="dialog-overlay" onclick={() => (showDeleteDialog = false)} onkeydown={(e) => { if (e.key === 'Escape') showDeleteDialog = false; }}>
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div class="dialog" onclick={(e) => e.stopPropagation()} onkeydown={() => {}}>
			<h3>{t('vault.delete_vault')}</h3>
			<p class="delete-warning">
				{t('vault.delete_vault_confirm')}
			</p>
			{#if error}
				<p class="error">{error}</p>
			{/if}
			<div class="dialog-actions">
				<button class="btn-secondary" onclick={() => (showDeleteDialog = false)} disabled={deleting}>
					{t('common.cancel')}
				</button>
				<button class="btn-danger" onclick={handleDelete} disabled={deleting}>
					{#if deleting}{t('vault.deleting')}{:else}{t('common.delete')}{/if}
				</button>
			</div>
		</div>
	</div>
{/if}

<style>
	.vault-switcher {
		position: relative;
		padding: 4px 0 8px;
		border-bottom: 1px solid var(--color-border);
		margin-bottom: 8px;
	}

	.trigger {
		display: flex;
		align-items: center;
		gap: 8px;
		width: 100%;
		padding: 7px 10px;
		font-family: inherit;
		text-align: left;
		color: var(--color-text-secondary);
		background: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
		border-radius: 8px;
		cursor: pointer;
		transition: border-color 0.15s, color 0.15s;
	}

	.trigger:hover,
	.trigger.open {
		color: var(--color-text-primary);
		border-color: color-mix(in srgb, var(--color-accent) 55%, var(--color-border));
	}

	.trigger:focus-visible {
		outline: 2px solid var(--color-accent);
		outline-offset: 1px;
	}

	.trigger.shared {
		color: #10b981;
	}

	.trigger-text {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		line-height: 1.2;
	}

	.trigger-name {
		font-size: 0.8125rem;
		font-weight: 600;
		color: var(--color-text-primary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.trigger.shared .trigger-name {
		color: #10b981;
	}

	.trigger-meta {
		font-size: 0.6875rem;
		color: var(--color-text-tertiary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.trigger-meta.unreachable {
		color: var(--color-warning, #f59e0b);
	}

	.chevron {
		flex-shrink: 0;
		color: var(--color-text-tertiary);
		transition: transform 0.15s;
	}

	.trigger.open .chevron {
		transform: rotate(180deg);
	}

	/* The picker. Sits over the sessions rather than pushing them down: the
	   whole point is that the sessions keep their room. */
	.pop {
		position: absolute;
		left: 0;
		right: 0;
		top: calc(100% - 4px);
		z-index: 30;
		display: flex;
		flex-direction: column;
		background: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
		border-radius: 8px;
		box-shadow: var(--shadow-elevated, 0 8px 32px rgba(0, 0, 0, 0.35));
		padding: 6px;
	}

	.pop-search {
		width: 100%;
		padding: 6px 8px;
		font-family: inherit;
		font-size: 0.75rem;
		color: var(--color-text-primary);
		background: var(--color-bg-secondary);
		border: 1px solid var(--color-border);
		border-radius: 6px;
		outline: none;
	}

	.pop-search:focus {
		border-color: var(--color-accent);
	}

	.pop-search::placeholder {
		color: var(--color-text-tertiary);
	}

	.pop-list {
		max-height: min(46vh, 380px);
		overflow-y: auto;
		margin-top: 4px;
		scrollbar-width: thin;
	}

	.group-label {
		padding: 8px 8px 3px;
		font-size: 0.625rem;
		font-weight: 600;
		letter-spacing: 0.05em;
		text-transform: uppercase;
		color: var(--color-text-tertiary);
	}

	.opt-row {
		display: flex;
		align-items: center;
		gap: 2px;
	}

	.opt {
		flex: 1;
		min-width: 0;
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 5px 8px;
		border: none;
		border-radius: 6px;
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
		text-align: left;
		font-family: inherit;
		font-size: 0.75rem;
	}

	.opt.active {
		background: var(--color-surface-hover);
		color: var(--color-text-primary);
	}

	.opt.selected {
		color: var(--color-accent);
	}

	.opt.shared {
		color: #10b981;
	}

	.opt.shared.active {
		background: rgba(16, 185, 129, 0.12);
	}

	.opt-name {
		flex: 1;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.opt-n {
		font-size: 0.625rem;
		color: var(--color-text-tertiary);
		font-variant-numeric: tabular-nums;
	}

	.dot {
		width: 13px;
		height: 13px;
		flex-shrink: 0;
		display: grid;
		place-items: center;
	}

	.dot::before {
		content: '';
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: var(--color-warning, #f59e0b);
	}

	.member-count {
		font-size: 0.625rem;
		padding: 1px 6px;
		border-radius: 8px;
		background: rgba(16, 185, 129, 0.18);
		color: #10b981;
	}

	/* Invite and delete: present, quiet until the row is pointed at. */
	.opt-action {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 22px;
		height: 22px;
		flex-shrink: 0;
		border: none;
		border-radius: 4px;
		background: transparent;
		color: var(--color-text-tertiary);
		cursor: pointer;
		opacity: 0;
		transition: opacity 0.1s, color 0.1s;
	}

	.opt-row:hover .opt-action,
	.opt-action:focus-visible {
		opacity: 1;
	}

	.opt-action:hover {
		color: var(--color-text-primary);
		background: var(--color-surface-active);
	}

	.opt-action.danger:hover {
		color: var(--color-danger, #ef4444);
	}

	.no-match {
		margin: 0;
		padding: 12px 8px;
		font-size: 0.75rem;
		color: var(--color-text-tertiary);
		text-align: center;
	}

	.pop-foot {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 4px;
		margin-top: 4px;
		padding-top: 6px;
		border-top: 1px solid var(--color-border);
	}

	.foot-btn {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		padding: 5px 8px;
		border: none;
		border-radius: 6px;
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
		font-family: inherit;
		font-size: 0.75rem;
	}

	.foot-btn:hover:not(:disabled) {
		background: var(--color-surface-hover);
		color: var(--color-text-primary);
	}

	.foot-btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	@keyframes spin {
		from { transform: rotate(0deg); }
		to { transform: rotate(360deg); }
	}

	.spinning {
		animation: spin 0.8s linear infinite;
	}

	.dialog-overlay {
		position: fixed;
		inset: 0;
		background: var(--color-surface-sunken);
		backdrop-filter: blur(4px);
		display: flex;
		align-items: center;
		justify-content: center;
		z-index: 1000;
	}

	.dialog {
		background: var(--color-bg-secondary);
		border: 1px solid var(--color-border);
		border-radius: 12px;
		padding: 20px;
		width: 100%;
		max-width: 360px;
	}

	.dialog h3 {
		margin: 0 0 16px;
		font-size: 1rem;
		font-weight: 600;
	}

	.form-group {
		margin-bottom: 16px;
	}

	.form-group label,
	.form-group .field-label {
		display: block;
		font-size: 0.75rem;
		font-weight: 500;
		color: var(--color-text-secondary);
		margin-bottom: 6px;
	}

	.form-group input {
		width: 100%;
		padding: 8px 12px;
		border: 1px solid var(--color-border);
		border-radius: 6px;
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		font-size: 0.875rem;
	}

	.form-group input:focus {
		outline: none;
		border-color: var(--color-accent);
	}

	.type-toggle {
		display: flex;
		gap: 8px;
	}

	.type-btn {
		flex: 1;
		padding: 8px;
		border: 1px solid var(--color-border);
		border-radius: 6px;
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
		font-size: 0.8125rem;
		transition: all 0.15s;
	}

	.type-btn:hover {
		border-color: var(--color-text-tertiary);
	}

	.type-btn.active {
		background: var(--color-accent);
		border-color: var(--color-accent);
		color: white;
	}

	.type-hint {
		margin: 8px 0 0;
		font-size: 0.6875rem;
		color: var(--color-text-tertiary);
	}

	.error {
		margin: 0 0 12px;
		font-size: 0.75rem;
		color: var(--color-danger);
	}

	.dialog-actions {
		display: flex;
		justify-content: flex-end;
		gap: 8px;
	}

	.btn-primary, .btn-secondary {
		padding: 8px 16px;
		border-radius: 6px;
		font-size: 0.8125rem;
		font-weight: 500;
		cursor: pointer;
	}

	.btn-primary {
		background: var(--color-accent);
		border: none;
		color: white;
	}

	.btn-primary:hover:not(:disabled) {
		opacity: 0.9;
	}

	.btn-primary:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.btn-secondary {
		background: transparent;
		border: 1px solid var(--color-border);
		color: var(--color-text-primary);
	}

	.btn-secondary:hover:not(:disabled) {
		background: var(--color-surface-hover);
	}

	.btn-danger {
		background: var(--color-danger);
		border: none;
		color: white;
		padding: 8px 16px;
		border-radius: 6px;
		font-size: 0.8125rem;
		font-weight: 500;
		cursor: pointer;
	}

	.btn-danger:hover:not(:disabled) {
		opacity: 0.9;
	}

	.btn-danger:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.invite-dialog {
		max-width: 420px;
	}

	.invite-success {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.success-msg {
		margin: 0;
		font-size: 0.8125rem;
		color: var(--color-success);
	}

	.invite-field {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.field-row {
		display: flex;
		gap: 8px;
		align-items: center;
	}

	.field-value {
		flex: 1;
		padding: 6px 8px;
		background: var(--color-bg-primary);
		border-radius: 4px;
		font-size: 0.6875rem;
		word-break: break-all;
	}

	.field-value.truncate {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.copy-btn {
		padding: 4px 8px;
		font-size: 0.625rem;
		background: transparent;
		border: 1px solid var(--color-accent);
		border-radius: 4px;
		color: var(--color-accent);
		cursor: pointer;
	}

	.copy-btn:hover {
		background: rgba(59, 130, 246, 0.1);
	}

	.invite-section {
		margin-bottom: 16px;
		padding: 12px;
		background: var(--color-surface-hover);
		border-radius: 8px;
	}

	.section-label {
		margin: 0 0 8px;
		font-size: 0.6875rem;
		font-weight: 600;
		text-transform: uppercase;
		color: var(--color-text-tertiary);
	}

	.info-row {
		display: flex;
		align-items: center;
		gap: 8px;
		margin-bottom: 6px;
	}

	.info-label {
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
		width: 40px;
	}

	.info-value {
		flex: 1;
		font-size: 0.6875rem;
		color: var(--color-text-primary);
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.copy-btn-sm {
		padding: 2px 6px;
		font-size: 0.5625rem;
		background: transparent;
		border: 1px solid var(--color-border);
		border-radius: 3px;
		color: var(--color-text-secondary);
		cursor: pointer;
	}

	.copy-btn-sm:hover {
		border-color: var(--color-accent);
		color: var(--color-accent);
	}

	.form-group select {
		width: 100%;
		padding: 8px 12px;
		border: 1px solid var(--color-border);
		border-radius: 6px;
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		font-size: 0.875rem;
	}

	.delete-warning {
		margin: 0 0 16px;
		font-size: 0.8125rem;
		color: var(--color-text-secondary);
		line-height: 1.5;
	}

</style>
