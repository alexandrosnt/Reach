<!--
	Sharing settings: the switch, and the ICE servers.

	The switch is the important control. Off is the default, and off means
	none of the sharing code is loaded — not disabled, absent. The server
	list is what makes the feature work across networks, and it is the
	user's list: add, remove, bring your own TURN, or clear it entirely for
	LAN-only sharing that contacts nothing outside.
-->
<script lang="ts">
	import Toggle from '$lib/components/shared/Toggle.svelte';
	import { t } from '$lib/state/i18n.svelte';
	import { addToast } from '$lib/state/toasts.svelte';
	import * as share from '$lib/state/share.svelte';
	import type { IceServer } from '$lib/ipc/share';

	let config = $derived(share.getConfig());

	/** Editable copy. Saved as a whole, so a half-typed entry is never live. */
	let draft = $state<IceServer[]>([]);
	let dirty = $state(false);
	let saving = $state(false);
	let error = $state<string | null>(null);

	// Re-seed whenever the stored list changes underneath (load, reset).
	$effect(() => {
		draft = config.iceServers.map((s) => ({
			urls: [...s.urls],
			username: s.username,
			credential: s.credential
		}));
		dirty = false;
	});

	const isTurn = (s: IceServer) => s.urls.some((u) => /^turns?:/i.test(u.trim()));

	function addStun(): void {
		draft = [...draft, { urls: ['stun:'] }];
		dirty = true;
	}

	function addTurn(): void {
		draft = [...draft, { urls: ['turn:'], username: '', credential: '' }];
		dirty = true;
	}

	function remove(i: number): void {
		draft = draft.filter((_, j) => j !== i);
		dirty = true;
	}

	function touch(): void {
		dirty = true;
		error = null;
	}

	async function save(): Promise<void> {
		saving = true;
		error = null;
		try {
			await share.setIceServers(
				draft.map((s) => ({
					urls: s.urls.map((u) => u.trim()).filter(Boolean),
					username: s.username?.trim() || undefined,
					credential: s.credential?.trim() || undefined
				}))
			);
			addToast(t('sharing.saved'), 'success');
		} catch (e) {
			error = String(e);
		} finally {
			saving = false;
		}
	}

	async function reset(): Promise<void> {
		await share.resetIceServers();
		addToast(t('sharing.reset_done'), 'success');
	}

	async function toggle(on: boolean): Promise<void> {
		try {
			await share.setEnabled(on);
		} catch (e) {
			addToast(String(e), 'error');
		}
	}
</script>

<div class="tab-content">
	<div class="setting-row">
		<div class="setting-info">
			<span class="setting-label">{t('sharing.enable')}</span>
			<span class="setting-description">{t('sharing.enable_desc')}</span>
		</div>
		<div class="setting-control">
			<Toggle checked={config.enabled} label={t('sharing.enable')} onchange={toggle} />
		</div>
	</div>

	{#if config.enabled}
		<section class="ice">
			<h3 class="section-title">{t('sharing.ice_title')}</h3>
			<p class="section-desc">{t('sharing.ice_desc')}</p>

			{#if draft.length === 0}
				<p class="empty">{t('sharing.ice_empty')}</p>
			{/if}

			<ul class="servers">
				{#each draft as server, i (i)}
					<li class="server" class:turn={isTurn(server)}>
						<div class="server-head">
							<span class="kind">{isTurn(server) ? 'TURN' : 'STUN'}</span>
							<button class="mini danger" onclick={() => remove(i)}>{t('sharing.remove')}</button>
						</div>
						<label class="field">
							<span>{t('sharing.urls')}</span>
							<input
								type="text"
								value={server.urls.join(', ')}
								placeholder={isTurn(server) ? 'turn:relay.example.com:3478' : 'stun:stun.example.com:3478'}
								spellcheck="false"
								oninput={(e) => {
									server.urls = e.currentTarget.value.split(',').map((u) => u.trim());
									touch();
								}}
							/>
						</label>
						{#if isTurn(server)}
							<div class="creds">
								<label class="field">
									<span>{t('sharing.username')}</span>
									<input type="text" bind:value={server.username} oninput={touch} spellcheck="false" autocomplete="off" />
								</label>
								<label class="field">
									<span>{t('sharing.credential')}</span>
									<input type="password" bind:value={server.credential} oninput={touch} autocomplete="new-password" />
								</label>
							</div>
						{/if}
					</li>
				{/each}
			</ul>

			<div class="actions">
				<button class="mini" onclick={addStun}>{t('sharing.add_stun')}</button>
				<button class="mini" onclick={addTurn}>{t('sharing.add_turn')}</button>
				<span class="spacer"></span>
				<button class="mini" onclick={reset}>{t('sharing.reset')}</button>
				<button class="mini primary" disabled={!dirty || saving} onclick={save}>{t('common.save')}</button>
			</div>

			{#if error}
				<p class="error">{error}</p>
			{/if}

			<p class="note">{t('sharing.stun_note')}</p>
			<p class="note">{t('sharing.turn_note')}</p>
		</section>
	{/if}
</div>

<style>
	.tab-content {
		display: flex;
		flex-direction: column;
	}

	.setting-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 24px;
		padding: 14px 0;
		border-bottom: 1px solid var(--color-border);
	}

	.setting-info {
		display: flex;
		flex-direction: column;
		gap: 2px;
		min-width: 0;
	}

	.setting-label {
		font-size: 0.875rem;
		font-weight: 500;
		color: var(--color-text-primary);
	}

	.setting-description {
		font-size: 0.75rem;
		line-height: 1.45;
		color: var(--color-text-secondary);
	}

	.setting-control {
		flex-shrink: 0;
	}

	.ice {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		padding-top: 14px;
	}

	.section-title {
		margin: 0;
		font-size: var(--text-2xs);
		font-weight: 600;
		letter-spacing: 0.06em;
		text-transform: uppercase;
		color: var(--color-text-tertiary);
	}

	.section-desc,
	.note,
	.empty {
		margin: 0;
		font-size: var(--text-xs);
		line-height: 1.5;
		color: var(--color-text-secondary);
	}

	.note {
		color: var(--color-text-tertiary);
	}

	.servers {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.server {
		display: flex;
		flex-direction: column;
		gap: 6px;
		padding: var(--space-2) var(--space-3);
		background: var(--color-surface-sunken);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-sm);
	}

	.server.turn {
		border-color: color-mix(in srgb, var(--color-accent) 40%, var(--color-border));
	}

	.server-head {
		display: flex;
		align-items: center;
		justify-content: space-between;
	}

	.kind {
		font-size: var(--text-2xs);
		font-weight: 700;
		letter-spacing: 0.06em;
		color: var(--color-text-tertiary);
	}

	.field {
		display: flex;
		flex-direction: column;
		gap: 3px;
		font-size: var(--text-2xs);
		color: var(--color-text-tertiary);
	}

	.field input {
		padding: 5px 8px;
		font-family: var(--font-mono);
		font-size: var(--text-xs);
		color: var(--color-text-primary);
		background: var(--color-bg-primary);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-sm);
	}

	.creds {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: var(--space-2);
	}

	.actions {
		display: flex;
		align-items: center;
		gap: 6px;
	}

	.spacer {
		flex: 1;
	}

	.mini {
		padding: 4px 10px;
		font-family: inherit;
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
		background: transparent;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-sm);
		cursor: pointer;
	}

	.mini:hover:not(:disabled) {
		color: var(--color-text-primary);
	}

	.mini:disabled {
		opacity: 0.45;
		cursor: not-allowed;
	}

	.mini.primary {
		color: var(--color-accent);
		border-color: color-mix(in srgb, var(--color-accent) 50%, var(--color-border));
	}

	.mini.danger:hover {
		color: var(--color-danger);
		border-color: var(--color-danger);
	}

	.error {
		margin: 0;
		font-size: var(--text-xs);
		color: var(--color-danger);
	}
</style>
