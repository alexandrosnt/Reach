<script lang="ts">
	import Modal from '$lib/components/shared/Modal.svelte';
	import Button from '$lib/components/shared/Button.svelte';
	import { sshconfigListHosts, type SshHostEntry } from '$lib/ipc/sshconfig';
	import { sessionCreate, sessionList, type SessionConfig, type AuthMethod, type JumpHostConfig } from '$lib/ipc/sessions';
	import { addToast } from '$lib/state/toasts.svelte';
	import { t } from '$lib/state/i18n.svelte';

	interface Props {
		open: boolean;
		onsave?: () => void;
	}

	let { open = $bindable(), onsave }: Props = $props();

	let hosts = $state<SshHostEntry[]>([]);
	let existingSessions = $state<SessionConfig[]>([]);
	let selected = $state<Set<string>>(new Set());
	let loading = $state(false);
	let importing = $state(false);
	let error = $state<string | undefined>();

	let selectableHosts = $derived(hosts.filter(h => !isAlreadyImported(h)));
	let selectedCount = $derived(selected.size);
	let canImport = $derived(selectedCount > 0 && !importing);

	function isAlreadyImported(host: SshHostEntry): boolean {
		return existingSessions.some(
			s => s.host === host.hostname && s.port === host.port && s.username === host.user
		);
	}

	function buildProxyChainLabel(host: SshHostEntry): string {
		if (!host.proxy_jump || host.proxy_jump.length === 0) return '';
		return host.proxy_jump.map(j => j.host).join(' → ');
	}

	$effect(() => {
		if (open) {
			loadHosts();
		}
	});

	async function loadHosts(): Promise<void> {
		loading = true;
		error = undefined;
		selected = new Set();

		try {
			const [hostList, sessions] = await Promise.all([
				sshconfigListHosts(),
				sessionList(),
			]);
			hosts = hostList;
			existingSessions = sessions;
		} catch (err) {
			error = String(err);
		} finally {
			loading = false;
		}
	}

	function selectAll(): void {
		selected = new Set(selectableHosts.map(h => h.name));
	}

	function deselectAll(): void {
		selected = new Set();
	}

	function toggleHost(name: string): void {
		const next = new Set(selected);
		if (next.has(name)) {
			next.delete(name);
		} else {
			next.add(name);
		}
		selected = next;
	}

	async function handleImport(): Promise<void> {
		if (!canImport) return;
		importing = true;
		error = undefined;
		let importedCount = 0;

		try {
			for (const host of hosts) {
				if (!selected.has(host.name)) continue;

				const authMethod: AuthMethod = host.identity_files.length > 0
					? { type: 'Key', path: host.identity_files[0] }
					: { type: 'Password' };

				const jumpChain: JumpHostConfig[] | null = host.proxy_jump.length > 0
					? host.proxy_jump.map(j => ({
						host: j.host,
						port: j.port,
						username: j.user,
						auth_method: j.identity_files.length > 0
							? { type: 'Key' as const, path: j.identity_files[0] }
							: { type: 'Password' as const },
					}))
					: null;

				await sessionCreate({
					name: host.name,
					host: host.hostname,
					port: host.port,
					username: host.user,
					authMethod,
					folderId: null,
					tags: ['ssh-config'],
					jumpChain,
				});
				importedCount++;
			}

			addToast(t('session.import_success', { count: String(importedCount) }), 'success');
			onsave?.();
			open = false;
		} catch (err) {
			error = String(err);
		} finally {
			importing = false;
		}
	}

	function handleClose(): void {
		if (!importing) {
			open = false;
		}
	}
</script>

<Modal {open} onclose={handleClose} title={t('session.import_title')}>
	<div class="import-content">
		<p class="import-desc">{t('session.import_desc')}</p>

		{#if loading}
			<div class="import-loading">
				<span class="spinner"></span>
				<span>{t('session.import_parsing')}</span>
			</div>
		{:else if error}
			<div class="import-error">{error}</div>
		{:else if hosts.length === 0}
			<p class="import-empty">{t('session.import_no_hosts')}</p>
		{:else}
			<div class="select-actions">
				<button class="select-btn" onclick={selectAll} disabled={importing}>
					{t('session.import_select_all')}
				</button>
				<button class="select-btn" onclick={deselectAll} disabled={importing}>
					{t('session.import_deselect_all')}
				</button>
			</div>

			<div class="host-list">
				{#each hosts as host (host.name)}
					{@const alreadyImported = isAlreadyImported(host)}
					{@const proxyLabel = buildProxyChainLabel(host)}
					<label class="host-row" class:disabled={alreadyImported}>
						<input
							type="checkbox"
							class="host-check"
							checked={selected.has(host.name)}
							disabled={alreadyImported || importing}
							onchange={() => toggleHost(host.name)}
						/>
						<div class="host-info">
							<div class="host-name-row">
								<span class="host-name">{host.name}</span>
								{#if alreadyImported}
									<span class="badge-imported">{t('session.import_already_exists')}</span>
								{/if}
							</div>
							<span class="host-detail">
								{host.user}@{host.hostname}:{host.port}
								{#if proxyLabel}
									<span class="proxy-chain">{t('session.import_proxy_chain', { chain: proxyLabel })}</span>
								{/if}
							</span>
						</div>
					</label>
				{/each}
			</div>
		{/if}
	</div>

	{#snippet actions()}
		<Button variant="secondary" onclick={handleClose} disabled={importing}>
			{t('common.cancel')}
		</Button>
		<Button variant="primary" onclick={handleImport} disabled={!canImport}>
			{#if importing}
				<span class="spinner"></span>
			{/if}
			{t('session.import_selected', { count: String(selectedCount) })}
		</Button>
	{/snippet}
</Modal>

<style>
	.import-content {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.import-desc {
		margin: 0;
		font-size: 0.8125rem;
		color: var(--color-text-secondary);
	}

	.import-loading {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 8px;
		padding: 24px 0;
		font-size: 0.8125rem;
		color: var(--color-text-secondary);
	}

	.import-error {
		padding: 8px 12px;
		font-size: 0.8125rem;
		color: var(--color-danger);
		background-color: rgba(255, 69, 58, 0.08);
		border: 1px solid rgba(255, 69, 58, 0.2);
		border-radius: var(--radius-btn);
	}

	.import-empty {
		margin: 0;
		padding: 24px 0;
		font-size: 0.8125rem;
		color: var(--color-text-secondary);
		text-align: center;
	}

	.select-actions {
		display: flex;
		gap: 6px;
	}

	.select-btn {
		padding: 4px 10px;
		font-family: var(--font-sans);
		font-size: 0.6875rem;
		font-weight: 500;
		color: var(--color-text-secondary);
		background: transparent;
		border: 1px solid var(--color-border);
		border-radius: 4px;
		cursor: pointer;
		transition: background-color var(--duration-default) var(--ease-default);
	}

	.select-btn:hover:not(:disabled) {
		background-color: rgba(255, 255, 255, 0.06);
		color: var(--color-text-primary);
	}

	.select-btn:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}

	.host-list {
		display: flex;
		flex-direction: column;
		gap: 2px;
		max-height: 320px;
		overflow-y: auto;
	}

	.host-row {
		display: flex;
		align-items: flex-start;
		gap: 10px;
		padding: 8px 10px;
		border-radius: 6px;
		cursor: pointer;
		transition: background-color var(--duration-default) var(--ease-default);
	}

	.host-row:hover:not(.disabled) {
		background-color: rgba(255, 255, 255, 0.04);
	}

	.host-row.disabled {
		opacity: 0.5;
		cursor: default;
	}

	.host-check {
		width: 14px;
		height: 14px;
		margin-top: 2px;
		accent-color: var(--color-accent);
		flex-shrink: 0;
	}

	.host-info {
		display: flex;
		flex-direction: column;
		gap: 2px;
		min-width: 0;
	}

	.host-name-row {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.host-name {
		font-size: 0.8125rem;
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.badge-imported {
		padding: 1px 6px;
		font-size: 0.5625rem;
		font-weight: 500;
		color: var(--color-text-secondary);
		background-color: rgba(255, 255, 255, 0.06);
		border-radius: 3px;
		text-transform: uppercase;
		letter-spacing: 0.03em;
	}

	.host-detail {
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
		font-family: var(--font-mono);
	}

	.proxy-chain {
		margin-left: 6px;
		color: var(--color-accent);
		font-family: var(--font-sans);
		font-style: italic;
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
</style>
