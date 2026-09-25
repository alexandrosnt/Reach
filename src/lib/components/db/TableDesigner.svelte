<script lang="ts">
	/**
	 * The table designer, laid out as Navicat's: a toolbar, one tab per part of
	 * the table, a property panel under the field grid, and a live SQL preview.
	 *
	 * It edits a draft of what was loaded. Each part remembers its original
	 * name, so renaming a field keeps its data; the Rust side turns the
	 * difference into ALTER statements, and Save shows exactly those before
	 * anything runs.
	 */
	import { onMount } from 'svelte';
	import FaIcon from '$lib/components/shared/FaIcon.svelte';
	import { faFloppyDisk, faPlus, faArrowUp, faArrowDown, faTrash, faKey, faRotateLeft, faIndent } from '@fortawesome/free-solid-svg-icons';
	import SqlReviewDialog from './SqlReviewDialog.svelte';
	import * as db from '$lib/ipc/db';
	import type { ColumnDef, TableDesign, IndexDef, ForeignKeyDef, CheckDef, TriggerDef, Engine } from '$lib/ipc/db';
	import * as st from '$lib/state/db.svelte';
	import { engineInfo, FK_ACTIONS, indexMethods, mysqlEngines } from '$lib/db/engines';
	import { addToast } from '$lib/state/toasts.svelte';
	import { t } from '$lib/state/i18n.svelte';

	interface Props {
		tab: Extract<st.DbTab, { kind: 'design' }>;
	}

	let { tab }: Props = $props();

	type Part = 'fields' | 'indexes' | 'foreignKeys' | 'checks' | 'triggers' | 'options' | 'comment' | 'sql';

	let engine = $derived<Engine>(st.engineOf(tab.connId) ?? 'postgres');
	let info = $derived(engineInfo(engine));
	let mysql = $derived(engine === 'mysql' || engine === 'mariadb');

	let original = $state<TableDesign | null>(null);
	let draft = $state<TableDesign>(emptyTable());
	let part = $state<Part>('fields');
	let selected = $state(0);
	let selectedTrigger = $state(0);
	let loadError = $state<string | null>(null);
	let preview = $state<{ statements: string[]; error: string | null }>({ statements: [], error: null });
	let review = $state<string[] | null>(null);
	let applying = $state(false);
	let applyError = $state<string | null>(null);

	let dirty = $derived(JSON.stringify(draft) !== JSON.stringify(original ?? emptyTable()) || !original);

	$effect(() => {
		st.setDirty(tab.id, !!original && dirty);
	});

	function column(name = '', dataType = ''): ColumnDef {
		return {
			original: null, name, dataType, length: null, scale: null, notNull: false, default: null, autoIncrement: false,
			comment: null, collation: null, unsigned: false, onUpdate: null, generated: null, generatedStored: false, defaultConstraint: null
		};
	}

	function emptyTable(): TableDesign {
		const idType = engine === 'mysql' || engine === 'mariadb' || engine === 'mssql' ? 'int' : engine === 'sqlite' ? 'integer' : 'bigint';
		return {
			// MySQL's "schema" is the database; name it so the preview matches what runs.
			schema: tab.schema ?? (engine === 'mysql' || engine === 'mariadb' ? tab.database : null),
			originalName: null,
			name: '',
			columns: [{ ...column('id', idType), notNull: true, autoIncrement: true }],
			primaryKey: ['id'],
			primaryKeyName: null,
			indexes: [],
			foreignKeys: [],
			checks: [],
			triggers: [],
			comment: null,
			options: { engine: null, charset: null, collation: null, autoIncrement: null }
		};
	}

	const clone = <T,>(v: T): T => JSON.parse(JSON.stringify(v));

	onMount(async () => {
		if (!tab.table) {
			draft = emptyTable();
			if (mysql) draft.options.engine = 'InnoDB';
			return;
		}
		try {
			const d = await db.tableDesign(tab.connId, tab.database, tab.schema, tab.table);
			original = d;
			draft = clone(d);
		} catch (e) {
			loadError = String(e);
		}
	});

	// Live preview, a moment after typing stops.
	let timer: ReturnType<typeof setTimeout> | undefined;
	$effect(() => {
		const snapshot = JSON.stringify(draft);
		clearTimeout(timer);
		timer = setTimeout(async () => {
			if (!draft.name.trim()) {
				preview = { statements: [], error: t('db.designer_needs_name') };
				return;
			}
			try {
				const plan = await db.previewDesign(engine, original, JSON.parse(snapshot));
				preview = { statements: plan.statements, error: null };
			} catch (e) {
				preview = { statements: [], error: String(e) };
			}
		}, 250);
	});

	// --- Fields -------------------------------------------------------------

	let field = $derived(draft.columns[selected] as ColumnDef | undefined);

	function addField(at?: number): void {
		const i = at ?? draft.columns.length;
		draft.columns.splice(i, 0, column('', info.types[0] ?? 'text'));
		selected = i;
	}

	function deleteField(): void {
		const f = draft.columns[selected];
		if (!f) return;
		draft.columns.splice(selected, 1);
		draft.primaryKey = draft.primaryKey.filter((k) => k !== f.name);
		selected = Math.max(0, Math.min(selected, draft.columns.length - 1));
	}

	function moveField(dir: -1 | 1): void {
		const j = selected + dir;
		if (j < 0 || j >= draft.columns.length) return;
		[draft.columns[selected], draft.columns[j]] = [draft.columns[j], draft.columns[selected]];
		selected = j;
	}

	function togglePrimary(i = selected): void {
		const name = draft.columns[i]?.name;
		if (!name) return;
		draft.primaryKey = draft.primaryKey.includes(name) ? draft.primaryKey.filter((k) => k !== name) : [...draft.primaryKey, name];
		if (draft.primaryKey.includes(name)) draft.columns[i].notNull = true;
	}

	/** Keep key and index references in step with a field's new name. */
	function renameField(i: number, next: string): void {
		const prev = draft.columns[i].name;
		draft.columns[i].name = next;
		if (!prev) return;
		draft.primaryKey = draft.primaryKey.map((k) => (k === prev ? next : k));
		for (const ix of draft.indexes) for (const c of ix.columns) if (c.name === prev) c.name = next;
		for (const fk of draft.foreignKeys) fk.columns = fk.columns.map((c) => (c === prev ? next : c));
	}

	function numOrNull(v: string): number | null {
		const n = parseInt(v, 10);
		return Number.isFinite(n) ? n : null;
	}

	function textOrNull(v: string): string | null {
		return v.trim() === '' ? null : v;
	}

	// Engines that cannot reorder existing columns.
	let canMove = $derived(mysql || !original);

	// --- Indexes, keys, checks, triggers --------------------------------------

	function uniqueName(prefix: string, taken: string[]): string {
		const base = `${prefix}_${draft.name || 'table'}`;
		let n = 1;
		while (taken.includes(`${base}_${n}`)) n++;
		return `${base}_${n}`;
	}

	function addIndex(): void {
		draft.indexes.push({
			original: null,
			name: uniqueName('idx', draft.indexes.map((i) => i.name)),
			columns: field ? [{ name: field.name, descending: false, length: null }] : [],
			unique: false,
			constraint: false,
			method: null,
			whereClause: null,
			comment: null
		});
	}

	function indexColumnsText(ix: IndexDef): string {
		return ix.columns.map((c) => c.name + (c.length ? `(${c.length})` : '') + (c.descending ? ' DESC' : '')).join(', ');
	}

	function setIndexColumns(ix: IndexDef, text: string): void {
		ix.columns = text
			.split(',')
			.map((p) => p.trim())
			.filter(Boolean)
			.map((p) => {
				const desc = /\s+desc$/i.test(p);
				const bare = p.replace(/\s+(asc|desc)$/i, '');
				const m = bare.match(/^(.*?)\((\d+)\)$/);
				return { name: m ? m[1] : bare, length: m ? Number(m[2]) : null, descending: desc };
			});
	}

	function addForeignKey(): void {
		draft.foreignKeys.push({
			original: null,
			name: uniqueName('fk', draft.foreignKeys.map((f) => f.name)),
			columns: field ? [field.name] : [],
			refSchema: null,
			refTable: '',
			refColumns: ['id'],
			onDelete: null,
			onUpdate: null
		});
	}

	const list = (s: string) => s.split(',').map((x) => x.trim()).filter(Boolean);

	function addCheck(): void {
		draft.checks.push({ original: null, name: uniqueName('chk', draft.checks.map((c) => c.name)), expression: '' });
	}

	function triggerTemplate(name: string): string {
		const table = draft.name || 'table_name';
		switch (engine) {
			case 'postgres':
				return `CREATE TRIGGER ${name}\n  BEFORE UPDATE ON ${draft.schema ? draft.schema + '.' : ''}${table}\n  FOR EACH ROW\n  EXECUTE FUNCTION function_name();`;
			case 'mssql':
				return `CREATE TRIGGER ${draft.schema ?? 'dbo'}.${name}\nON ${draft.schema ?? 'dbo'}.${table}\nAFTER UPDATE\nAS\nBEGIN\n  SET NOCOUNT ON;\nEND`;
			case 'sqlite':
				return `CREATE TRIGGER ${name}\nAFTER UPDATE ON ${table}\nBEGIN\n  SELECT 1;\nEND`;
			default:
				return `CREATE TRIGGER \`${name}\` BEFORE UPDATE ON \`${table}\`\nFOR EACH ROW\nBEGIN\n  SET NEW.updated_at = NOW();\nEND`;
		}
	}

	function addTrigger(): void {
		const name = uniqueName('trg', draft.triggers.map((x) => x.name));
		draft.triggers.push({ original: null, name, definition: triggerTemplate(name) });
		selectedTrigger = draft.triggers.length - 1;
	}

	function removeAt<T>(arr: T[], i: number): void {
		arr.splice(i, 1);
	}

	// --- Save -----------------------------------------------------------------

	async function openReview(): Promise<void> {
		try {
			const plan = await db.previewDesign(engine, original, draft);
			if (!plan.statements.length) {
				addToast(t('db.designer_no_changes'), 'info');
				return;
			}
			applyError = null;
			review = plan.statements;
		} catch (e) {
			addToast(String(e), 'error', 6000);
		}
	}

	async function apply(): Promise<void> {
		applying = true;
		applyError = null;
		try {
			const fresh = await db.applyDesign(tab.connId, tab.database, original, draft);
			const created = !original;
			original = fresh;
			draft = clone(fresh);
			review = null;
			st.invalidate(tab.connId, tab.database);
			if (created) st.retarget(tab.id, fresh.name, `${fresh.name} · Design`);
			addToast(created ? t('db.table_created') : t('db.table_saved'), 'success');
		} catch (e) {
			applyError = String(e);
		} finally {
			applying = false;
		}
	}

	function revert(): void {
		draft = original ? clone(original) : emptyTable();
		selected = 0;
	}

	const PARTS: { id: Part; label: () => string; show: () => boolean; count?: () => number }[] = [
		{ id: 'fields', label: () => t('db.fields'), show: () => true, count: () => draft.columns.length },
		{ id: 'indexes', label: () => t('db.indexes'), show: () => true, count: () => draft.indexes.length },
		{ id: 'foreignKeys', label: () => t('db.foreign_keys'), show: () => true, count: () => draft.foreignKeys.length },
		{ id: 'checks', label: () => t('db.checks'), show: () => true, count: () => draft.checks.length },
		{ id: 'triggers', label: () => t('db.triggers'), show: () => true, count: () => draft.triggers.length },
		{ id: 'options', label: () => t('db.options'), show: () => mysql },
		{ id: 'comment', label: () => t('db.comment'), show: () => engine !== 'sqlite' },
		{ id: 'sql', label: () => t('db.sql_preview'), show: () => true }
	];
</script>

{#if loadError}
	<div class="load-error">{loadError}</div>
{:else}
	<div class="designer">
		<div class="toolbar">
			<label class="tname">
				<span>{t('db.table_name')}</span>
				<input bind:value={draft.name} placeholder="new_table" spellcheck="false" />
			</label>
			<button class="btn primary" onclick={openReview} disabled={!dirty || !draft.name.trim() || !!preview.error}>
				<FaIcon icon={faFloppyDisk} /> <span>{original ? t('db.save') : t('db.create_table')}</span>
			</button>
			<span class="sep"></span>
			{#if part === 'fields'}
				<button class="btn" onclick={() => addField()}><FaIcon icon={faPlus} /> <span>{t('db.add_field')}</span></button>
				<button class="btn" onclick={() => addField(selected)} disabled={!canMove} title={t('db.insert_field')}><FaIcon icon={faIndent} /></button>
				<button class="btn" onclick={deleteField} disabled={!field} title={t('db.delete_field')}><FaIcon icon={faTrash} /></button>
				<button class="btn" class:on={field && draft.primaryKey.includes(field.name)} onclick={() => togglePrimary()} disabled={!field} title={t('db.primary_key')}>
					<FaIcon icon={faKey} />
				</button>
				<button class="btn" onclick={() => moveField(-1)} disabled={!canMove || selected === 0} title={t('db.move_up')}><FaIcon icon={faArrowUp} /></button>
				<button class="btn" onclick={() => moveField(1)} disabled={!canMove || selected >= draft.columns.length - 1} title={t('db.move_down')}><FaIcon icon={faArrowDown} /></button>
			{:else if part === 'indexes'}
				<button class="btn" onclick={addIndex}><FaIcon icon={faPlus} /> <span>{t('db.add_index')}</span></button>
			{:else if part === 'foreignKeys'}
				<button class="btn" onclick={addForeignKey}><FaIcon icon={faPlus} /> <span>{t('db.add_foreign_key')}</span></button>
			{:else if part === 'checks'}
				<button class="btn" onclick={addCheck}><FaIcon icon={faPlus} /> <span>{t('db.add_check')}</span></button>
			{:else if part === 'triggers'}
				<button class="btn" onclick={addTrigger}><FaIcon icon={faPlus} /> <span>{t('db.add_trigger')}</span></button>
			{/if}
			<span class="spacer"></span>
			{#if original && dirty}
				<button class="btn ghost" onclick={revert}><FaIcon icon={faRotateLeft} /> <span>{t('db.revert')}</span></button>
			{/if}
		</div>

		<div class="parts" role="tablist">
			{#each PARTS.filter((p) => p.show()) as p (p.id)}
				<button class="part" class:on={part === p.id} role="tab" aria-selected={part === p.id} onclick={() => (part = p.id)}>
					{p.label()}
					{#if p.count && p.count() > 0}<span class="count">{p.count()}</span>{/if}
					{#if p.id === 'sql' && preview.statements.length}<span class="count accent">{preview.statements.length}</span>{/if}
				</button>
			{/each}
		</div>

		<div class="content">
			{#if part === 'fields'}
				<div class="fields">
					<div class="table-scroll">
						<table class="edit">
							<thead>
								<tr>
									<th class="w-name">{t('db.col_name')}</th>
									<th class="w-type">{t('db.col_type')}</th>
									<th class="w-num">{t('db.col_length')}</th>
									<th class="w-num">{t('db.col_decimals')}</th>
									<th class="w-chk">{t('db.col_not_null')}</th>
									<th class="w-chk">{t('db.col_key')}</th>
									{#if engine !== 'sqlite'}<th>{t('db.comment')}</th>{/if}
								</tr>
							</thead>
							<tbody>
								{#each draft.columns as c, i (i)}
									{@const pk = draft.primaryKey.indexOf(c.name)}
									<tr class:selected={selected === i} onclick={() => (selected = i)}>
										<td><input value={c.name} oninput={(e) => renameField(i, e.currentTarget.value)} spellcheck="false" onfocus={() => (selected = i)} /></td>
										<td>
											<input list="db-types-{tab.id}" bind:value={c.dataType} spellcheck="false" onfocus={() => (selected = i)} />
										</td>
										<td><input class="num" value={c.length ?? ''} oninput={(e) => (c.length = numOrNull(e.currentTarget.value))} disabled={!!c.generated && engine === 'mssql'} /></td>
										<td><input class="num" value={c.scale ?? ''} oninput={(e) => (c.scale = numOrNull(e.currentTarget.value))} /></td>
										<td class="center"><input type="checkbox" bind:checked={c.notNull} /></td>
										<td class="center">
											<button class="keybtn" class:on={pk >= 0} onclick={() => togglePrimary(i)} aria-label={t('db.primary_key')}>
												{#if pk >= 0}<FaIcon icon={faKey} />{#if draft.primaryKey.length > 1}<sub>{pk + 1}</sub>{/if}{/if}
											</button>
										</td>
										{#if engine !== 'sqlite'}
											<td><input value={c.comment ?? ''} oninput={(e) => (c.comment = textOrNull(e.currentTarget.value))} onfocus={() => (selected = i)} /></td>
										{/if}
									</tr>
								{/each}
							</tbody>
						</table>
						<datalist id="db-types-{tab.id}">
							{#each info.types as ty (ty)}<option value={ty}></option>{/each}
						</datalist>
					</div>

					{#if field}
						<div class="props">
							<div class="props-title">{field.name || t('db.new_field')}</div>
							<div class="props-grid">
								<label>
									<span>{t('db.default')}</span>
									<input value={field.default ?? ''} oninput={(e) => (field!.default = textOrNull(e.currentTarget.value))} placeholder={t('db.default_hint')} spellcheck="false" disabled={!!field.generated} />
								</label>
								<label class="check">
									<input type="checkbox" bind:checked={field.autoIncrement} />
									<span>{engine === 'postgres' ? t('db.identity') : t('db.auto_increment')}</span>
								</label>
								{#if mysql}
									<label class="check">
										<input type="checkbox" bind:checked={field.unsigned} />
										<span>{t('db.unsigned')}</span>
									</label>
									<label>
										<span>{t('db.on_update')}</span>
										<input value={field.onUpdate ?? ''} oninput={(e) => (field!.onUpdate = textOrNull(e.currentTarget.value))} placeholder="CURRENT_TIMESTAMP" />
									</label>
								{/if}
								{#if engine !== 'sqlite'}
									<label>
										<span>{t('db.collation')}</span>
										<input value={field.collation ?? ''} oninput={(e) => (field!.collation = textOrNull(e.currentTarget.value))} spellcheck="false" />
									</label>
								{/if}
								<label class="wide">
									<span>{t('db.generated')}</span>
									<input value={field.generated ?? ''} oninput={(e) => (field!.generated = textOrNull(e.currentTarget.value))} placeholder={t('db.generated_hint')} spellcheck="false" />
								</label>
								{#if field.generated && engine !== 'postgres'}
									<label class="check">
										<input type="checkbox" bind:checked={field.generatedStored} />
										<span>{t('db.stored')}</span>
									</label>
								{/if}
							</div>
						</div>
					{/if}
				</div>
			{:else if part === 'indexes'}
				<div class="table-scroll">
					<table class="edit">
						<thead>
							<tr>
								<th class="w-name">{t('db.col_name')}</th>
								<th>{t('db.fields')}</th>
								<th class="w-chk">{t('db.unique')}</th>
								<th class="w-type">{t('db.method')}</th>
								{#if !mysql}<th>{t('db.where')}</th>{/if}
								{#if mysql}<th>{t('db.comment')}</th>{/if}
								<th class="w-del"></th>
							</tr>
						</thead>
						<tbody>
							{#each draft.indexes as ix, i (i)}
								<tr>
									<td><input bind:value={ix.name} spellcheck="false" /></td>
									<td><input value={indexColumnsText(ix)} onchange={(e) => setIndexColumns(ix, e.currentTarget.value)} placeholder="email, created_at DESC" spellcheck="false" /></td>
									<td class="center"><input type="checkbox" bind:checked={ix.unique} /></td>
									<td>
										<select value={ix.method ?? ''} onchange={(e) => (ix.method = e.currentTarget.value || null)}>
											<option value="">{t('db.default_method')}</option>
											{#each indexMethods(engine) as m (m)}<option value={m}>{m}</option>{/each}
										</select>
									</td>
									{#if !mysql}<td><input value={ix.whereClause ?? ''} oninput={(e) => (ix.whereClause = textOrNull(e.currentTarget.value))} spellcheck="false" /></td>{/if}
									{#if mysql}<td><input value={ix.comment ?? ''} oninput={(e) => (ix.comment = textOrNull(e.currentTarget.value))} /></td>{/if}
									<td><button class="del" onclick={() => removeAt(draft.indexes, i)} aria-label={t('db.remove')}><FaIcon icon={faTrash} /></button></td>
								</tr>
							{:else}
								<tr><td colspan="7" class="none">{t('db.none_yet')}</td></tr>
							{/each}
						</tbody>
					</table>
				</div>
			{:else if part === 'foreignKeys'}
				<div class="table-scroll">
					<table class="edit">
						<thead>
							<tr>
								<th class="w-name">{t('db.col_name')}</th>
								<th>{t('db.fields')}</th>
								{#if st.hasSchemas(engine)}<th class="w-num2">{t('db.ref_schema')}</th>{/if}
								<th class="w-name">{t('db.ref_table')}</th>
								<th>{t('db.ref_fields')}</th>
								<th class="w-type">{t('db.on_delete')}</th>
								<th class="w-type">{t('db.on_update_fk')}</th>
								<th class="w-del"></th>
							</tr>
						</thead>
						<tbody>
							{#each draft.foreignKeys as fk, i (i)}
								<tr>
									<td><input bind:value={fk.name} spellcheck="false" /></td>
									<td><input value={fk.columns.join(', ')} onchange={(e) => (fk.columns = list(e.currentTarget.value))} spellcheck="false" /></td>
									{#if st.hasSchemas(engine)}<td><input value={fk.refSchema ?? ''} oninput={(e) => (fk.refSchema = textOrNull(e.currentTarget.value))} placeholder={draft.schema ?? ''} /></td>{/if}
									<td><input bind:value={fk.refTable} spellcheck="false" /></td>
									<td><input value={fk.refColumns.join(', ')} onchange={(e) => (fk.refColumns = list(e.currentTarget.value))} spellcheck="false" /></td>
									<td>
										<select value={fk.onDelete ?? 'NO ACTION'} onchange={(e) => (fk.onDelete = e.currentTarget.value === 'NO ACTION' ? null : e.currentTarget.value)}>
											{#each FK_ACTIONS as a (a)}<option value={a}>{a}</option>{/each}
										</select>
									</td>
									<td>
										<select value={fk.onUpdate ?? 'NO ACTION'} onchange={(e) => (fk.onUpdate = e.currentTarget.value === 'NO ACTION' ? null : e.currentTarget.value)}>
											{#each FK_ACTIONS as a (a)}<option value={a}>{a}</option>{/each}
										</select>
									</td>
									<td><button class="del" onclick={() => removeAt(draft.foreignKeys, i)} aria-label={t('db.remove')}><FaIcon icon={faTrash} /></button></td>
								</tr>
							{:else}
								<tr><td colspan="8" class="none">{t('db.none_yet')}</td></tr>
							{/each}
						</tbody>
					</table>
				</div>
			{:else if part === 'checks'}
				<div class="table-scroll">
					<table class="edit">
						<thead>
							<tr>
								<th class="w-name">{t('db.col_name')}</th>
								<th>{t('db.expression')}</th>
								<th class="w-del"></th>
							</tr>
						</thead>
						<tbody>
							{#each draft.checks as ck, i (i)}
								<tr>
									<td><input bind:value={ck.name} spellcheck="false" /></td>
									<td><input bind:value={ck.expression} placeholder="price > 0" spellcheck="false" class="mono" /></td>
									<td><button class="del" onclick={() => removeAt(draft.checks, i)} aria-label={t('db.remove')}><FaIcon icon={faTrash} /></button></td>
								</tr>
							{:else}
								<tr><td colspan="3" class="none">{t('db.none_yet')}</td></tr>
							{/each}
						</tbody>
					</table>
				</div>
			{:else if part === 'triggers'}
				<div class="triggers">
					<div class="tlist">
						{#each draft.triggers as tr, i (i)}
							<div class="titem" class:on={selectedTrigger === i}>
								<button class="tname-btn" onclick={() => (selectedTrigger = i)}>{tr.name}</button>
								<button class="del" onclick={() => removeAt(draft.triggers, i)} aria-label={t('db.remove')}><FaIcon icon={faTrash} /></button>
							</div>
						{:else}
							<p class="none">{t('db.none_yet')}</p>
						{/each}
					</div>
					{#if draft.triggers[selectedTrigger]}
						{@const tr = draft.triggers[selectedTrigger]}
						<div class="tedit">
							<label>
								<span>{t('db.col_name')}</span>
								<input bind:value={tr.name} spellcheck="false" />
							</label>
							<textarea bind:value={tr.definition} spellcheck="false" class="mono"></textarea>
							<small>{t('db.trigger_hint')}</small>
						</div>
					{/if}
				</div>
			{:else if part === 'options'}
				<div class="form">
					<label>
						<span>{t('db.storage_engine')}</span>
						<input list="db-engines-{tab.id}" value={draft.options.engine ?? ''} oninput={(e) => (draft.options.engine = textOrNull(e.currentTarget.value))} />
						<datalist id="db-engines-{tab.id}">{#each mysqlEngines() as m (m)}<option value={m}></option>{/each}</datalist>
					</label>
					<label>
						<span>{t('db.charset')}</span>
						<input value={draft.options.charset ?? ''} oninput={(e) => (draft.options.charset = textOrNull(e.currentTarget.value))} placeholder="utf8mb4" />
					</label>
					<label>
						<span>{t('db.collation')}</span>
						<input value={draft.options.collation ?? ''} oninput={(e) => (draft.options.collation = textOrNull(e.currentTarget.value))} placeholder="utf8mb4_0900_ai_ci" />
					</label>
					<label>
						<span>{t('db.auto_increment_next')}</span>
						<input value={draft.options.autoIncrement ?? ''} oninput={(e) => (draft.options.autoIncrement = numOrNull(e.currentTarget.value))} />
					</label>
				</div>
			{:else if part === 'comment'}
				<div class="form">
					<textarea class="comment" value={draft.comment ?? ''} oninput={(e) => (draft.comment = textOrNull(e.currentTarget.value))} placeholder={t('db.comment_hint')}></textarea>
				</div>
			{:else}
				<div class="sql">
					{#if preview.error}
						<div class="preview-error">{preview.error}</div>
					{:else if !preview.statements.length}
						<p class="none">{t('db.designer_no_changes')}</p>
					{:else}
						<pre>{preview.statements.map((s) => s.trim().replace(/;$/, '') + ';').join('\n\n')}</pre>
					{/if}
				</div>
			{/if}
		</div>

		{#if preview.error && part !== 'sql'}
			<div class="status bad">{preview.error}</div>
		{/if}
	</div>
{/if}

<SqlReviewDialog
	open={!!review}
	title={original ? t('db.review_alter', { table: draft.name }) : t('db.review_create', { table: draft.name })}
	intro={engine === 'mysql' || engine === 'mariadb' ? t('db.review_mysql_note') : t('db.review_tx_note')}
	statements={review ?? []}
	confirmLabel={original ? t('db.apply') : t('db.create_table')}
	danger={review?.some((s) => /\bDROP\b/i.test(s))}
	typeToConfirm={st.getConnection(tab.connId)?.production && review?.some((s) => /\bDROP\b/i.test(s)) ? st.getConnection(tab.connId)?.name : null}
	busy={applying}
	error={applyError}
	onconfirm={apply}
	onclose={() => (review = null)}
/>

<style>
	.designer {
		display: flex;
		flex-direction: column;
		height: 100%;
		min-height: 0;
	}

	.load-error {
		padding: 20px;
		color: var(--color-danger);
		font-size: var(--text-sm);
	}

	.toolbar {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 6px 8px;
		background: var(--color-bg-elevated);
		border-bottom: 1px solid var(--color-border);
		flex-wrap: wrap;
	}

	.tname {
		display: flex;
		align-items: center;
		gap: 6px;
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
	}

	.tname input {
		width: 180px;
		padding: 4px 8px;
		border-radius: var(--radius-sm);
		border: 1px solid var(--color-border);
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		font-family: var(--font-mono);
		font-size: var(--text-sm);
	}

	.sep {
		width: 1px;
		height: 18px;
		background: var(--color-border);
		margin: 0 2px;
	}

	.btn {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		padding: 4px 10px;
		border-radius: var(--radius-btn);
		border: 1px solid var(--color-border);
		background: transparent;
		color: var(--color-text-primary);
		font-size: var(--text-xs);
		cursor: pointer;
	}

	.btn:hover:not(:disabled) {
		background: var(--color-surface-hover);
	}

	.btn:disabled {
		opacity: 0.45;
		cursor: default;
	}

	.btn.primary {
		background: var(--color-accent);
		border-color: var(--color-accent);
		color: white;
	}

	.btn.on {
		color: #ffd60a;
		border-color: #ffd60a;
	}

	.btn.ghost {
		border-color: transparent;
		color: var(--color-text-secondary);
	}

	.spacer {
		flex: 1;
	}

	.parts {
		display: flex;
		gap: 2px;
		padding: 0 8px;
		border-bottom: 1px solid var(--color-border);
		overflow-x: auto;
	}

	.part {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		padding: 7px 10px;
		border: none;
		border-bottom: 2px solid transparent;
		background: transparent;
		color: var(--color-text-secondary);
		font-size: var(--text-xs);
		cursor: pointer;
		white-space: nowrap;
	}

	.part.on {
		color: var(--color-text-primary);
		border-bottom-color: var(--color-accent);
	}

	.count {
		padding: 0 5px;
		border-radius: 8px;
		font-size: 10px;
		background: var(--color-surface-hover);
		color: var(--color-text-secondary);
	}

	.count.accent {
		background: color-mix(in srgb, var(--color-accent) 25%, transparent);
		color: var(--color-accent);
	}

	.content {
		flex: 1;
		min-height: 0;
		overflow: hidden;
		display: flex;
		flex-direction: column;
	}

	.fields {
		display: flex;
		flex-direction: column;
		height: 100%;
		min-height: 0;
	}

	.table-scroll {
		flex: 1;
		overflow: auto;
		min-height: 0;
	}

	table.edit {
		width: 100%;
		border-collapse: collapse;
		font-size: var(--text-sm);
	}

	.edit th {
		position: sticky;
		top: 0;
		z-index: 1;
		padding: 6px 8px;
		background: var(--color-bg-elevated);
		border-bottom: 1px solid var(--color-border);
		color: var(--color-text-secondary);
		font-size: var(--text-xs);
		font-weight: 600;
		text-align: left;
		white-space: nowrap;
	}

	.edit td {
		padding: 2px 4px;
		border-bottom: 1px solid color-mix(in srgb, var(--color-border) 60%, transparent);
	}

	.edit tr.selected td {
		background: color-mix(in srgb, var(--color-accent) 10%, transparent);
	}

	.edit input:not([type='checkbox']),
	.edit select {
		width: 100%;
		padding: 4px 6px;
		border: 1px solid transparent;
		border-radius: var(--radius-sm);
		background: transparent;
		color: var(--color-text-primary);
		font-family: var(--font-mono);
		font-size: var(--text-xs);
	}

	.edit input:focus,
	.edit select:focus {
		outline: none;
		border-color: var(--color-accent);
		background: var(--color-bg-primary);
	}

	.edit select {
		font-family: inherit;
	}

	.edit .num {
		text-align: right;
	}

	.w-name {
		width: 22%;
	}

	.w-type {
		width: 16%;
	}

	.w-num {
		width: 72px;
	}

	.w-num2 {
		width: 110px;
	}

	.w-chk {
		width: 64px;
	}

	.w-del {
		width: 36px;
	}

	.center {
		text-align: center;
	}

	.keybtn {
		border: none;
		background: none;
		cursor: pointer;
		color: var(--color-text-tertiary);
		min-width: 24px;
		min-height: 18px;
	}

	.keybtn.on {
		color: #ffd60a;
	}

	.keybtn sub {
		font-size: 9px;
	}

	.del {
		border: none;
		background: none;
		cursor: pointer;
		color: var(--color-text-tertiary);
	}

	.del:hover {
		color: var(--color-danger);
	}

	.none {
		padding: 16px;
		color: var(--color-text-tertiary);
		font-size: var(--text-sm);
		text-align: center;
	}

	.props {
		border-top: 1px solid var(--color-border);
		background: var(--color-bg-elevated);
		padding: 10px 12px;
	}

	.props-title {
		font-size: var(--text-xs);
		font-weight: 600;
		color: var(--color-text-secondary);
		margin-bottom: 8px;
		font-family: var(--font-mono);
	}

	.props-grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
		gap: 8px 16px;
		align-items: end;
	}

	.props-grid label,
	.form label,
	.tedit label {
		display: flex;
		flex-direction: column;
		gap: 4px;
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
	}

	.props-grid label.check {
		flex-direction: row;
		align-items: center;
		gap: 6px;
		padding-bottom: 6px;
	}

	.props-grid label.wide {
		grid-column: span 2;
	}

	.props-grid input:not([type='checkbox']),
	.form input,
	.tedit input {
		padding: 5px 8px;
		border-radius: var(--radius-sm);
		border: 1px solid var(--color-border);
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		font-family: var(--font-mono);
		font-size: var(--text-xs);
	}

	.form {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
		gap: 12px;
		padding: 14px;
		align-content: start;
		overflow: auto;
	}

	.comment {
		grid-column: 1 / -1;
		min-height: 160px;
		padding: 8px;
		border-radius: var(--radius-sm);
		border: 1px solid var(--color-border);
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		font-size: var(--text-sm);
		resize: vertical;
	}

	.triggers {
		display: flex;
		height: 100%;
		min-height: 0;
	}

	.tlist {
		width: 220px;
		border-right: 1px solid var(--color-border);
		overflow: auto;
	}

	.titem {
		display: flex;
		align-items: center;
		padding-right: 6px;
	}

	.titem.on {
		background: color-mix(in srgb, var(--color-accent) 12%, transparent);
	}

	.tname-btn {
		flex: 1;
		padding: 7px 10px;
		border: none;
		background: none;
		color: var(--color-text-primary);
		font-family: var(--font-mono);
		font-size: var(--text-xs);
		text-align: left;
		cursor: pointer;
	}

	.tedit {
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: 8px;
		padding: 10px;
		min-width: 0;
	}

	.tedit textarea {
		flex: 1;
		min-height: 160px;
		padding: 8px;
		border-radius: var(--radius-sm);
		border: 1px solid var(--color-border);
		background: var(--color-surface-sunken, var(--color-bg-primary));
		color: var(--color-text-primary);
		font-size: var(--text-xs);
		resize: none;
	}

	.tedit small {
		color: var(--color-text-tertiary);
		font-size: var(--text-xs);
	}

	.mono {
		font-family: var(--font-mono);
	}

	.sql {
		flex: 1;
		overflow: auto;
		padding: 12px;
	}

	.sql pre {
		margin: 0;
		font-family: var(--font-mono);
		font-size: var(--text-xs);
		white-space: pre-wrap;
		color: var(--color-text-primary);
	}

	.preview-error,
	.status.bad {
		padding: 8px 12px;
		font-size: var(--text-xs);
		color: var(--color-danger);
		background: color-mix(in srgb, var(--color-danger) 10%, transparent);
		white-space: pre-wrap;
	}

	@media (max-width: 720px) {
		.btn span {
			display: none;
		}

		.tlist {
			width: 140px;
		}

		.props-grid label.wide {
			grid-column: auto;
		}
	}
</style>
