<script lang="ts">
	/**
	 * Redis: a key browser, a value editor per type, a console, server info
	 * and clients. Keys are walked with SCAN a page at a time, so a server with
	 * millions of keys never stalls; values are read with a cap and say so.
	 * Every edit is a single Redis command sent as arguments, so a value with
	 * spaces or quotes needs no escaping.
	 */
	import { onMount, tick } from 'svelte';
	import FaIcon from '$lib/components/shared/FaIcon.svelte';
	import { faRotate, faPlus, faTrash, faMagnifyingGlass, faFloppyDisk } from '@fortawesome/free-solid-svg-icons';
	import * as db from '$lib/ipc/db';
	import type { KeyInfo, KeyValue, Reply } from '$lib/ipc/db';
	import * as st from '$lib/state/db.svelte';
	import { addToast } from '$lib/state/toasts.svelte';
	import { t } from '$lib/state/i18n.svelte';

	interface Props {
		tab: Extract<st.DbTab, { kind: 'redis' }>;
	}

	let { tab }: Props = $props();

	type View = 'keys' | 'console' | 'info' | 'clients';

	let view = $state<View>('keys');
	let dbs = $state<[number, number][]>([]);
	let dbIndex = $state(0);
	let pattern = $state('*');
	let keys = $state<KeyInfo[]>([]);
	let cursor = $state('0');
	let scanning = $state(false);
	let selectedKey = $state<string | null>(null);
	let value = $state<KeyValue | null>(null);
	let draftString = $state('');
	let ttlInput = $state('');
	let newKey = $state<{ kind: string; key: string; a: string; b: string } | null>(null);
	// The "add an item" row under each collection.
	let addA = $state('');
	let addB = $state('');
	let renameTo = $state('');

	let lines = $state<{ cmd: string; reply: Reply }[]>([]);
	let consoleInput = $state('');
	let consoleHistory = $state<string[]>([]);
	let historyAt = $state(-1);
	let consoleEnd = $state<HTMLDivElement>();

	let info = $state<[string, [string, string][]][]>([]);
	let clients = $state<Record<string, string>[]>([]);

	let conn = $derived(st.getConnection(tab.connId));
	let readOnly = $derived(!!conn?.readOnly);

	onMount(async () => {
		dbIndex = Number(st.getOpen(tab.connId)?.defaultDatabase ?? 0) || 0;
		try {
			dbs = await db.redisDatabases(tab.connId);
		} catch {
			dbs = [[dbIndex, 0]];
		}
		scan(true);
	});

	async function scan(reset: boolean): Promise<void> {
		if (scanning) return;
		scanning = true;
		try {
			// A sparse keyspace can return empty pages; keep going until
			// something turns up or the walk ends.
			let next = reset ? '0' : cursor;
			const found: KeyInfo[] = [];
			let rounds = 0;
			do {
				const page = await db.redisScan(tab.connId, dbIndex, pattern.trim() || '*', next, 500);
				found.push(...page.keys);
				next = page.cursor;
				rounds++;
			} while (next !== '0' && found.length < 200 && rounds < 20);
			keys = reset ? found : [...keys, ...found];
			cursor = next;
		} catch (e) {
			addToast(String(e), 'error', 6000);
		} finally {
			scanning = false;
		}
	}

	async function openKey(key: string): Promise<void> {
		selectedKey = key;
		addA = '';
		addB = '';
		renameTo = key;
		try {
			value = await db.redisGet(tab.connId, dbIndex, key);
			draftString = value.value.kind === 'string' ? value.value.value : '';
			ttlInput = value.ttlMs !== null ? String(Math.ceil(value.ttlMs / 1000)) : '';
		} catch (e) {
			addToast(String(e), 'error');
		}
	}

	/** Run one command; ask first when the backend holds it back. */
	async function cmd(args: string[], quiet = false): Promise<Reply | null> {
		try {
			let out = await db.redisCommand(tab.connId, dbIndex, { args }, false);
			if (out.status === 'confirm') {
				if (!confirm(`${out.reason}.\n\n${t('db.run_anyway')}?`)) return null;
				out = await db.redisCommand(tab.connId, dbIndex, { args }, true);
			}
			if (out.status === 'done') {
				if (out.reply.isError) addToast(out.reply.text, 'error', 6000);
				else if (!quiet) addToast(t('db.done'), 'success', 1500);
				return out.reply;
			}
		} catch (e) {
			addToast(String(e), 'error', 6000);
		}
		return null;
	}

	async function refreshKey(): Promise<void> {
		if (selectedKey) await openKey(selectedKey);
	}

	async function saveString(): Promise<void> {
		if (!value) return;
		if (await cmd(['SET', value.key, draftString, 'KEEPTTL'])) refreshKey();
	}

	async function setTtl(): Promise<void> {
		if (!value) return;
		const s = parseInt(ttlInput, 10);
		const reply = Number.isFinite(s) && s > 0 ? await cmd(['EXPIRE', value.key, String(s)]) : await cmd(['PERSIST', value.key]);
		if (reply) refreshKey();
	}

	async function deleteKey(): Promise<void> {
		if (!value || !confirm(t('db.delete_key_confirm', { key: value.key }))) return;
		if (await cmd(['DEL', value.key])) {
			keys = keys.filter((k) => k.key !== value!.key);
			value = null;
			selectedKey = null;
		}
	}

	async function renameKey(): Promise<void> {
		if (!value) return;
		const next = renameTo.trim();
		if (!next || next === value.key) return;
		if (await cmd(['RENAME', value.key, next])) {
			keys = keys.map((k) => (k.key === value!.key ? { ...k, key: next } : k));
			openKey(next);
		}
	}

	async function editItem(args: string[]): Promise<void> {
		if (await cmd(args, true)) refreshKey();
	}

	async function addItem(): Promise<void> {
		if (!value) return;
		const k = value.key;
		const args =
			value.value.kind === 'hash' ? (addA ? ['HSET', k, addA, addB] : null) :
			value.value.kind === 'list' ? ['RPUSH', k, addA] :
			value.value.kind === 'set' ? (addA ? ['SADD', k, addA] : null) :
			value.value.kind === 'zset' ? (addA ? ['ZADD', k, addB || '0', addA] : null) :
			value.value.kind === 'stream' ? (addA ? ['XADD', k, '*', addA, addB] : null) :
			null;
		if (args) await editItem(args);
	}

	async function createKey(): Promise<void> {
		if (!newKey || !newKey.key) return;
		const { kind, key, a, b } = newKey;
		const args =
			kind === 'string' ? ['SET', key, a] :
			kind === 'hash' ? ['HSET', key, a, b] :
			kind === 'list' ? ['RPUSH', key, a] :
			kind === 'set' ? ['SADD', key, a] :
			kind === 'zset' ? ['ZADD', key, b || '0', a] :
			['XADD', key, '*', a || 'field', b];
		if (await cmd(args)) {
			newKey = null;
			await scan(true);
			openKey(key);
		}
	}

	async function runConsole(): Promise<void> {
		const line = consoleInput.trim();
		if (!line) return;
		consoleHistory = [line, ...consoleHistory.filter((h) => h !== line)].slice(0, 100);
		historyAt = -1;
		consoleInput = '';
		try {
			let out = await db.redisCommand(tab.connId, dbIndex, { line }, false);
			if (out.status === 'confirm') {
				if (!confirm(`${out.reason}.\n\n${t('db.run_anyway')}?`)) {
					lines = [...lines, { cmd: line, reply: { text: t('db.not_run'), isError: true } }];
					return;
				}
				out = await db.redisCommand(tab.connId, dbIndex, { line }, true);
			}
			if (out.status === 'done') lines = [...lines, { cmd: line, reply: out.reply }];
		} catch (e) {
			lines = [...lines, { cmd: line, reply: { text: String(e), isError: true } }];
		}
		await tick();
		consoleEnd?.scrollIntoView({ block: 'end' });
	}

	function consoleKey(e: KeyboardEvent): void {
		if (e.key === 'Enter') runConsole();
		else if (e.key === 'ArrowUp' && consoleHistory.length) {
			historyAt = Math.min(consoleHistory.length - 1, historyAt + 1);
			consoleInput = consoleHistory[historyAt];
			e.preventDefault();
		} else if (e.key === 'ArrowDown') {
			historyAt = Math.max(-1, historyAt - 1);
			consoleInput = historyAt >= 0 ? consoleHistory[historyAt] : '';
			e.preventDefault();
		}
	}

	async function showView(v: View): Promise<void> {
		view = v;
		try {
			if (v === 'info') info = await db.redisInfo(tab.connId);
			if (v === 'clients') clients = await db.redisClients(tab.connId);
		} catch (e) {
			addToast(String(e), 'error');
		}
	}

	async function killClient(id: string): Promise<void> {
		if (!confirm(t('db.kill_client_confirm', { id }))) return;
		if (await cmd(['CLIENT', 'KILL', 'ID', id])) showView('clients');
	}

	function ttlText(ms: number | null): string {
		if (ms === null) return '';
		const s = Math.ceil(ms / 1000);
		if (s < 60) return `${s}s`;
		if (s < 3600) return `${Math.floor(s / 60)}m`;
		if (s < 86400) return `${Math.floor(s / 3600)}h`;
		return `${Math.floor(s / 86400)}d`;
	}

	const TYPE_COLORS: Record<string, string> = {
		string: '#30d158',
		hash: '#0a84ff',
		list: '#ff9f0a',
		set: '#bf5af2',
		zset: '#ff375f',
		stream: '#64d2ff'
	};
</script>

<div class="redis">
	<div class="toolbar">
		<select bind:value={dbIndex} onchange={() => { value = null; selectedKey = null; scan(true); }} title={t('db.redis_db')}>
			{#each dbs as [n, count] (n)}<option value={n}>db{n} ({count})</option>{/each}
		</select>
		<div class="views" role="tablist">
			{#each [['keys', t('db.keys')], ['console', t('db.console')], ['info', t('db.info')], ['clients', t('db.clients')]] as [id, label] (id)}
				<button class="vbtn" class:on={view === id} role="tab" aria-selected={view === id} onclick={() => showView(id as View)}>{label}</button>
			{/each}
		</div>
		<span class="spacer"></span>
		{#if readOnly}<span class="pill">{t('db.read_only')}</span>{/if}
	</div>

	{#if view === 'keys'}
		<div class="keys-view">
			<div class="keylist">
				<div class="search">
					<FaIcon icon={faMagnifyingGlass} />
					<input bind:value={pattern} placeholder="user:*" spellcheck="false" onkeydown={(e) => e.key === 'Enter' && scan(true)} />
					<button class="icon" onclick={() => scan(true)} title={t('db.refresh')}><FaIcon icon={faRotate} /></button>
					{#if !readOnly}
						<button class="icon" onclick={() => (newKey = { kind: 'string', key: '', a: '', b: '' })} title={t('db.new_key')}><FaIcon icon={faPlus} /></button>
					{/if}
				</div>
				<div class="list">
					{#each keys as k (k.key)}
						<button class="key" class:on={selectedKey === k.key} onclick={() => openKey(k.key)}>
							<span class="type" style:--c={TYPE_COLORS[k.kind] ?? '#8e8e93'}>{k.kind}</span>
							<span class="kname">{k.key}</span>
							{#if k.ttlMs !== null}<span class="ttl">{ttlText(k.ttlMs)}</span>{/if}
						</button>
					{:else}
						<p class="none">{scanning ? t('db.loading') : t('db.no_keys')}</p>
					{/each}
					{#if cursor !== '0'}
						<button class="more" onclick={() => scan(false)} disabled={scanning}>{scanning ? t('db.loading') : t('db.load_more')}</button>
					{/if}
				</div>
			</div>

			<div class="value">
				{#if newKey}
					<div class="newkey">
						<h3>{t('db.new_key')}</h3>
						<label>{t('db.type')}
							<select bind:value={newKey.kind}>
								{#each ['string', 'hash', 'list', 'set', 'zset', 'stream'] as k (k)}<option value={k}>{k}</option>{/each}
							</select>
						</label>
						<label>{t('db.key')}<input bind:value={newKey.key} spellcheck="false" /></label>
						<label>{newKey.kind === 'hash' || newKey.kind === 'stream' ? t('db.field') : newKey.kind === 'zset' ? t('db.member') : t('db.value')}<input bind:value={newKey.a} spellcheck="false" /></label>
						{#if newKey.kind === 'hash' || newKey.kind === 'stream' || newKey.kind === 'zset'}
							<label>{newKey.kind === 'zset' ? t('db.score') : t('db.value')}<input bind:value={newKey.b} spellcheck="false" /></label>
						{/if}
						<div class="row">
							<button class="btn" onclick={() => (newKey = null)}>{t('db.cancel')}</button>
							<button class="btn primary" onclick={createKey} disabled={!newKey.key}>{t('db.create')}</button>
						</div>
					</div>
				{:else if value}
					<div class="vhead">
						<span class="type" style:--c={TYPE_COLORS[value.value.kind] ?? '#8e8e93'}>{value.value.kind}</span>
						<input class="kname big" bind:value={renameTo} readonly={readOnly} title={t('db.rename_key')} spellcheck="false" onkeydown={(e) => e.key === 'Enter' && renameKey()} onblur={renameKey} />
						<span class="spacer"></span>
						<label class="ttlbox">
							TTL
							<input bind:value={ttlInput} placeholder="∞" disabled={readOnly} onkeydown={(e) => e.key === 'Enter' && setTtl()} />
							<button class="btn small" onclick={setTtl} disabled={readOnly}>{t('db.set')}</button>
						</label>
						<button class="icon" onclick={refreshKey} title={t('db.refresh')}><FaIcon icon={faRotate} /></button>
						{#if !readOnly}<button class="icon danger" onclick={deleteKey} title={t('db.delete')}><FaIcon icon={faTrash} /></button>{/if}
					</div>
					{#if value.truncated}<div class="note">{t('db.showing_first', { length: value.length.toLocaleString() })}</div>{/if}

					<div class="vbody">
						{#if value.value.kind === 'string'}
							<textarea bind:value={draftString} readonly={readOnly || value.value.binary} spellcheck="false"></textarea>
							{#if !readOnly && !value.value.binary}
								<div class="row"><button class="btn primary" onclick={saveString} disabled={draftString === value.value.value}><FaIcon icon={faFloppyDisk} /> {t('db.save')}</button></div>
							{/if}
						{:else if value.value.kind === 'hash'}
							{@const v = value.value}
							<table>
								<thead><tr><th>{t('db.field')}</th><th>{t('db.value')}</th><th></th></tr></thead>
								<tbody>
									{#each v.entries as [f, val] (f)}
										<tr>
											<td class="mono">{f}</td>
											<td><input value={val} readonly={readOnly} onchange={(e) => editItem(['HSET', value!.key, f, e.currentTarget.value])} /></td>
											<td>{#if !readOnly}<button class="icon danger" onclick={() => editItem(['HDEL', value!.key, f])}><FaIcon icon={faTrash} /></button>{/if}</td>
										</tr>
									{/each}
								</tbody>
							</table>

						{:else if value.value.kind === 'list'}
							{@const v = value.value}
							<table>
								<thead><tr><th>#</th><th>{t('db.value')}</th><th></th></tr></thead>
								<tbody>
									{#each v.items as item, i (i)}
										<tr>
											<td class="mono muted">{i}</td>
											<td><input value={item} readonly={readOnly} onchange={(e) => editItem(['LSET', value!.key, String(i), e.currentTarget.value])} /></td>
											<td>{#if !readOnly}<button class="icon danger" onclick={() => editItem(['LREM', value!.key, '1', item])}><FaIcon icon={faTrash} /></button>{/if}</td>
										</tr>
									{/each}
								</tbody>
							</table>

						{:else if value.value.kind === 'set'}
							{@const v = value.value}
							<table>
								<thead><tr><th>{t('db.member')}</th><th></th></tr></thead>
								<tbody>
									{#each v.members as m (m)}
										<tr>
											<td class="mono">{m}</td>
											<td>{#if !readOnly}<button class="icon danger" onclick={() => editItem(['SREM', value!.key, m])}><FaIcon icon={faTrash} /></button>{/if}</td>
										</tr>
									{/each}
								</tbody>
							</table>

						{:else if value.value.kind === 'zset'}
							{@const v = value.value}
							<table>
								<thead><tr><th>{t('db.member')}</th><th>{t('db.score')}</th><th></th></tr></thead>
								<tbody>
									{#each v.members as [m, score] (m)}
										<tr>
											<td class="mono">{m}</td>
											<td><input class="num" value={score} readonly={readOnly} onchange={(e) => editItem(['ZADD', value!.key, e.currentTarget.value, m])} /></td>
											<td>{#if !readOnly}<button class="icon danger" onclick={() => editItem(['ZREM', value!.key, m])}><FaIcon icon={faTrash} /></button>{/if}</td>
										</tr>
									{/each}
								</tbody>
							</table>

						{:else if value.value.kind === 'stream'}
							{@const v = value.value}
							<table>
								<thead><tr><th>ID</th><th>{t('db.fields')}</th><th></th></tr></thead>
								<tbody>
									{#each v.entries as [id, fields] (id)}
										<tr>
											<td class="mono">{id}</td>
											<td class="mono">{fields.map(([k, x]) => `${k}=${x}`).join('  ')}</td>
											<td>{#if !readOnly}<button class="icon danger" onclick={() => editItem(['XDEL', value!.key, id])}><FaIcon icon={faTrash} /></button>{/if}</td>
										</tr>
									{/each}
								</tbody>
							</table>
						{/if}
						{#if !readOnly && ['hash', 'list', 'set', 'zset', 'stream'].includes(value.value.kind)}
							{@const two = value.value.kind === 'hash' || value.value.kind === 'zset' || value.value.kind === 'stream'}
							<div class="additem">
								<input
									bind:value={addA}
									placeholder={value.value.kind === 'hash' || value.value.kind === 'stream' ? t('db.field') : value.value.kind === 'list' ? t('db.value') : t('db.member')}
									spellcheck="false"
									onkeydown={(e) => e.key === 'Enter' && addItem()}
								/>
								{#if two}
									<input
										bind:value={addB}
										placeholder={value.value.kind === 'zset' ? t('db.score') : t('db.value')}
										spellcheck="false"
										onkeydown={(e) => e.key === 'Enter' && addItem()}
									/>
								{/if}
								<button class="btn" onclick={addItem}><FaIcon icon={faPlus} /> {value.value.kind === 'list' ? t('db.append') : t('db.add')}</button>
							</div>
						{/if}
						{#if value.value.kind === 'none'}
							<p class="none">{t('db.key_gone')}</p>
						{:else if value.value.kind === 'other'}
							<p class="none">{t('db.type_not_shown', { type: value.value.typeName })}</p>
						{/if}
					</div>
				{:else}
					<p class="none">{t('db.pick_key')}</p>
				{/if}
			</div>
		</div>
	{:else if view === 'console'}
		<div class="console">
			<div class="out">
				{#each lines as l, i (i)}
					<div class="cmdline">› {l.cmd}</div>
					<pre class:err={l.reply.isError}>{l.reply.text}</pre>
				{:else}
					<p class="none">{t('db.console_hint')}</p>
				{/each}
				<div bind:this={consoleEnd}></div>
			</div>
			<div class="prompt">
				<span>db{dbIndex}›</span>
				<input bind:value={consoleInput} onkeydown={consoleKey} spellcheck="false" autocapitalize="off" placeholder="GET key" />
			</div>
		</div>
	{:else if view === 'info'}
		<div class="info">
			{#each info as [section, kv] (section)}
				<details open={section === 'Server' || section === 'Memory' || section === 'Keyspace'}>
					<summary>{section}</summary>
					<table>
						<tbody>
							{#each kv as [k, v] (k)}<tr><td class="muted">{k}</td><td class="mono">{v}</td></tr>{/each}
						</tbody>
					</table>
				</details>
			{/each}
		</div>
	{:else}
		<div class="info">
			<table>
				<thead><tr><th>ID</th><th>{t('db.client')}</th><th>{t('db.name')}</th><th>db</th><th>{t('db.time')}</th><th>{t('db.last_command')}</th><th></th></tr></thead>
				<tbody>
					{#each clients as c (c.id)}
						<tr>
							<td class="mono">{c.id}</td>
							<td class="mono">{c.addr}</td>
							<td>{c.name}</td>
							<td>{c.db}</td>
							<td class="mono">{c.age}s</td>
							<td class="mono">{c.cmd}</td>
							<td>{#if !readOnly && c['lib-name'] !== 'reach'}<button class="link danger" onclick={() => killClient(c.id)}>{t('db.kill_session')}</button>{/if}</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	{/if}
</div>

<style>
	.redis {
		display: flex;
		flex-direction: column;
		height: 100%;
		min-height: 0;
		font-size: var(--text-sm);
	}

	.toolbar {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 6px 8px;
		background: var(--color-bg-elevated);
		border-bottom: 1px solid var(--color-border);
	}

	select,
	input {
		padding: 4px 6px;
		border-radius: var(--radius-sm);
		border: 1px solid var(--color-border);
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		font-size: var(--text-xs);
	}

	.views {
		display: flex;
		gap: 2px;
	}

	.vbtn {
		padding: 4px 10px;
		border: none;
		border-radius: var(--radius-sm);
		background: transparent;
		color: var(--color-text-secondary);
		font-size: var(--text-xs);
		cursor: pointer;
	}

	.vbtn.on {
		background: var(--color-surface-hover);
		color: var(--color-text-primary);
	}

	.spacer {
		flex: 1;
	}

	.pill {
		padding: 1px 6px;
		border-radius: 3px;
		font-size: 10px;
		background: var(--color-surface-hover);
		color: var(--color-text-secondary);
	}

	.keys-view {
		flex: 1;
		display: flex;
		min-height: 0;
	}

	.keylist {
		width: 320px;
		display: flex;
		flex-direction: column;
		border-right: 1px solid var(--color-border);
		min-height: 0;
	}

	.search {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 6px 8px;
		border-bottom: 1px solid var(--color-border);
		color: var(--color-text-tertiary);
	}

	.search input {
		flex: 1;
		font-family: var(--font-mono);
	}

	.icon {
		border: none;
		background: none;
		color: var(--color-text-secondary);
		cursor: pointer;
		padding: 4px;
	}

	.icon.danger:hover {
		color: var(--color-danger);
	}

	.list {
		flex: 1;
		overflow: auto;
	}

	.key {
		display: flex;
		align-items: center;
		gap: 8px;
		width: 100%;
		padding: 5px 10px;
		border: none;
		background: transparent;
		color: var(--color-text-primary);
		text-align: left;
		cursor: pointer;
	}

	.key:hover {
		background: var(--color-surface-hover);
	}

	.key.on {
		background: color-mix(in srgb, var(--color-accent) 14%, transparent);
	}

	.type {
		flex-shrink: 0;
		min-width: 44px;
		padding: 0 5px;
		border-radius: 3px;
		font-size: 10px;
		font-weight: 600;
		text-align: center;
		background: color-mix(in srgb, var(--c) 20%, transparent);
		color: var(--c);
	}

	.kname {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-family: var(--font-mono);
		font-size: var(--text-xs);
	}

	.kname.big {
		flex: 0 1 auto;
		border: none;
		background: none;
		color: var(--color-text-primary);
		font-size: var(--text-sm);
		cursor: text;
		text-align: left;
	}

	.ttl {
		font-size: 10px;
		color: var(--color-text-tertiary);
	}

	.more {
		width: 100%;
		padding: 8px;
		border: none;
		background: transparent;
		color: var(--color-accent);
		cursor: pointer;
		font-size: var(--text-xs);
	}

	.value {
		flex: 1;
		display: flex;
		flex-direction: column;
		min-width: 0;
		min-height: 0;
	}

	.vhead {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 8px 10px;
		border-bottom: 1px solid var(--color-border);
	}

	.ttlbox {
		display: flex;
		align-items: center;
		gap: 5px;
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
	}

	.ttlbox input {
		width: 80px;
	}

	.note {
		padding: 5px 10px;
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
		background: color-mix(in srgb, #ffd60a 10%, transparent);
	}

	.vbody {
		flex: 1;
		overflow: auto;
		padding: 10px;
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.vbody textarea {
		flex: 1;
		min-height: 200px;
		padding: 8px;
		border-radius: var(--radius-sm);
		border: 1px solid var(--color-border);
		background: var(--color-surface-sunken, var(--color-bg-primary));
		color: var(--color-text-primary);
		font-family: var(--font-mono);
		font-size: var(--text-xs);
		resize: none;
	}

	table {
		width: 100%;
		border-collapse: collapse;
		font-size: var(--text-xs);
	}

	th {
		text-align: left;
		padding: 5px 6px;
		color: var(--color-text-secondary);
		border-bottom: 1px solid var(--color-border);
	}

	td {
		padding: 3px 6px;
		border-bottom: 1px solid color-mix(in srgb, var(--color-border) 60%, transparent);
		color: var(--color-text-primary);
	}

	td input {
		width: 100%;
		border-color: transparent;
		background: transparent;
		font-family: var(--font-mono);
	}

	td input:focus {
		border-color: var(--color-accent);
		background: var(--color-bg-primary);
		outline: none;
	}

	.num {
		text-align: right;
	}

	.mono {
		font-family: var(--font-mono);
	}

	.muted {
		color: var(--color-text-tertiary);
	}

	.btn {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		align-self: flex-start;
		padding: 4px 10px;
		border-radius: var(--radius-btn);
		border: 1px solid var(--color-border);
		background: transparent;
		color: var(--color-text-primary);
		font-size: var(--text-xs);
		cursor: pointer;
	}

	.btn.primary {
		background: var(--color-accent);
		border-color: var(--color-accent);
		color: white;
	}

	.btn:disabled {
		opacity: 0.45;
	}

	.btn.small {
		padding: 2px 8px;
	}

	.row {
		display: flex;
		gap: 8px;
	}

	.none {
		padding: 20px;
		text-align: center;
		color: var(--color-text-tertiary);
	}

	.additem {
		display: flex;
		gap: 6px;
		align-items: center;
	}

	.additem input {
		flex: 1;
		font-family: var(--font-mono);
	}

	.newkey {
		display: flex;
		flex-direction: column;
		gap: 10px;
		padding: 16px;
		max-width: 420px;
	}

	.newkey h3 {
		margin: 0;
		font-size: var(--text-sm);
	}

	.newkey label {
		display: flex;
		flex-direction: column;
		gap: 4px;
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
	}

	.console {
		flex: 1;
		display: flex;
		flex-direction: column;
		min-height: 0;
		background: var(--color-surface-sunken, var(--color-bg-primary));
		font-family: var(--font-mono);
	}

	.out {
		flex: 1;
		overflow: auto;
		padding: 10px;
		font-size: var(--text-xs);
	}

	.cmdline {
		color: var(--color-accent);
		margin-top: 6px;
	}

	.out pre {
		margin: 2px 0 0;
		white-space: pre-wrap;
		word-break: break-word;
		color: var(--color-text-primary);
	}

	.out pre.err {
		color: var(--color-danger);
	}

	.prompt {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 8px 10px;
		border-top: 1px solid var(--color-border);
		color: var(--color-text-secondary);
		font-size: var(--text-xs);
	}

	.prompt input {
		flex: 1;
		border: none;
		background: transparent;
		font-family: var(--font-mono);
		font-size: var(--text-sm);
		outline: none;
	}

	.info {
		flex: 1;
		overflow: auto;
		padding: 10px;
	}

	details {
		margin-bottom: 8px;
	}

	summary {
		cursor: pointer;
		font-weight: 600;
		padding: 4px 0;
	}

	.link {
		border: none;
		background: none;
		cursor: pointer;
		font-size: var(--text-xs);
	}

	.link.danger {
		color: var(--color-danger);
	}

	@media (max-width: 720px) {
		.keys-view {
			flex-direction: column;
		}

		.keylist {
			width: 100%;
			max-height: 40%;
			border-right: none;
			border-bottom: 1px solid var(--color-border);
		}
	}
</style>
