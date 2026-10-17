<script lang="ts" module>
	import type { LogRule } from '$lib/devops/logsource';

	/** Sources remembered for hosts that are not saved (a terminal tab's server), until Reach closes. */
	const unsaved = new Map<string, LogRule[]>();
	/** The source last shown per container, so coming back finds it again. */
	const lastShown = new Map<string, string[] | null>();
</script>

<script lang="ts">
	/**
	 * A log with a choice of where it comes from (issue #101): the engine's
	 * own output (`docker logs`, `kubectl logs`), or files inside the
	 * container followed as `tail -F`. Files are found for the user (what the
	 * processes hold open, the usual log folders) or typed, and can be
	 * remembered for the service, the container or the whole host, saved with
	 * the host in the vault.
	 */
	import LogViewer from './LogViewer.svelte';
	import { t } from '$lib/state/i18n.svelte';
	import { addToast } from '$lib/state/toasts.svelte';
	import { ago } from '$lib/devops/status';
	import { devopsLogRules, devopsLogRulesSave, type FoundFile, type LogView } from '$lib/ipc/containers';
	import { sourcesFor, sourceLabel, templated, upsert, remove, size, groupRotated, type Scope, type Source, type Target } from '$lib/devops/logsource';

	type Start = (onData: (chunk: string) => void) => Promise<() => void>;

	interface Props {
		/** Container or pod name: the download's name and the picker's title. */
		name: string;
		target: Target;
		/** The saved host's or cluster's id; null for one that is not saved. */
		owner: string | null;
		kind: 'docker' | 'k8s';
		output: (view: LogView) => Start;
		files: (paths: string[], lines: number) => Start;
		listFiles: () => Promise<FoundFile[]>;
		/** Open the file picker (the row menu's "Log files…"). */
		pick?: boolean;
	}

	let { name, target, owner, kind, output, files, listFiles, pick = $bindable(false) }: Props = $props();

	const LINES = [100, 500, 2000];
	const SINCE: { secs: number | null; key: string }[] = [
		{ secs: null, key: 'logsrc.since_all' },
		{ secs: 300, key: 'logsrc.since_5m' },
		{ secs: 900, key: 'logsrc.since_15m' },
		{ secs: 3600, key: 'logsrc.since_1h' },
		{ secs: 86400, key: 'logsrc.since_24h' }
	];

	const memoryKey = $derived(`${owner ?? 'unsaved'}\u0000${target.namespace ?? ''}\u0000${target.name}`);

	let rules = $state<LogRule[]>([]);
	let paths = $state<string[] | null>(null);
	let lines = $state(500);
	let since = $state<number | null>(null);
	let timestamps = $state(false);

	// Saved sources for this host, and the last one shown for this container.
	$effect(() => {
		const o = owner;
		paths = lastShown.get(memoryKey) ?? null;
		if (!o) {
			rules = unsaved.get('unsaved') ?? [];
			return;
		}
		devopsLogRules(o)
			.then((r) => (rules = r))
			.catch(() => (rules = []));
	});

	let sources = $derived<Source[]>(sourcesFor(rules, target));
	let current = $derived(paths ? sources.find((s) => s.paths.join('\n') === paths!.join('\n')) ?? null : null);
	let selectValue = $derived(paths === null ? 'output' : current ? `src:${sources.indexOf(current)}` : 'custom');

	/** Bumped by "Read again": the same source, started afresh. */
	let again = $state(0);

	let start = $derived.by<Start>(() => {
		void again;
		if (paths) return files(paths, lines);
		return output({ tail: lines, sinceSecs: since, timestamps });
	});

	function show(next: string[] | null): void {
		paths = next;
		lastShown.set(memoryKey, next);
	}

	function onSource(value: string): void {
		if (value === 'output') show(null);
		else if (value === 'pick') openPicker();
		else if (value.startsWith('src:')) show(sources[Number(value.slice(4))]?.paths ?? null);
	}

	async function saveRules(next: LogRule[]): Promise<boolean> {
		if (!owner) {
			unsaved.set('unsaved', next);
			rules = next;
			return true;
		}
		try {
			await devopsLogRulesSave(owner, next);
			rules = next;
			return true;
		} catch (e) {
			addToast(String(e), 'error');
			return false;
		}
	}

	async function forget(): Promise<void> {
		if (!current) return;
		if (await saveRules(remove(rules, current.rule))) {
			addToast(t('logsrc.forgotten'), 'info');
			show(null);
		}
	}

	// The picker

	type Remember = 'none' | 'service' | 'container' | 'podContainer' | 'host';

	let picking = $state(false);
	let found = $state<FoundFile[] | null>(null);
	let finding = $state(false);
	let findError = $state('');
	let chosen = $state<string[]>([]);
	let typed = $state('');
	let typedError = $state('');
	let remember = $state<Remember>('none');
	/** Which files' rotated copies are shown. */
	let expanded = $state<Record<string, boolean>>({});

	$effect(() => {
		if (pick) {
			pick = false;
			openPicker();
		}
	});

	function openPicker(): void {
		picking = true;
		chosen = paths ? [...paths] : [];
		typed = '';
		typedError = '';
		remember = 'none';
		void find();
	}

	async function find(): Promise<void> {
		finding = true;
		findError = '';
		try {
			found = await listFiles();
		} catch (e) {
			found = null;
			findError = String(e);
		} finally {
			finding = false;
		}
	}

	function toggle(path: string): void {
		chosen = chosen.includes(path) ? chosen.filter((p) => p !== path) : [...chosen, path];
	}

	function addTyped(): void {
		const p = typed.trim();
		if (!p) return;
		if (!p.startsWith('/')) {
			typedError = t('logsrc.full_path');
			return;
		}
		typedError = '';
		if (!chosen.includes(p)) chosen = [...chosen, p];
		typed = '';
	}

	let rememberOptions = $derived.by<{ value: Remember; label: string }[]>(() => {
		const o: { value: Remember; label: string }[] = [{ value: 'none', label: t('logsrc.remember_none') }];
		if (kind === 'docker' && target.service) o.push({ value: 'service', label: t('logsrc.remember_service', { service: `${target.project ?? ''} · ${target.service}` }) });
		if (kind === 'docker') o.push({ value: 'container', label: t('logsrc.remember_container', { name: target.name }) });
		if (kind === 'k8s' && target.container) o.push({ value: 'podContainer', label: t('logsrc.remember_pod', { container: target.container, namespace: target.namespace ?? '' }) });
		o.push({ value: 'host', label: kind === 'k8s' ? t('logsrc.remember_cluster') : t('logsrc.remember_host') });
		return o;
	});

	/** What would be saved, as it will apply. */
	let savedPaths = $derived(remember === 'host' ? chosen.map((p) => templated(p, target)) : chosen);

	function scope(r: Remember): Scope | null {
		switch (r) {
			case 'service':
				return { kind: 'service', project: target.project ?? '', service: target.service ?? '' };
			case 'container':
				return { kind: 'container', name: target.name };
			case 'podContainer':
				return { kind: 'podContainer', namespace: target.namespace ?? '', container: target.container ?? '' };
			case 'host':
				return { kind: 'host' };
			default:
				return null;
		}
	}

	async function confirm(): Promise<void> {
		addTyped();
		if (chosen.length === 0) return;
		const s = scope(remember);
		if (s && !(await saveRules(upsert(rules, { scope: s, paths: savedPaths })))) return;
		picking = false;
		show([...chosen]);
	}

	function onPickerKey(e: KeyboardEvent): void {
		if (e.key === 'Escape') {
			e.stopPropagation();
			picking = false;
		}
	}
</script>

{#snippet fileRow(f: FoundFile, older: boolean)}
	<li class:older>
		<label>
			<input type="checkbox" checked={chosen.includes(f.path)} onchange={() => toggle(f.path)} />
			<span class="path" title={f.path}>{f.path}</span>
			<span class="meta">
				{#if f.open}<span class="badge live">{t('logsrc.written')}</span>{/if}
				{#if f.link}<span class="badge">{t('logsrc.link')}</span>{/if}
				{size(f.size)} · {ago(f.modified)}
			</span>
		</label>
	</li>
{/snippet}

<div class="sources">
	<div class="bar">
		<label class="field source">
			<span class="label">{t('logsrc.source')}</span>
			<select
				value={selectValue}
				onchange={(e) => {
					const el = e.currentTarget;
					onSource(el.value);
					// "Files in the container…" is an action, not a source to stay on.
					el.value = selectValue;
				}}
				aria-label={t('logsrc.source')}
				title={paths ? `tail -n ${lines} -F -- ${paths.join(' ')}` : undefined}
			>
				<option value="output">{kind === 'k8s' ? t('logsrc.output_k8s') : t('logsrc.output')}</option>
				{#each sources as s, i (s.paths.join('\n'))}
					<option value={`src:${i}`} title={s.paths.join('\n')}>{s.label}</option>
				{/each}
				{#if paths && !current}
					<option value="custom">{sourceLabel(paths)}</option>
				{/if}
				<option value="pick">{t('logsrc.files_in')}</option>
			</select>
		</label>
		<label class="field">
			<span class="label">{t('logsrc.lines')}</span>
			<select bind:value={lines} disabled={!paths && since !== null} aria-label={t('logsrc.lines')}>
				{#each LINES as n (n)}<option value={n}>{n}</option>{/each}
			</select>
		</label>
		{#if !paths}
			<label class="field">
				<span class="label">{t('logsrc.since')}</span>
				<select bind:value={since} aria-label={t('logsrc.since')}>
					{#each SINCE as s (s.key)}<option value={s.secs}>{t(s.key)}</option>{/each}
				</select>
			</label>
			<label class="check"><input type="checkbox" bind:checked={timestamps} />{t('logsrc.timestamps')}</label>
		{:else}
			{#if current}
				<button type="button" class="quiet" onclick={forget} title={t('logsrc.forget_hint')}>{t('logsrc.forget')}</button>
			{/if}
			<button type="button" class="quiet" onclick={() => again++} title={t('logsrc.read_again_hint')} aria-label={t('logsrc.read_again')}>↻</button>
			<button type="button" class="quiet" onclick={openPicker}>{t('logsrc.change')}</button>
		{/if}
	</div>

	<div class="body">
		{#if picking}
			<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
			<div class="picker" role="dialog" aria-label={t('logsrc.picker_title', { name })} tabindex="-1" onkeydown={onPickerKey}>
				<header>
					<strong>{t('logsrc.picker_title', { name })}</strong>
					<button type="button" class="quiet" onclick={find} disabled={finding}>{t('logsrc.look_again')}</button>
				</header>

				{#if finding}
					<p class="note">{t('logsrc.looking')}</p>
				{:else if findError}
					<p class="note warn">{findError}</p>
				{:else if found && found.length === 0}
					<p class="note">{t('logsrc.none_found')}</p>
				{:else if found}
					<ul class="files">
						{#each groupRotated(found) as g (g.file.path)}
							{@render fileRow(g.file, false)}
							{#if g.older.length}
								<li class="older-toggle">
									<button type="button" aria-expanded={!!expanded[g.file.path]} onclick={() => (expanded[g.file.path] = !expanded[g.file.path])}>
										{expanded[g.file.path] ? '▾' : '▸'} {t('logsrc.older', { count: g.older.length })}
									</button>
								</li>
								{#if expanded[g.file.path]}
									{#each g.older as f (f.path)}{@render fileRow(f, true)}{/each}
								{/if}
							{/if}
						{/each}
					</ul>
				{/if}

				<div class="typed">
					<input
						type="text"
						placeholder="/logs/app.log"
						aria-label={t('logsrc.type_path')}
						bind:value={typed}
						onkeydown={(e) => e.key === 'Enter' && (e.preventDefault(), addTyped())}
						spellcheck="false"
						autocomplete="off"
					/>
					<button type="button" class="quiet" onclick={addTyped}>{t('logsrc.add')}</button>
				</div>
				{#if typedError}<p class="note warn">{typedError}</p>{/if}
				{#each chosen.filter((p) => !found?.some((f) => f.path === p)) as p (p)}
					<div class="chip"><span class="path">{p}</span><button type="button" aria-label={t('logsrc.remove_path')} onclick={() => toggle(p)}>×</button></div>
				{/each}

				<label class="field remember">
					<span class="label">{t('logsrc.remember')}</span>
					<select bind:value={remember}>
						{#each rememberOptions as o (o.value)}<option value={o.value}>{o.label}</option>{/each}
					</select>
				</label>
				{#if remember !== 'none' && chosen.length}
					<p class="note">
						{owner ? t('logsrc.saved_with_host') : t('logsrc.saved_until_close')}
						{#if remember === 'host'}<code>{savedPaths.join(', ')}</code>{/if}
					</p>
				{/if}

				<footer>
					<button type="button" class="quiet" onclick={() => (picking = false)}>{t('common.cancel')}</button>
					<button type="button" class="primary" disabled={chosen.length === 0 && !typed.trim()} onclick={confirm}>
						{t('logsrc.show')}
					</button>
				</footer>
			</div>
		{:else}
			<LogViewer {start} {name} />
		{/if}
	</div>
</div>

<style>
	.sources {
		display: flex;
		flex-direction: column;
		height: 100%;
		min-height: 0;
		/* The bar fits the pane it is in (a pod's pane is narrower than a container's). */
		container-type: inline-size;
	}

	@container (max-width: 440px) {
		/* The selects keep their names for screen readers (aria-label). */
		.field .label {
			display: none;
		}

		.bar {
			gap: 6px;
		}

		button.quiet {
			padding: 0 6px;
		}
	}

	.bar {
		display: flex;
		align-items: center;
		gap: 6px 10px;
		flex-wrap: wrap;
		padding: 6px 8px;
		border-bottom: 1px solid var(--color-border);
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
	}

	.field {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		min-width: 0;
	}

	.field.source {
		flex: 1 1 120px;
	}

	.field.source select {
		flex: 1;
		min-width: 0;
	}

	select,
	.typed input {
		min-height: 26px;
		padding: 2px 6px;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		font: inherit;
	}

	select:focus,
	.typed input:focus {
		outline: none;
		border-color: var(--color-accent);
	}

	.check {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		white-space: nowrap;
		cursor: pointer;
	}

	button.quiet,
	button.primary {
		min-height: 26px;
		padding: 0 8px;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		background: none;
		color: var(--color-text-primary);
		font: inherit;
		cursor: pointer;
	}

	button.quiet:hover:not(:disabled) {
		border-color: var(--color-accent);
	}

	button.primary {
		border-color: var(--color-accent);
		background: var(--color-accent);
		color: #fff;
		font-weight: 600;
	}

	button:disabled {
		opacity: 0.5;
		cursor: default;
	}

	.body {
		flex: 1;
		min-height: 0;
	}

	.picker {
		display: flex;
		flex-direction: column;
		gap: 8px;
		height: 100%;
		overflow: auto;
		padding: 10px 12px;
		font-size: var(--text-sm);
	}

	.picker header,
	.picker footer {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
	}

	.picker footer {
		justify-content: flex-end;
		margin-top: auto;
		padding-top: 6px;
	}

	.note {
		margin: 0;
		color: var(--color-text-secondary);
		font-size: var(--text-xs);
		line-height: 1.5;
	}

	.note.warn {
		color: var(--color-warning);
	}

	.note code {
		display: block;
		margin-top: 2px;
		font-family: var(--font-mono);
		color: var(--color-text-primary);
		word-break: break-all;
	}

	.files {
		margin: 0;
		padding: 0;
		list-style: none;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		/* Takes the room the picker has; the rest stays in view below it. */
		flex: 1 1 auto;
		min-height: 120px;
		overflow: auto;
	}

	.files li.older label {
		padding-left: 28px;
		opacity: 0.85;
	}

	.older-toggle button {
		width: 100%;
		padding: 4px 8px 4px 32px;
		border: none;
		background: none;
		color: var(--color-text-secondary);
		font: inherit;
		font-size: var(--text-xs);
		text-align: left;
		cursor: pointer;
	}

	.older-toggle button:hover {
		color: var(--color-text-primary);
	}

	.files li + li {
		border-top: 1px solid var(--color-border);
	}

	.files label {
		display: grid;
		grid-template-columns: auto 1fr;
		gap: 2px 8px;
		align-items: center;
		padding: 6px 8px;
		cursor: pointer;
	}

	.files label:hover {
		background: var(--color-surface-sunken);
	}

	.path {
		font-family: var(--font-mono);
		font-size: 12px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.meta {
		grid-column: 2;
		display: flex;
		align-items: center;
		gap: 6px;
		font-size: var(--text-xs);
		color: var(--color-text-tertiary);
	}

	.badge {
		padding: 0 6px;
		border-radius: 999px;
		border: 1px solid var(--color-border);
		color: var(--color-text-secondary);
	}

	.badge.live {
		border-color: color-mix(in srgb, var(--color-success) 50%, transparent);
		color: var(--color-success);
	}

	.typed {
		display: flex;
		gap: 6px;
	}

	.typed input {
		flex: 1;
		min-width: 0;
		font-family: var(--font-mono);
		font-size: 12px;
	}

	.chip {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 2px 4px 2px 8px;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		width: fit-content;
		max-width: 100%;
	}

	.chip button {
		border: none;
		background: none;
		color: var(--color-text-secondary);
		font-size: 14px;
		cursor: pointer;
	}

	.field.remember {
		flex-wrap: wrap;
	}

	.field.remember select {
		flex: 1;
		min-width: 0;
	}
</style>
