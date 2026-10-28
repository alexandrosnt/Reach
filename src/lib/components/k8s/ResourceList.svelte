<script lang="ts">
	/**
	 * The list for one view of the nav: pods, workloads, Helm releases,
	 * events or any listed kind. It loads on every refresh tick, filters by
	 * the search box and "Problems only", and groups by namespace when all
	 * of them are shown.
	 *
	 * Long lists are drawn a page at a time ("Show more"): rows differ in
	 * height on a phone and between kinds, which a fixed-height virtual list
	 * cannot follow, and a few hundred rows draw fast enough.
	 */
	import { untrack } from 'svelte';
	import StatusPill from '#lib/components/devops/StatusPill.svelte';
	import ErrorHelp from '#lib/components/devops/ErrorHelp.svelte';
	import EventList from './EventList.svelte';
	import {
		k8sEvents,
		k8sHelmReleases,
		k8sObjects,
		k8sPods,
		k8sWorkloads,
		type EventRow,
		type Kind,
		type ObjectRow,
		type PodRow,
		type Revision,
		type WorkloadRow
	} from '#lib/ipc/k8s.js';
	import { ago, podStatus, replicaStatus, type Status } from '#lib/devops/status.js';
	import { t } from '#lib/state/i18n.svelte.js';
	import {
		CLUSTER_SCOPED,
		OBJECT_KIND,
		helmProblem,
		helmStatus,
		nodeStatus,
		podProblem,
		selId,
		workloadProblem,
		type Ctx,
		type Selection,
		type View
	} from './model';

	interface Props {
		ctx: Ctx;
		view: Exclude<View, 'overview'>;
		tick: number;
		search: string;
		problemsOnly: boolean;
		selectedId: string | null;
		onselect: (s: Selection) => void;
		/** The selected row, re-read on a refresh, so its detail stays current. */
		onfresh: (s: Selection) => void;
		onevent: (e: EventRow) => void;
	}

	let { ctx, view, tick, search, problemsOnly, selectedId, onselect, onfresh, onevent }: Props = $props();

	const PAGE = 300;

	let pods = $state<PodRow[]>([]);
	let workloads = $state<WorkloadRow[]>([]);
	let objects = $state<ObjectRow[]>([]);
	let releases = $state<Revision[]>([]);
	let events = $state<EventRow[]>([]);
	let loadedFor = $state('');
	let error = $state('');
	let limit = $state(PAGE);
	let sort = $state<'name' | 'age' | 'status'>('name');
	let seq = 0;

	let kind = $derived(OBJECT_KIND[view] ?? null);
	let scoped = $derived(kind !== null && CLUSTER_SCOPED.includes(kind));
	let target = $derived(`${ctx.key}\n${view}\n${ctx.namespace ?? ''}`);

	async function load(): Promise<void> {
		const mine = ++seq;
		const want = target;
		const { key } = ctx;
		const ns = scoped ? null : ctx.namespace;
		try {
			switch (view) {
				case 'pods': {
					const rows = await k8sPods(key, ns);
					if (mine === seq) pods = rows;
					break;
				}
				case 'workloads': {
					const rows = await k8sWorkloads(key, ns);
					if (mine === seq) workloads = rows;
					break;
				}
				case 'helm': {
					const rows = await k8sHelmReleases(key, ns);
					if (mine === seq) releases = rows;
					break;
				}
				case 'events': {
					const rows = await k8sEvents(key, ns);
					if (mine === seq) events = rows;
					break;
				}
				default: {
					const rows = await k8sObjects(key, kind as Kind, ns);
					if (mine === seq) objects = rows;
				}
			}
			if (mine !== seq) return;
			error = '';
			loadedFor = want;
			refreshSelection();
		} catch (e) {
			if (mine === seq) {
				error = String(e);
				loadedFor = want;
			}
		}
	}

	$effect(() => {
		void target;
		void tick;
		untrack(load);
	});

	// A different list: from the top again, and never another kind's rows
	// under this one's heading while it loads.
	$effect(() => {
		void target;
		untrack(() => {
			limit = PAGE;
			pods = [];
			workloads = [];
			objects = [];
			releases = [];
			events = [];
		});
	});

	let loading = $derived(loadedFor !== target);

	function refreshSelection(): void {
		if (!selectedId) return;
		const fresh = all.find((s) => selId(s) === selectedId);
		if (fresh) onfresh(fresh);
	}

	/** Every row of the current view, as selections. */
	let all = $derived.by((): Selection[] => {
		switch (view) {
			case 'pods':
				return pods.map((row) => ({ type: 'pod', row }));
			case 'workloads':
				return workloads.map((row) => ({ type: 'workload', row }));
			case 'helm':
				return releases.map((row) => ({ type: 'helm', row }));
			case 'events':
				return [];
			default:
				return objects.map((row) => ({ type: 'object', kind: kind as Kind, row }));
		}
	});

	let problemsApply = $derived(view === 'pods' || view === 'workloads' || view === 'helm' || view === 'events' || view === 'nodes');

	function isProblem(s: Selection): boolean {
		switch (s.type) {
			case 'pod':
				return podProblem(s.row);
			case 'workload':
				return workloadProblem(s.row);
			case 'helm':
				return helmProblem(s.row);
			default:
				return s.kind === 'Node' && nodeStatus(s.row.summary).tone !== 'ok';
		}
	}

	function status(s: Selection): Status | null {
		switch (s.type) {
			case 'pod':
				return podStatus(s.row.status, s.row.restarts);
			case 'workload':
				return replicaStatus(s.row.ready, s.row.desired);
			case 'helm':
				return helmStatus(s.row.status);
			default:
				return s.kind === 'Node' ? nodeStatus(s.row.summary) : null;
		}
	}

	const RANK = { bad: 0, warn: 1, busy: 2, idle: 3, ok: 4 } as const;

	function created(s: Selection): number {
		const when = s.type === 'helm' ? s.row.updated : s.row.created;
		const ms = when ? Date.parse(when) : NaN;
		return Number.isFinite(ms) ? ms : 0;
	}

	let needle = $derived(search.trim().toLowerCase());

	let filtered = $derived.by(() => {
		let rows = all;
		if (needle) rows = rows.filter((s) => s.row.name.toLowerCase().includes(needle));
		if (problemsOnly && problemsApply) rows = rows.filter(isProblem);
		const byName = (a: Selection, b: Selection) => a.row.namespace.localeCompare(b.row.namespace) || a.row.name.localeCompare(b.row.name);
		const sorted = rows.slice();
		if (sort === 'age') sorted.sort((a, b) => created(b) - created(a) || byName(a, b));
		else if (sort === 'status') sorted.sort((a, b) => RANK[status(a)?.tone ?? 'ok'] - RANK[status(b)?.tone ?? 'ok'] || byName(a, b));
		else sorted.sort(byName);
		return sorted;
	});

	let grouped = $derived(!ctx.namespace && !scoped && sort === 'name');

	/** Rows with a namespace heading wherever the namespace changes. */
	let items = $derived.by(() => {
		const out: ({ group: string; count: number } | { sel: Selection })[] = [];
		let last: string | null = null;
		const counts = new Map<string, number>();
		if (grouped) for (const s of filtered) counts.set(s.row.namespace, (counts.get(s.row.namespace) ?? 0) + 1);
		for (const s of filtered.slice(0, limit)) {
			if (grouped && s.row.namespace !== last) {
				last = s.row.namespace;
				out.push({ group: last, count: counts.get(last) ?? 0 });
			}
			out.push({ sel: s });
		}
		return out;
	});

	let shownEvents = $derived.by(() => {
		let rows = events;
		if (needle) rows = rows.filter((e) => `${e.object} ${e.reason} ${e.message}`.toLowerCase().includes(needle));
		if (problemsOnly) rows = rows.filter((e) => e.kind === 'Warning');
		return rows.slice(0, limit);
	});
	let eventTotal = $derived(view === 'events' ? events.length : 0);

	let total = $derived(view === 'events' ? eventTotal : all.length);
	let matching = $derived(view === 'events' ? shownEvents.length : filtered.length);

	function key(s: Selection): string {
		return selId(s);
	}
</script>

<div class="list" class:pods={view === 'pods'} class:workloads={view === 'workloads'} class:helm={view === 'helm'} class:nodes={view === 'nodes'}>
	<div class="bar">
		<span class="count">
			{#if loading && !error}
				{t('common.loading')}
			{:else if matching !== total}
				{t('k8s.showing_of', { shown: matching, total })}
			{:else}
				{t('k8s.items', { count: total })}
			{/if}
		</span>
		{#if problemsOnly && !problemsApply}
			<span class="note">{t('k8s.problems_na')}</span>
		{/if}
		<span class="spacer"></span>
		{#if view !== 'events'}
			<label class="sort">
				<span>{t('k8s.sort_by')}</span>
				<select bind:value={sort}>
					<option value="name">{t('k8s.sort_name')}</option>
					<option value="age">{view === 'helm' ? t('k8s.sort_updated') : t('k8s.sort_age')}</option>
					{#if problemsApply}<option value="status">{t('k8s.sort_status')}</option>{/if}
				</select>
			</label>
		{/if}
	</div>

	<div class="rows">
		{#if error}
			<div class="pad"><ErrorHelp {error} onretry={load} /></div>
		{:else if view === 'events'}
			<div class="pad">
				{#if shownEvents.length}
					<EventList events={shownEvents} showNamespace={!ctx.namespace} onopen={onevent} />
				{:else if !loading}
					<p class="empty">{needle || problemsOnly ? t('k8s.nothing_matches') : t('k8s.no_events')}</p>
				{/if}
			</div>
		{:else}
			{#if items.length}
				<div class="head" aria-hidden="true">
					{#if view === 'pods'}
						<span>{t('k8s.col_name')}</span><span>{t('k8s.col_ready')}</span><span>{t('k8s.col_status')}</span><span
							>{t('k8s.col_restarts')}</span
						><span>{t('k8s.col_age')}</span><span>{t('k8s.col_node')}</span>
					{:else if view === 'workloads'}
						<span>{t('k8s.kind')}</span><span>{t('k8s.col_name')}</span><span>{t('k8s.col_status')}</span><span>{t('k8s.col_images')}</span><span
							>{t('k8s.col_age')}</span
						>
					{:else if view === 'helm'}
						<span>{t('k8s.col_name')}</span><span>{t('k8s.helm_chart')}</span><span>{t('k8s.helm_app_version')}</span><span
							>{t('k8s.helm_revision')}</span
						><span>{t('k8s.col_status')}</span><span>{t('k8s.helm_updated')}</span>
					{:else if view === 'nodes'}
						<span>{t('k8s.col_name')}</span><span>{t('k8s.col_status')}</span><span>{t('k8s.node_version')}</span><span>{t('k8s.col_age')}</span>
					{:else}
						<span>{t('k8s.col_name')}</span><span>{t('k8s.col_details')}</span><span>{t('k8s.col_age')}</span>
					{/if}
				</div>
			{/if}
			{#each items as it ('group' in it ? `g:${it.group}` : key(it.sel))}
				{#if 'group' in it}
					<div class="group">{it.group || t('k8s.cluster_wide')} <span>{it.count}</span></div>
				{:else}
					{@const s = it.sel}
					{@const st = status(s)}
					<button type="button" class="row" class:selected={key(s) === selectedId} aria-current={key(s) === selectedId} onclick={() => onselect(s)}>
						{#if s.type === 'pod'}
							<span class="name" title={s.row.name}>{s.row.name}</span>
							<span class="dim" data-label={t('k8s.col_ready')}>{s.row.ready}</span>
							<span>{#if st}<StatusPill status={st} />{/if}</span>
							<span class:hot={s.row.restarts > 0} data-label={t('k8s.col_restarts')}>{s.row.restarts}</span>
							<span class="dim">{ago(s.row.created)}</span>
							<span class="dim mono" title={s.row.node}>{s.row.node}</span>
						{:else if s.type === 'workload'}
							<span class="dim">{s.row.kind}</span>
							<span class="name" title={s.row.name}>{s.row.name}</span>
							<span>{#if st}<StatusPill status={st} />{/if}</span>
							<span class="dim mono" title={s.row.images.join('\n')}>{s.row.images.join(', ')}</span>
							<span class="dim">{ago(s.row.created)}</span>
						{:else if s.type === 'helm'}
							<span class="name" title={s.row.name}>{s.row.name}</span>
							<span class="dim mono">{s.row.chart}-{s.row.chartVersion}</span>
							<span class="dim mono">{s.row.appVersion}</span>
							<span class="dim" data-label={t('k8s.helm_revision')}>{s.row.revision}</span>
							<span>{#if st}<StatusPill status={st} />{/if}</span>
							<span class="dim" title={s.row.updated}>{ago(s.row.updated) || s.row.updated}</span>
						{:else if s.kind === 'Node'}
							<span class="name" title={s.row.name}>{s.row.name}</span>
							<span>{#if st}<StatusPill status={st} />{/if}</span>
							<span class="dim mono">{s.row.summary.split(' ')[1] ?? ''}</span>
							<span class="dim">{ago(s.row.created)}</span>
						{:else}
							<span class="name" title={s.row.name}>{s.row.name}</span>
							<span class="dim mono" title={s.row.summary}>{s.row.summary}</span>
							<span class="dim">{ago(s.row.created)}</span>
						{/if}
					</button>
				{/if}
			{:else}
				{#if !loading}
					<p class="empty pad">
						{#if needle || (problemsOnly && problemsApply)}
							{problemsOnly && !needle ? t('k8s.no_problems') : t('k8s.nothing_matches')}
						{:else}
							{ctx.namespace && !scoped ? t('k8s.empty_in_namespace', { namespace: ctx.namespace }) : t('k8s.empty')}
						{/if}
					</p>
				{/if}
			{/each}
		{/if}
		{#if matching > limit}
			<button type="button" class="more" onclick={() => (limit += PAGE)}>{t('k8s.show_more', { count: matching - limit })}</button>
		{/if}
	</div>
</div>

<style>
	.list {
		display: flex;
		flex-direction: column;
		height: 100%;
		min-height: 0;
		--cols: minmax(160px, 2fr) minmax(140px, 3fr) 90px;
	}

	.pods {
		--cols: minmax(160px, 3fr) 56px minmax(120px, 1.6fr) 64px 80px minmax(80px, 1.2fr);
	}

	.workloads {
		--cols: 96px minmax(140px, 2fr) minmax(110px, 1.2fr) minmax(120px, 2fr) 80px;
	}

	.helm {
		--cols: minmax(140px, 2fr) minmax(120px, 1.6fr) 90px 64px minmax(100px, 1.1fr) 100px;
	}

	.nodes {
		--cols: minmax(160px, 2fr) minmax(100px, 1fr) minmax(90px, 1fr) 90px;
	}

	.bar {
		display: flex;
		align-items: center;
		gap: 10px;
		flex-wrap: wrap;
		padding: 6px 12px;
		border-bottom: 1px solid var(--color-border);
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
		flex-shrink: 0;
	}

	.note {
		color: var(--color-text-tertiary);
	}

	.spacer {
		flex: 1;
	}

	.sort {
		display: inline-flex;
		align-items: center;
		gap: 6px;
	}

	.sort select {
		min-height: 28px;
		padding: 0 6px;
		border-radius: var(--radius-sm);
		border: 1px solid var(--color-border);
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		font: inherit;
	}

	.rows {
		flex: 1;
		min-height: 0;
		overflow: auto;
	}

	.pad {
		padding: 12px;
	}

	.head,
	.row {
		display: grid;
		grid-template-columns: var(--cols);
		gap: 10px;
		align-items: center;
		padding: 0 12px;
	}

	.head {
		position: sticky;
		top: 0;
		z-index: 1;
		min-height: 28px;
		background: var(--color-bg-primary);
		border-bottom: 1px solid var(--color-border);
		font-size: var(--text-xs);
		color: var(--color-text-tertiary);
	}

	.group {
		position: sticky;
		top: 28px;
		padding: 6px 12px 4px;
		background: var(--color-bg-primary);
		font-size: var(--text-xs);
		font-weight: 600;
		color: var(--color-text-secondary);
		font-family: var(--font-mono);
	}

	.group span {
		margin-left: 4px;
		font-weight: 400;
		color: var(--color-text-tertiary);
	}

	.row {
		width: 100%;
		min-height: 36px;
		border: none;
		border-bottom: 1px solid color-mix(in srgb, var(--color-border) 60%, transparent);
		background: none;
		color: var(--color-text-primary);
		font: inherit;
		font-size: var(--text-sm);
		text-align: left;
		cursor: pointer;
	}

	.row > span {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.row:hover {
		background: var(--color-surface-hover);
	}

	.row.selected {
		background: color-mix(in srgb, var(--color-accent) 14%, transparent);
	}

	.row:focus-visible {
		outline: 2px solid var(--color-accent);
		outline-offset: -2px;
	}

	.name {
		font-family: var(--font-mono);
		font-size: var(--text-xs);
	}

	.dim {
		color: var(--color-text-secondary);
		font-size: var(--text-xs);
	}

	.mono {
		font-family: var(--font-mono);
	}

	.hot {
		color: var(--color-warning);
		font-weight: 600;
	}

	.empty {
		margin: 0;
		font-size: var(--text-sm);
		color: var(--color-text-tertiary);
	}

	.more {
		display: block;
		margin: 10px auto;
		min-height: 36px;
		padding: 0 16px;
		border-radius: var(--radius-btn);
		border: 1px solid var(--color-border);
		background: var(--color-bg-elevated);
		color: var(--color-text-primary);
		font: inherit;
		font-size: var(--text-sm);
		cursor: pointer;
	}

	/* A phone: each row becomes a small card, name on its own line. */
	@media (max-width: 759px) {
		.head {
			display: none;
		}

		.group {
			top: 0;
		}

		.row {
			display: flex;
			flex-wrap: wrap;
			gap: 4px 12px;
			min-height: 56px;
			padding: 8px 12px;
		}

		.row > .name {
			flex: 1 0 100%;
			white-space: normal;
			overflow-wrap: anywhere;
		}

		.row > span[data-label]::before {
			content: attr(data-label) ' ';
			color: var(--color-text-tertiary);
		}

		.row > span:empty {
			display: none;
		}
	}
</style>
