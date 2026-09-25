<script lang="ts">
	/**
	 * New or edited connection. The engine comes first because it decides
	 * which fields make sense; "how to reach it" comes next, since for most
	 * servers the answer is "through a session you already have".
	 */
	import { onMount } from 'svelte';
	import { open as openDialog } from '@tauri-apps/plugin-dialog';
	import Modal from '$lib/components/shared/Modal.svelte';
	import Button from '$lib/components/shared/Button.svelte';
	import Toggle from '$lib/components/shared/Toggle.svelte';
	import EngineBadge from './EngineBadge.svelte';
	import { ENGINES, engineInfo } from '$lib/db/engines';
	import type { DbConnection, DbConnectionView, Engine, TlsMode } from '$lib/ipc/db';
	import { testConnection } from '$lib/ipc/db';
	import { sessionList, type SessionConfig } from '$lib/ipc/sessions';
	import * as dbState from '$lib/state/db.svelte';
	import { addToast } from '$lib/state/toasts.svelte';
	import { t } from '$lib/state/i18n.svelte';

	interface Props {
		open: boolean;
		/** Editing an existing one, or a starting point (a detected database). */
		initial?: Partial<DbConnectionView> | null;
		/** A detected database is connected without being saved unless asked. */
		allowConnectOnly?: boolean;
		onclose: () => void;
		onsaved?: (c: DbConnectionView) => void;
		onconnect?: (c: DbConnection) => void;
	}

	let { open, initial = null, allowConnectOnly = false, onclose, onsaved, onconnect }: Props = $props();

	const COLORS = ['', '#30d158', '#0a84ff', '#ffd60a', '#ff9f0a', '#ff453a', '#bf5af2'];

	let form = $state<DbConnection>(blank('postgres'));
	let password = $state('');
	let hasStoredPassword = $state(false);
	let sessions = $state<SessionConfig[]>([]);
	let testing = $state(false);
	let saving = $state(false);
	let testResult = $state<{ ok: boolean; text: string } | null>(null);

	function blank(engine: Engine): DbConnection {
		const info = engineInfo(engine);
		return {
			id: '',
			name: '',
			engine,
			host: engine === 'sqlite' ? '' : '127.0.0.1',
			port: info.port,
			username: info.defaultUser,
			password: null,
			database: null,
			filePath: null,
			route: { kind: 'direct' },
			tls: 'prefer',
			color: null,
			readOnly: false,
			production: false,
			lastUsedAt: 0
		};
	}

	$effect(() => {
		if (!open) return;
		const base = blank(initial?.engine ?? 'postgres');
		form = { ...base, ...(initial ?? {}), password: null } as DbConnection;
		hasStoredPassword = !!initial?.hasPassword;
		password = (initial?.password as string | undefined) ?? '';
		testResult = null;
	});

	onMount(async () => {
		try {
			sessions = (await sessionList()).filter((s) => (s.kind ?? 'ssh') === 'ssh');
		} catch {
			sessions = [];
		}
	});

	let isEdit = $derived(!!initial?.id && !allowConnectOnly);
	let isSqlite = $derived(form.engine === 'sqlite');
	let isRedis = $derived(form.engine === 'redis');

	function pickEngine(e: Engine): void {
		const info = engineInfo(e);
		const prevDefault = engineInfo(form.engine);
		form.engine = e;
		if (!form.port || form.port === prevDefault.port) form.port = info.port;
		if (!form.username || form.username === prevDefault.defaultUser) form.username = info.defaultUser;
		if (e === 'sqlite') form.route = { kind: 'direct' };
	}

	let routeValue = $derived(form.route.kind === 'session' ? `s:${form.route.sessionId}` : form.route.kind === 'live' ? 'live' : 'direct');

	function setRoute(v: string): void {
		if (v === 'direct') form.route = { kind: 'direct' };
		else if (v.startsWith('s:')) form.route = { kind: 'session', sessionId: v.slice(2) };
	}

	async function browseFile(): Promise<void> {
		const picked = await openDialog({
			multiple: false,
			filters: [{ name: 'SQLite', extensions: ['db', 'sqlite', 'sqlite3', 'db3'] }, { name: '*', extensions: ['*'] }]
		});
		if (typeof picked === 'string') {
			form.filePath = picked;
			if (!form.name) form.name = picked.split(/[\\/]/).pop() ?? '';
		}
	}

	function payload(): { conn: DbConnection; keep: boolean } {
		const conn: DbConnection = { ...form, password: password || null, name: form.name.trim() || suggestedName() };
		return { conn, keep: !password && hasStoredPassword };
	}

	function suggestedName(): string {
		if (isSqlite) return form.filePath?.split(/[\\/]/).pop() ?? 'SQLite';
		const host = form.route.kind === 'session' ? (sessions.find((s) => form.route.kind === 'session' && s.id === form.route.sessionId)?.name ?? form.host) : form.host;
		return `${engineInfo(form.engine).name} · ${host}`;
	}

	async function test(): Promise<void> {
		testing = true;
		testResult = null;
		try {
			const { conn, keep } = payload();
			const info = await testConnection(conn, keep);
			testResult = { ok: true, text: info.version };
		} catch (e) {
			testResult = { ok: false, text: String(e) };
		} finally {
			testing = false;
		}
	}

	async function submit(): Promise<void> {
		saving = true;
		try {
			const { conn, keep } = payload();
			const saved = await dbState.save(conn, keep);
			onsaved?.(saved);
			onclose();
		} catch (e) {
			addToast(String(e), 'error', 6000);
		} finally {
			saving = false;
		}
	}

	function connectOnly(): void {
		const { conn } = payload();
		onconnect?.(conn);
		onclose();
	}
</script>

<Modal {open} {onclose} title={isEdit ? t('db.edit_connection') : t('db.new_connection')} maxWidth="560px">
	{#snippet children()}
		<div class="form">
			<div class="engines" role="radiogroup" aria-label={t('db.engine')}>
				{#each ENGINES as e (e.id)}
					<button
						type="button"
						class="engine"
						class:selected={form.engine === e.id}
						role="radio"
						aria-checked={form.engine === e.id}
						onclick={() => pickEngine(e.id)}
					>
						<EngineBadge engine={e.id} size={22} />
						<span>{e.name}</span>
					</button>
				{/each}
			</div>

			<label class="field">
				<span>{t('db.name')}</span>
				<input bind:value={form.name} placeholder={suggestedName()} />
			</label>

			{#if isSqlite}
				<div class="field">
					<span>{t('db.file')}</span>
					<div class="row">
						<input bind:value={form.filePath} placeholder="/path/to/data.db" class="mono" />
						<Button variant="secondary" size="sm" onclick={browseFile}>{t('db.browse')}</Button>
					</div>
				</div>
			{:else}
				<label class="field">
					<span>{t('db.reach_it')}</span>
					<select value={routeValue} onchange={(e) => setRoute(e.currentTarget.value)}>
						<option value="direct">{t('db.route_direct')}</option>
						{#if form.route.kind === 'live'}
							<option value="live">{t('db.route_live')}</option>
						{/if}
						{#if sessions.length}
							<optgroup label={t('db.route_through_session')}>
								{#each sessions as s (s.id)}
									<option value="s:{s.id}">{s.name} ({s.username}@{s.host})</option>
								{/each}
							</optgroup>
						{/if}
					</select>
					{#if form.route.kind !== 'direct'}
						<small>{t('db.route_hint')}</small>
					{/if}
				</label>

				<div class="grid2">
					<label class="field grow">
						<span>{form.route.kind === 'direct' ? t('db.host') : t('db.host_from_server')}</span>
						<input bind:value={form.host} class="mono" autocapitalize="off" spellcheck="false" />
					</label>
					<label class="field port">
						<span>{t('db.port')}</span>
						<input type="number" bind:value={form.port} min="1" max="65535" />
					</label>
				</div>

				<div class="grid2">
					<label class="field grow">
						<span>{t('db.username')}</span>
						<input bind:value={form.username} autocapitalize="off" spellcheck="false" autocomplete="off" />
					</label>
					<label class="field grow">
						<span>{t('db.password')}</span>
						<input
							type="password"
							bind:value={password}
							autocomplete="new-password"
							placeholder={hasStoredPassword ? t('db.password_kept') : ''}
						/>
					</label>
				</div>

				<div class="grid2">
					<label class="field grow">
						<span>{isRedis ? t('db.redis_db') : t('db.database')}</span>
						<input bind:value={form.database} placeholder={isRedis ? '0' : t('db.database_default')} class="mono" />
					</label>
					<label class="field grow">
						<span>{t('db.tls')}</span>
						<select bind:value={form.tls}>
							<option value={'prefer' satisfies TlsMode}>{t('db.tls_prefer')}</option>
							<option value={'require' satisfies TlsMode}>{t('db.tls_require')}</option>
							<option value={'disable' satisfies TlsMode}>{t('db.tls_disable')}</option>
						</select>
					</label>
				</div>
			{/if}

			<div class="field">
				<span>{t('db.color')}</span>
				<div class="colors">
					{#each COLORS as c (c)}
						<button
							type="button"
							class="swatch"
							class:none={!c}
							class:selected={(form.color ?? '') === c}
							style:--c={c || 'transparent'}
							aria-label={c || t('db.color_none')}
							onclick={() => (form.color = c || null)}
						></button>
					{/each}
				</div>
			</div>

			<div class="toggles">
				<div class="toggle-row">
					<div>
						<strong>{t('db.read_only')}</strong>
						<small>{t('db.read_only_desc')}</small>
					</div>
					<Toggle hideLabel label={t('db.read_only')} checked={form.readOnly} onchange={(v) => (form.readOnly = v)} />
				</div>
				<div class="toggle-row">
					<div>
						<strong>{t('db.production')}</strong>
						<small>{t('db.production_desc')}</small>
					</div>
					<Toggle hideLabel label={t('db.production')} checked={form.production} onchange={(v) => (form.production = v)} />
				</div>
			</div>

			{#if testResult}
				<div class="test" class:ok={testResult.ok} role="status">
					{testResult.ok ? t('db.test_ok', { version: testResult.text }) : testResult.text}
				</div>
			{/if}
		</div>
	{/snippet}
	{#snippet actions()}
		<Button variant="ghost" onclick={test} disabled={testing}>{testing ? t('db.testing') : t('db.test')}</Button>
		<span class="spacer"></span>
		{#if allowConnectOnly}
			<Button variant="secondary" onclick={connectOnly}>{t('db.connect_once')}</Button>
		{/if}
		{#if !(allowConnectOnly && form.route.kind === 'live')}
			<Button variant="primary" onclick={submit} disabled={saving}>{isEdit ? t('db.save') : t('db.save_and_add')}</Button>
		{/if}
	{/snippet}
</Modal>

<style>
	.form {
		display: flex;
		flex-direction: column;
		gap: 14px;
	}

	.engines {
		display: grid;
		grid-template-columns: repeat(3, 1fr);
		gap: 6px;
	}

	.engine {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 8px 10px;
		border-radius: var(--radius-btn);
		border: 1px solid var(--color-border);
		background: transparent;
		color: var(--color-text-primary);
		font-size: var(--text-sm);
		cursor: pointer;
		text-align: left;
	}

	.engine:hover {
		background: var(--color-surface-hover);
	}

	.engine.selected {
		border-color: var(--color-accent);
		background: color-mix(in srgb, var(--color-accent) 12%, transparent);
	}

	.field {
		display: flex;
		flex-direction: column;
		gap: 5px;
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
		min-width: 0;
	}

	.field small {
		color: var(--color-text-tertiary);
		line-height: 1.4;
	}

	.field input,
	.field select {
		width: 100%;
		padding: 7px 9px;
		border-radius: var(--radius-sm);
		border: 1px solid var(--color-border);
		background: var(--color-surface-sunken, var(--color-bg-primary));
		color: var(--color-text-primary);
		font-size: var(--text-sm);
	}

	.field input:focus,
	.field select:focus {
		outline: none;
		border-color: var(--color-accent);
	}

	.mono {
		font-family: var(--font-mono);
	}

	.row {
		display: flex;
		gap: 8px;
		align-items: center;
	}

	.grid2 {
		display: flex;
		gap: 10px;
	}

	.grow {
		flex: 1;
	}

	.port {
		width: 100px;
	}

	.colors {
		display: flex;
		gap: 8px;
	}

	.swatch {
		width: 22px;
		height: 22px;
		border-radius: 50%;
		background: var(--c);
		border: 2px solid transparent;
		cursor: pointer;
		padding: 0;
	}

	.swatch.none {
		border: 2px dashed var(--color-border);
	}

	.swatch.selected {
		outline: 2px solid var(--color-text-primary);
		outline-offset: 2px;
	}

	.toggles {
		display: flex;
		flex-direction: column;
		border-top: 1px solid var(--color-border);
	}

	.toggle-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 16px;
		padding: 10px 0;
		border-bottom: 1px solid var(--color-border);
	}

	.toggle-row div {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.toggle-row strong {
		font-size: var(--text-sm);
		font-weight: 500;
		color: var(--color-text-primary);
	}

	.toggle-row small {
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
	}

	.test {
		padding: 8px 10px;
		border-radius: var(--radius-sm);
		font-size: var(--text-xs);
		background: color-mix(in srgb, var(--color-danger) 14%, transparent);
		color: var(--color-danger);
		word-break: break-word;
	}

	.test.ok {
		background: color-mix(in srgb, #30d158 14%, transparent);
		color: #30d158;
	}

	.spacer {
		flex: 1;
	}

	@media (max-width: 560px) {
		.engines {
			grid-template-columns: repeat(2, 1fr);
		}

		.grid2 {
			flex-direction: column;
		}

		.port {
			width: 100%;
		}
	}
</style>
