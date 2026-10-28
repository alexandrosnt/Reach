<script lang="ts">
	/**
	 * A shared vault joined twice is two entries over one database: every
	 * connection in it showed twice (issue #77). This asks, once per unlock,
	 * to merge each such vault back into one entry. Merging forgets the other
	 * entries on this device only; the connections live in the vault itself,
	 * so all of them stay. The entry kept is one that connected, since the two
	 * may hold different tokens and an expired one must not be what remains.
	 */
	import Modal from '#lib/components/shared/Modal.svelte';
	import Button from '#lib/components/shared/Button.svelte';
	import { vaultDuplicates, type VaultInfo } from '#lib/ipc/vault.js';
	import { vaultState, deleteVault, refreshVaults } from '#lib/state/vault.svelte.js';
	import { addToast } from '#lib/state/toasts.svelte.js';
	import { t } from '#lib/state/i18n.svelte.js';

	let groups = $state<VaultInfo[][]>([]);
	/** Per group, the entry to keep. */
	let keep = $state<string[]>([]);
	let merging = $state(false);
	let current = $derived(groups[0]);

	function defaultKeep(group: VaultInfo[]): string {
		return (group.find((v) => !v.unreachable) ?? group[0]).id;
	}

	async function check(): Promise<void> {
		try {
			const found = await vaultDuplicates();
			groups = found;
			keep = found.map(defaultKeep);
		} catch {
			// A check that cannot run is no reason to stand in the way.
		}
	}

	let wasOpen = false;
	$effect(() => {
		const open = !vaultState.locked && !vaultState.held;
		if (open && !wasOpen) void check();
		wasOpen = open;
	});

	function later(): void {
		groups = groups.slice(1);
		keep = keep.slice(1);
	}

	async function merge(): Promise<void> {
		const group = current;
		if (!group || merging) return;
		const kept = keep[0];
		merging = true;
		try {
			for (const v of group) {
				if (v.id !== kept) await deleteVault(v.id);
			}
			await refreshVaults();
			window.dispatchEvent(new CustomEvent('reach:sessions-changed'));
			addToast(t('vault.duplicate_merged', { name: group[0].name }), 'success');
			later();
		} catch (err) {
			addToast(String(err), 'error');
		} finally {
			merging = false;
		}
	}
</script>

{#if current}
	<Modal open={true} onclose={later} title={t('vault.duplicate_title', { name: current[0].name })} maxWidth="520px" zIndex={900}>
		<p class="msg">{t('vault.duplicate_message', { name: current[0].name, count: String(current.length) })}</p>

		<fieldset class="entries" disabled={merging}>
			<legend>{t('vault.duplicate_keep')}</legend>
			{#each current as entry, i (entry.id)}
				<label class="entry">
					<input type="radio" name="keep" value={entry.id} bind:group={keep[0]} />
					<span class="entry-name">{t('vault.duplicate_entry', { n: String(i + 1) })}</span>
					<span class="entry-state" class:bad={entry.unreachable}>
						{entry.unreachable
							? t('vault.duplicate_not_connecting')
							: t('vault.duplicate_items', { count: String(entry.secretCount) })}
					</span>
				</label>
			{/each}
		</fieldset>

		<p class="hint">{t('vault.duplicate_hint')}</p>

		{#snippet actions()}
			<Button variant="secondary" onclick={later} disabled={merging}>{t('vault.duplicate_later')}</Button>
			<Button variant="primary" onclick={merge} disabled={merging}>{t('vault.duplicate_merge')}</Button>
		{/snippet}
	</Modal>
{/if}

<style>
	.msg {
		margin: 0 0 14px;
		font-size: 0.875rem;
		line-height: 1.5;
		color: var(--color-text-primary);
	}

	.entries {
		margin: 0;
		padding: 0;
		border: none;
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.entries legend {
		margin-bottom: 6px;
		font-size: 0.75rem;
		color: var(--color-text-secondary);
	}

	.entry {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 10px 12px;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		background: var(--color-bg-elevated);
		cursor: pointer;
		font-size: 0.875rem;
	}

	.entry:has(input:checked) {
		border-color: var(--color-accent);
	}

	.entry-name {
		flex: 1;
		color: var(--color-text-primary);
	}

	.entry-state {
		font-size: 0.75rem;
		color: var(--color-success);
	}

	.entry-state.bad {
		color: var(--color-danger);
	}

	.hint {
		margin: 12px 0 0;
		font-size: 0.75rem;
		line-height: 1.5;
		color: var(--color-text-secondary);
	}
</style>
