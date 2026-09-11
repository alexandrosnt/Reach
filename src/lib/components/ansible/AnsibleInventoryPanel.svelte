<script lang="ts">
	import { t } from '$lib/state/i18n.svelte';
	import { getActiveProject, saveInventory, generateInventory, writeInventory } from '$lib/state/ansible.svelte';
	import type { AnsibleInventoryHost, AnsibleInventoryGroup } from '$lib/ipc/ansible';
	import Button from '$lib/components/shared/Button.svelte';
	import Modal from '$lib/components/shared/Modal.svelte';
	import { sessionList, type SessionConfig } from '$lib/ipc/sessions';
	import { vaultState } from '$lib/state/vault.svelte';
	import { hostFromSession, alreadyInInventory, missingGroups } from '$lib/ansible/from-sessions';

	let project = $derived(getActiveProject());

	let hosts = $state<AnsibleInventoryHost[]>([]);
	let groups = $state<AnsibleInventoryGroup[]>([]);
	let iniPreview = $state<string | null>(null);

	// Sync from project
	$effect(() => {
		if (project) {
			hosts = structuredClone(project.inventoryHosts);
			groups = structuredClone(project.inventoryGroups);
		}
	});

	let editingHost = $state<number | null>(null);
	let editingGroup = $state<number | null>(null);

	function addHost() {
		hosts = [...hosts, { name: '', ansibleHost: null, ansiblePort: null, ansibleUser: null, groups: [], variables: {} }];
		editingHost = hosts.length - 1;
	}

	function removeHost(idx: number) {
		hosts = hosts.filter((_, i) => i !== idx);
		editingHost = null;
	}

	function addGroup() {
		groups = [...groups, { name: '', variables: {}, children: [] }];
		editingGroup = groups.length - 1;
	}

	function removeGroup(idx: number) {
		groups = groups.filter((_, i) => i !== idx);
		editingGroup = null;
	}

	async function handleSave() {
		await saveInventory(hosts, groups);
	}

	// ---- From sessions ------------------------------------------------------
	// Reach knows every saved session's address, user, key and bastion. Pick
	// the ones that belong in this inventory; nothing is copied that should
	// not be (a password never is).
	let showFromSessions = $state(false);
	let sessions = $state<SessionConfig[]>([]);
	let picked = $state<Set<string>>(new Set());
	let sessionsLoading = $state(false);

	async function openFromSessions() {
		showFromSessions = true;
		picked = new Set();
		sessionsLoading = true;
		try {
			sessions = await sessionList();
		} catch {
			sessions = [];
		} finally {
			sessionsLoading = false;
		}
	}

	function togglePick(id: string) {
		const next = new Set(picked);
		if (next.has(id)) next.delete(id);
		else next.add(id);
		picked = next;
	}

	let pickable = $derived(sessions.filter((s) => !alreadyInInventory(s, hosts)));
	let passwordPicked = $derived(
		sessions.some((s) => picked.has(s.id) && s.auth_method?.type === 'Password')
	);

	async function addPicked() {
		let next = [...hosts];
		for (const s of sessions) {
			if (!picked.has(s.id)) continue;
			next = [...next, hostFromSession(s, next).host];
		}
		const newGroups = missingGroups(next, groups);
		hosts = next;
		groups = [...groups, ...newGroups];
		showFromSessions = false;
		editingHost = null;
		await saveInventory(hosts, groups);
	}

	function vaultName(s: SessionConfig): string | null {
		if (!s.vault_id) return null;
		return vaultState.vaults.get(s.vault_id)?.name ?? null;
	}

	async function handleGenerateIni() {
		try {
			iniPreview = await generateInventory();
		} catch {
			iniPreview = '# Error generating inventory';
		}
	}

	async function handleWriteInventory() {
		if (!iniPreview) return;
		await writeInventory(iniPreview);
		iniPreview = null;
	}
</script>

<div class="inventory-panel">
	<!-- Hosts Section -->
	<div class="section">
		<div class="section-header">
			<h3 class="section-title">{t('ansible.hosts')}</h3>
			<div class="section-actions">
				<Button variant="secondary" size="sm" onclick={openFromSessions}>{t('ansible.from_sessions')}</Button>
				<Button variant="secondary" size="sm" onclick={addHost}>{t('ansible.add_host')}</Button>
			</div>
		</div>

		{#if hosts.length === 0}
			<p class="empty-text">{t('ansible.no_hosts')}</p>
		{:else}
			<div class="item-list">
				{#each hosts as host, idx (idx)}
					<div class="item-card" class:editing={editingHost === idx}>
						{#if editingHost === idx}
							<div class="edit-form">
								<div class="form-row">
									<label class="field-label">
										{t('ansible.host_name')}
										<input class="field-input" bind:value={host.name} placeholder="web1" />
									</label>
								</div>
								<div class="form-row">
									<label class="field-label">
										{t('ansible.host_address')}
										<input class="field-input" bind:value={host.ansibleHost} placeholder="192.168.1.10" />
									</label>
								</div>
								<div class="form-row-half">
									<div class="form-row">
										<label class="field-label">
											{t('ansible.host_port')}
											<input class="field-input" type="number" bind:value={host.ansiblePort} placeholder="22" />
										</label>
									</div>
									<div class="form-row">
										<label class="field-label">
											{t('ansible.host_user')}
											<input class="field-input" bind:value={host.ansibleUser} placeholder="admin" />
										</label>
									</div>
								</div>
								<div class="form-row">
									<label class="field-label">
										{t('ansible.host_groups')}
										<input
											class="field-input"
											value={host.groups.join(', ')}
											placeholder="webservers, production"
											onchange={(e) => {
												host.groups = (e.currentTarget as HTMLInputElement).value.split(',').map(s => s.trim()).filter(Boolean);
											}}
										/>
									</label>
								</div>
								<div class="edit-actions">
									<Button variant="secondary" size="sm" onclick={() => editingHost = null}>{t('common.ok')}</Button>
									<Button variant="danger" size="sm" onclick={() => removeHost(idx)}>{t('ansible.remove')}</Button>
								</div>
							</div>
						{:else}
							<div class="item-summary" role="button" tabindex="0" onclick={() => editingHost = idx} onkeydown={(e) => { if (e.key === 'Enter') editingHost = idx; }}>
								<span class="item-name">{host.name || '(unnamed)'}</span>
								<span class="item-detail">
									{host.ansibleHost || ''}
									{host.ansiblePort ? `:${host.ansiblePort}` : ''}
									{host.ansibleUser ? ` (${host.ansibleUser})` : ''}
								</span>
								{#if host.groups.length > 0}
									<div class="item-tags">
										{#each host.groups as g (g)}
											<span class="tag">{g}</span>
										{/each}
									</div>
								{/if}
							</div>
						{/if}
					</div>
				{/each}
			</div>
		{/if}
	</div>

	<!-- Groups Section -->
	<div class="section">
		<div class="section-header">
			<h3 class="section-title">{t('ansible.groups')}</h3>
			<Button variant="secondary" size="sm" onclick={addGroup}>{t('ansible.add_group')}</Button>
		</div>

		{#if groups.length === 0}
			<p class="empty-text">{t('ansible.no_groups')}</p>
		{:else}
			<div class="item-list">
				{#each groups as group, idx (idx)}
					<div class="item-card" class:editing={editingGroup === idx}>
						{#if editingGroup === idx}
							<div class="edit-form">
								<div class="form-row">
									<label class="field-label">
										{t('ansible.group_name')}
										<input class="field-input" bind:value={group.name} placeholder="webservers" />
									</label>
								</div>
								<div class="form-row">
									<label class="field-label">
										{t('ansible.group_children')}
										<input
											class="field-input"
											value={group.children.join(', ')}
											placeholder="child_group1, child_group2"
											onchange={(e) => {
												group.children = (e.currentTarget as HTMLInputElement).value.split(',').map(s => s.trim()).filter(Boolean);
											}}
										/>
									</label>
								</div>
								<div class="edit-actions">
									<Button variant="secondary" size="sm" onclick={() => editingGroup = null}>{t('common.ok')}</Button>
									<Button variant="danger" size="sm" onclick={() => removeGroup(idx)}>{t('ansible.remove')}</Button>
								</div>
							</div>
						{:else}
							<div class="item-summary" role="button" tabindex="0" onclick={() => editingGroup = idx} onkeydown={(e) => { if (e.key === 'Enter') editingGroup = idx; }}>
								<span class="item-name">{group.name || '(unnamed)'}</span>
								{#if group.children.length > 0}
									<span class="item-detail">children: {group.children.join(', ')}</span>
								{/if}
							</div>
						{/if}
					</div>
				{/each}
			</div>
		{/if}
	</div>

	<!-- Actions -->
	<div class="panel-actions">
		<Button variant="primary" size="sm" onclick={handleSave}>{t('ansible.save')}</Button>
		<Button variant="secondary" size="sm" onclick={handleGenerateIni}>{t('ansible.generate_ini')}</Button>
	</div>

	<!-- INI Preview -->
	{#if iniPreview !== null}
		<div class="ini-preview">
			<div class="ini-header">
				<h4 class="ini-title">{t('ansible.ini_preview')}</h4>
				<Button variant="primary" size="sm" onclick={handleWriteInventory}>{t('ansible.write_inventory')}</Button>
			</div>
			<pre class="ini-content">{iniPreview}</pre>
		</div>
	{/if}
</div>

<Modal open={showFromSessions} onclose={() => (showFromSessions = false)} title={t('ansible.from_sessions_title')} maxWidth="560px">
	{#if sessionsLoading}
		<p class="empty-text">{t('common.loading')}</p>
	{:else if sessions.length === 0}
		<p class="empty-text">{t('ansible.from_sessions_none')}</p>
	{:else}
		<div class="session-pick">
			{#each sessions as s (s.id)}
				{@const present = !pickable.includes(s)}
				<label class="pick-row" class:present>
					<input type="checkbox" checked={picked.has(s.id)} disabled={present} onchange={() => togglePick(s.id)} />
					<span class="pick-main">
						<span class="pick-name">{s.name}</span>
						<span class="pick-detail">{s.username}@{s.host}{s.port !== 22 ? `:${s.port}` : ''}</span>
					</span>
					<span class="pick-tags">
						{#if vaultName(s)}<span class="pick-tag vault">{vaultName(s)}</span>{/if}
						{#if s.auth_method?.type === 'Key'}<span class="pick-tag">KEY</span>{:else if s.auth_method?.type === 'Password'}<span class="pick-tag warn">PW</span>{/if}
						{#if s.jump_chain && s.jump_chain.length > 0}<span class="pick-tag">JUMP</span>{/if}
						{#if present}<span class="pick-tag dim">{t('ansible.already_in_inventory')}</span>{/if}
					</span>
				</label>
			{/each}
		</div>
		{#if passwordPicked}
			<p class="pick-hint">{t('ansible.from_sessions_password_hint')}</p>
		{/if}
		<p class="pick-hint">{t('ansible.from_sessions_hint')}</p>
	{/if}

	{#snippet actions()}
		<Button variant="ghost" onclick={() => (showFromSessions = false)}>{t('common.cancel')}</Button>
		<Button variant="primary" disabled={picked.size === 0} onclick={addPicked}>
			{t('ansible.add_n_hosts', { count: picked.size })}
		</Button>
	{/snippet}
</Modal>

<style>
	.section-actions {
		display: flex;
		gap: 6px;
	}

	.session-pick {
		display: flex;
		flex-direction: column;
		gap: 2px;
		max-height: 360px;
		overflow-y: auto;
		scrollbar-width: thin;
	}

	.pick-row {
		display: grid;
		grid-template-columns: auto minmax(0, 1fr) auto;
		align-items: center;
		gap: 10px;
		padding: 6px 8px;
		border-radius: 6px;
		cursor: pointer;
	}

	.pick-row:hover {
		background: var(--color-surface-hover);
	}

	.pick-row.present {
		opacity: 0.55;
		cursor: default;
	}

	.pick-main {
		display: flex;
		flex-direction: column;
		min-width: 0;
	}

	.pick-name {
		font-size: 0.8125rem;
		color: var(--color-text-primary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.pick-detail {
		font-size: 0.6875rem;
		font-family: var(--font-mono, monospace);
		color: var(--color-text-tertiary);
	}

	.pick-tags {
		display: flex;
		gap: 4px;
		flex-wrap: wrap;
		justify-content: flex-end;
	}

	.pick-tag {
		font-size: 0.625rem;
		font-weight: 600;
		letter-spacing: 0.04em;
		padding: 1px 6px;
		border-radius: 4px;
		background: var(--color-surface-hover);
		color: var(--color-text-secondary);
	}

	.pick-tag.vault {
		font-weight: 500;
		letter-spacing: 0;
		color: var(--color-text-secondary);
		border: 1px solid var(--color-border);
		background: transparent;
	}

	.pick-tag.warn {
		color: var(--color-warning, #f59e0b);
	}

	.pick-tag.dim {
		font-weight: 500;
		letter-spacing: 0;
		color: var(--color-text-tertiary);
		background: transparent;
	}

	.pick-hint {
		margin: 10px 0 0;
		font-size: 0.75rem;
		color: var(--color-text-secondary);
	}

	.inventory-panel {
		padding: 16px;
		display: flex;
		flex-direction: column;
		gap: 20px;
		overflow-y: auto;
	}

	.section {
		display: flex;
		flex-direction: column;
		gap: 10px;
	}

	.section-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
	}

	.section-title {
		margin: 0;
		font-size: 0.875rem;
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.empty-text {
		margin: 0;
		font-size: 0.8125rem;
		color: var(--color-text-secondary);
		font-style: italic;
	}

	.item-list {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.item-card {
		padding: 10px 12px;
		border-radius: var(--radius-btn);
		border: 1px solid var(--color-border);
		background: var(--color-bg-primary);
	}

	.item-card.editing {
		border-color: var(--color-accent);
	}

	.item-summary {
		cursor: pointer;
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.item-name {
		font-size: 0.8125rem;
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.item-detail {
		font-size: 0.75rem;
		color: var(--color-text-secondary);
		font-family: monospace;
	}

	.item-tags {
		display: flex;
		gap: 4px;
		flex-wrap: wrap;
	}

	.tag {
		font-size: 0.6875rem;
		padding: 2px 6px;
		border-radius: 3px;
		background: rgba(99, 102, 241, 0.15);
		color: var(--color-accent);
	}

	.edit-form {
		display: flex;
		flex-direction: column;
		gap: 10px;
	}

	.form-row {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.form-row-half {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 10px;
	}

	.field-label {
		font-size: 0.6875rem;
		font-weight: 600;
		color: var(--color-text-secondary);
		text-transform: uppercase;
		letter-spacing: 0.04em;
	}

	.field-input {
		padding: 6px 8px;
		border-radius: var(--radius-btn);
		border: 1px solid var(--color-border);
		background: var(--color-bg-elevated);
		color: var(--color-text-primary);
		font-size: 0.8125rem;
		font-family: inherit;
	}

	.field-input:focus {
		outline: none;
		border-color: var(--color-accent);
	}

	.edit-actions {
		display: flex;
		gap: 8px;
		justify-content: flex-end;
	}

	.panel-actions {
		display: flex;
		gap: 8px;
	}

	.ini-preview {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.ini-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
	}

	.ini-title {
		margin: 0;
		font-size: 0.8125rem;
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.ini-content {
		padding: 10px 12px;
		border-radius: var(--radius-btn);
		background: color-mix(in srgb, var(--color-bg-primary) 90%, black);
		border: 1px solid var(--color-border);
		font-family: monospace;
		font-size: 0.75rem;
		line-height: 1.5;
		color: var(--color-text-primary);
		overflow-x: auto;
		white-space: pre;
		margin: 0;
	}
</style>
