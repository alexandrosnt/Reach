<script lang="ts">
	/**
	 * Who is connected and what they are running, refreshed every few
	 * seconds while visible. Stopping a statement and ending a session are
	 * separate buttons because they are separate decisions: one cancels a
	 * query, the other drops someone's connection.
	 */
	import { onDestroy, onMount } from 'svelte';
	import FaIcon from '$lib/components/shared/FaIcon.svelte';
	import { faRotate, faPause, faPlay } from '@fortawesome/free-solid-svg-icons';
	import * as db from '$lib/ipc/db';
	import type { ServerInfo, ServerSession } from '$lib/ipc/db';
	import * as st from '$lib/state/db.svelte';
	import { addToast } from '$lib/state/toasts.svelte';
	import { t } from '$lib/state/i18n.svelte';

	interface Props {
		tab: Extract<st.DbTab, { kind: 'monitor' }>;
		active: boolean;
	}

	let { tab, active }: Props = $props();

	let info = $state<ServerInfo | null>(null);
	let rows = $state<ServerSession[]>([]);
	let auto = $state(true);
	let showIdle = $state(false);
	let error = $state<string | null>(null);
	let timer: ReturnType<typeof setInterval> | undefined;

	let engine = $derived(st.engineOf(tab.connId));
	let visible = $derived(showIdle ? rows : rows.filter((r) => !isIdle(r)));

	function isIdle(r: ServerSession): boolean {
		const s = (r.state ?? '').toLowerCase();
		return !r.query || s === 'idle' || s === 'sleep' || s === 'sleeping';
	}

	async function refresh(): Promise<void> {
		try {
			rows = await db.sessions(tab.connId);
			error = null;
		} catch (e) {
			error = String(e);
		}
	}

	onMount(async () => {
		const open = st.getOpen(tab.connId);
		if (open) {
			try {
				info = await db.serverInfo(tab.connId, open.defaultDatabase);
			} catch {
				info = null;
			}
		}
		refresh();
		timer = setInterval(() => {
			if (auto && active) refresh();
		}, 3000);
	});

	onDestroy(() => clearInterval(timer));

	async function kill(r: ServerSession, onlyQuery: boolean): Promise<void> {
		const question = onlyQuery ? t('db.cancel_query_confirm', { id: r.id }) : t('db.kill_session_confirm', { id: r.id, user: r.user ?? '?' });
		if (!confirm(question)) return;
		try {
			await db.kill(tab.connId, Number(r.id), onlyQuery);
			addToast(onlyQuery ? t('db.query_cancelled') : t('db.session_ended'), 'success');
			refresh();
		} catch (e) {
			addToast(String(e), 'error', 6000);
		}
	}

	function duration(s: number | null): string {
		if (s === null || s === undefined) return '';
		if (s < 60) return `${s}s`;
		if (s < 3600) return `${Math.floor(s / 60)}m ${s % 60}s`;
		return `${Math.floor(s / 3600)}h ${Math.floor((s % 3600) / 60)}m`;
	}
</script>

<div class="monitor">
	<div class="toolbar">
		{#if info}
			<span class="version" title={info.version}>{info.version}</span>
		{/if}
		<span class="spacer"></span>
		<label class="idle"><input type="checkbox" bind:checked={showIdle} /> {t('db.show_idle')}</label>
		<button class="btn" onclick={() => (auto = !auto)} title={auto ? t('db.pause') : t('db.resume')}>
			<FaIcon icon={auto ? faPause : faPlay} />
		</button>
		<button class="btn" onclick={refresh} title={t('db.refresh')}><FaIcon icon={faRotate} /></button>
	</div>

	{#if error}<div class="error">{error}</div>{/if}

	<div class="scroll">
		<table>
			<thead>
				<tr>
					<th>{t('db.session_id')}</th>
					<th>{t('db.user')}</th>
					<th>{t('db.database')}</th>
					<th>{t('db.client')}</th>
					<th>{t('db.state')}</th>
					<th>{t('db.time')}</th>
					<th class="q">{t('db.query')}</th>
					<th></th>
				</tr>
			</thead>
			<tbody>
				{#each visible as r (r.id)}
					<tr class:self={r.isSelf} class:long={(r.seconds ?? 0) > 30 && !isIdle(r)}>
						<td class="mono">{r.id}</td>
						<td>{r.user ?? ''}</td>
						<td>{r.database ?? ''}</td>
						<td class="mono">{r.client ?? ''}</td>
						<td>{r.state ?? ''}</td>
						<td class="mono">{duration(r.seconds)}</td>
						<td class="q mono" title={r.query ?? ''}>{r.query ?? ''}</td>
						<td class="actions">
							{#if r.isSelf}
								<span class="muted">{t('db.reach_itself')}</span>
							{:else}
								{#if engine !== 'mssql' && !isIdle(r)}
									<button class="link" onclick={() => kill(r, true)}>{t('db.cancel_query')}</button>
								{/if}
								<button class="link danger" onclick={() => kill(r, false)}>{t('db.kill_session')}</button>
							{/if}
						</td>
					</tr>
				{:else}
					<tr><td colspan="8" class="none">{showIdle ? t('db.no_sessions') : t('db.no_active_sessions')}</td></tr>
				{/each}
			</tbody>
		</table>
	</div>
</div>

<style>
	.monitor {
		display: flex;
		flex-direction: column;
		height: 100%;
		min-height: 0;
	}

	.toolbar {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 6px 10px;
		background: var(--color-bg-elevated);
		border-bottom: 1px solid var(--color-border);
		font-size: var(--text-xs);
	}

	.version {
		color: var(--color-text-secondary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		max-width: 60%;
	}

	.idle {
		display: flex;
		align-items: center;
		gap: 5px;
		color: var(--color-text-secondary);
	}

	.btn {
		padding: 4px 8px;
		border-radius: var(--radius-btn);
		border: 1px solid var(--color-border);
		background: transparent;
		color: var(--color-text-primary);
		cursor: pointer;
	}

	.spacer {
		flex: 1;
	}

	.error {
		padding: 8px 12px;
		color: var(--color-danger);
		font-size: var(--text-xs);
	}

	.scroll {
		flex: 1;
		overflow: auto;
	}

	table {
		width: 100%;
		border-collapse: collapse;
		font-size: var(--text-xs);
	}

	th {
		position: sticky;
		top: 0;
		padding: 6px 8px;
		background: var(--color-bg-elevated);
		border-bottom: 1px solid var(--color-border);
		color: var(--color-text-secondary);
		text-align: left;
		white-space: nowrap;
	}

	td {
		padding: 5px 8px;
		border-bottom: 1px solid color-mix(in srgb, var(--color-border) 60%, transparent);
		color: var(--color-text-primary);
		white-space: nowrap;
	}

	td.q {
		max-width: 420px;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	tr.self td {
		color: var(--color-text-tertiary);
	}

	tr.long td:nth-child(6) {
		color: #ff9f0a;
		font-weight: 600;
	}

	.mono {
		font-family: var(--font-mono);
	}

	.actions {
		text-align: right;
	}

	.link {
		border: none;
		background: none;
		color: var(--color-accent);
		cursor: pointer;
		font-size: var(--text-xs);
	}

	.link.danger {
		color: var(--color-danger);
	}

	.muted {
		color: var(--color-text-tertiary);
	}

	.none {
		text-align: center;
		color: var(--color-text-tertiary);
		padding: 20px;
	}
</style>
