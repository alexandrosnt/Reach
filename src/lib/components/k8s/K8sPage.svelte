<script lang="ts">
	/**
	 * The Kubernetes workspace: which cluster and namespace across the top,
	 * always in view with the cluster's environment (production also puts a
	 * red line over the whole page), the kinds of things on the left, a list,
	 * and the selected item's detail beside it, or full screen on a phone.
	 *
	 * The visible list refreshes every few seconds, only while the page is
	 * on screen and nothing is being edited.
	 */
	import { onDestroy, onMount } from 'svelte';
	import FaIcon from '#lib/components/shared/FaIcon.svelte';
	import {
		faAnchor,
		faAnglesLeft,
		faAnglesRight,
		faBell,
		faClock,
		faCube,
		faCubes,
		faFileLines,
		faGauge,
		faGear,
		faGlobe,
		faHardDrive,
		faKey,
		faLayerGroup,
		faListCheck,
		faNetworkWired,
		faRotateRight,
		faServer,
		faTriangleExclamation
	} from '@fortawesome/free-solid-svg-icons';
	import type { IconDefinition } from '@fortawesome/free-solid-svg-icons';
	import EnvBadge from '#lib/components/devops/EnvBadge.svelte';
	import ErrorHelp from '#lib/components/devops/ErrorHelp.svelte';
	import ClusterEditor from './ClusterEditor.svelte';
	import Welcome from './Welcome.svelte';
	import Overview from './Overview.svelte';
	import ResourceList from './ResourceList.svelte';
	import PodDetail from './PodDetail.svelte';
	import WorkloadDetail from './WorkloadDetail.svelte';
	import ObjectDetail from './ObjectDetail.svelte';
	import HelmDetail from './HelmDetail.svelte';
	import { k8sClose, k8sClusters, k8sNamespaces, k8sOpen, type ClusterView, type EventRow, type OpenedCluster } from '#lib/ipc/k8s.js';
	import { sessionList } from '#lib/ipc/sessions.js';
	import { t } from '#lib/state/i18n.svelte.js';
	import { NAV, loadLast, saveLast, selId, type Ctx, type Selection, type View } from './model';

	const ICONS: Record<View, IconDefinition> = {
		overview: faGauge,
		pods: faCube,
		workloads: faCubes,
		jobs: faListCheck,
		cronjobs: faClock,
		services: faNetworkWired,
		ingresses: faGlobe,
		configmaps: faFileLines,
		secrets: faKey,
		pvcs: faHardDrive,
		helm: faAnchor,
		events: faBell,
		nodes: faServer,
		namespaces: faLayerGroup
	};

	/** Where an event's object is listed, by its kind. */
	const VIEW_OF_KIND: Record<string, View> = {
		Pod: 'pods',
		Deployment: 'workloads',
		StatefulSet: 'workloads',
		DaemonSet: 'workloads',
		ReplicaSet: 'workloads',
		Job: 'jobs',
		CronJob: 'cronjobs',
		Service: 'services',
		Ingress: 'ingresses',
		ConfigMap: 'configmaps',
		Secret: 'secrets',
		PersistentVolumeClaim: 'pvcs',
		Node: 'nodes',
		Namespace: 'namespaces'
	};

	const ADD = '__add__';
	const REFRESH_MS = 5000;

	let clusters = $state<ClusterView[]>([]);
	let clustersLoaded = $state(false);
	let clustersError = $state('');
	let clusterId = $state('');
	let opened = $state<OpenedCluster | null>(null);
	let connecting = $state(false);
	let connectError = $state('');
	let namespaces = $state<string[]>([]);
	let namespace = $state('');
	let view = $state<View>('overview');
	let search = $state('');
	let problemsOnly = $state(false);
	let selection = $state<Selection | null>(null);
	let editor = $state<{ initial: ClusterView | null } | null>(null);
	let navCollapsed = $state(false);
	let narrow = $state(false);
	let tick = $state(0);
	let editing = $state(false);
	let sessionNames = $state<Record<string, string>>({});
	let root: HTMLDivElement | undefined = $state();
	let searchBox: HTMLInputElement | undefined = $state();
	let connectSeq = 0;

	let cluster = $derived(clusters.find((c) => c.id === clusterId) ?? null);

	// Built from plain values, so re-reading the cluster list does not make
	// every pane start over.
	let ctxKey = $derived(opened?.key ?? '');
	let ctxName = $derived(cluster?.name ?? '');
	let ctxEnv = $derived(cluster?.environment ?? 'none');
	let ctxReadOnly = $derived(!!opened?.readOnly || !!cluster?.readOnly);
	let ctxNs = $derived(namespace || null);
	let ctx = $derived<Ctx | null>(
		ctxKey ? { key: ctxKey, clusterId, clusterName: ctxName, environment: ctxEnv, readOnly: ctxReadOnly, namespace: ctxNs } : null
	);

	let via = $derived(cluster?.route.kind === 'session' ? (sessionNames[cluster.route.sessionId] ?? t('k8s.a_saved_session')) : '');
	let selectedId = $derived(selection ? selId(selection) : null);

	onMount(() => {
		const mq = window.matchMedia('(max-width: 759px)');
		narrow = mq.matches;
		const onMq = (e: MediaQueryListEvent) => (narrow = e.matches);
		mq.addEventListener('change', onMq);
		try {
			navCollapsed = localStorage.getItem('reach.k8s.nav') === 'collapsed';
		} catch {
			// No storage: the nav starts open.
		}
		loadClusters(true);
		sessionList()
			.then((list) => (sessionNames = Object.fromEntries(list.map((s) => [s.id, s.name]))))
			.catch(() => {});
		return () => mq.removeEventListener('change', onMq);
	});

	onDestroy(() => {
		if (opened) k8sClose(opened.key).catch(() => {});
	});

	async function loadClusters(pickFirst = false): Promise<void> {
		try {
			clusters = await k8sClusters();
			clustersError = '';
		} catch (e) {
			clustersError = String(e);
		}
		clustersLoaded = true;
		if (pickFirst && !clusterId && clusters.length) {
			const last = loadLast();
			const pick = clusters.find((c) => c.id === last?.cluster) ?? [...clusters].sort((a, b) => b.lastUsedAt - a.lastUsedAt)[0];
			connect(pick.id);
		}
	}

	async function connect(id: string): Promise<void> {
		const mine = ++connectSeq;
		if (opened && opened.key !== id) k8sClose(opened.key).catch(() => {});
		clusterId = id;
		opened = null;
		selection = null;
		namespaces = [];
		connectError = '';
		connecting = true;
		try {
			const o = await k8sOpen(id);
			if (mine !== connectSeq) return;
			const last = loadLast();
			namespace = last?.cluster === id ? (last.namespace ?? '') : o.namespace;
			opened = o;
			saveLast(id, namespace || null);
			loadNamespaces();
		} catch (e) {
			if (mine === connectSeq) connectError = String(e);
		} finally {
			if (mine === connectSeq) connecting = false;
		}
	}

	async function loadNamespaces(): Promise<void> {
		if (!opened) return;
		try {
			namespaces = await k8sNamespaces(opened.key);
		} catch {
			// A user limited to one namespace may not list them; the picker
			// then offers the one in use.
			namespaces = namespace ? [namespace] : [];
		}
	}

	function onClusterPick(value: string): void {
		if (value === ADD) {
			editor = { initial: null };
			return;
		}
		if (value && value !== clusterId) connect(value);
	}

	function onNamespacePick(value: string): void {
		namespace = value;
		selection = null;
		saveLast(clusterId, value || null);
	}

	function go(v: View): void {
		view = v;
		selection = null;
	}

	function open(s: Selection): void {
		if (s.type === 'pod') view = 'pods';
		else if (s.type === 'workload') view = 'workloads';
		else if (s.type === 'helm') view = 'helm';
		selection = s;
	}

	/** An event's object: its list, searched for its name. */
	function openEvent(e: EventRow): void {
		const [kind, ...rest] = e.object.split('/');
		const target = VIEW_OF_KIND[kind];
		if (!target) return;
		view = target;
		selection = null;
		search = rest.join('/');
	}

	function refresh(): void {
		tick += 1;
		loadNamespaces();
	}

	function toggleNav(): void {
		navCollapsed = !navCollapsed;
		try {
			localStorage.setItem('reach.k8s.nav', navCollapsed ? 'collapsed' : 'open');
		} catch {
			// Not remembered then.
		}
	}

	function onSaved(c: ClusterView): void {
		loadClusters().then(() => connect(c.id));
	}

	function onDeleted(id: string): void {
		if (id === clusterId) {
			if (opened) k8sClose(opened.key).catch(() => {});
			opened = null;
			clusterId = '';
			selection = null;
		}
		loadClusters(true);
	}

	// Refresh while the page is on screen: visible tab, mounted and shown,
	// and nothing open for editing.
	$effect(() => {
		if (!opened) return;
		const visible = () => document.visibilityState === 'visible' && !!root?.isConnected && root.getClientRects().length > 0;
		const timer = setInterval(() => {
			if (visible() && !editing) tick += 1;
		}, REFRESH_MS);
		const onVisibility = () => {
			if (visible() && !editing) tick += 1;
		};
		document.addEventListener('visibilitychange', onVisibility);
		return () => {
			clearInterval(timer);
			document.removeEventListener('visibilitychange', onVisibility);
		};
	});

	function onKeydown(e: KeyboardEvent): void {
		if (e.key !== '/' || e.ctrlKey || e.metaKey || e.altKey) return;
		const el = e.target as HTMLElement | null;
		if (el && (el.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(el.tagName))) return;
		if (!root?.isConnected || root.getClientRects().length === 0 || !searchBox) return;
		e.preventDefault();
		searchBox.focus();
	}

	let nsOptions = $derived(namespace && !namespaces.includes(namespace) ? [namespace, ...namespaces] : namespaces);
	let listView = $derived(view === 'overview' ? null : view);
</script>

<svelte:window onkeydown={onKeydown} />

<div class="k8s" class:production={ctxEnv === 'production' && !!cluster} bind:this={root}>
	{#if clustersLoaded && clusters.length === 0 && !clustersError}
		<Welcome onadd={() => (editor = { initial: null })} />
	{:else}
		<div class="topbar">
			<div class="group">
				<select
					class="cluster"
					aria-label={t('k8s.cluster')}
					value={clusterId}
					onchange={(e) => {
						onClusterPick(e.currentTarget.value);
						e.currentTarget.value = clusterId;
					}}
				>
					{#if !clusterId}<option value="">{t('k8s.pick_cluster')}</option>{/if}
					{#each clusters as c (c.id)}
						<option value={c.id}>{c.name}</option>
					{/each}
					<option value={ADD}>{t('k8s.add_cluster_option')}</option>
				</select>
				{#if cluster}
					<button type="button" class="icon" onclick={() => (editor = { initial: cluster })} aria-label={t('k8s.edit_cluster')} title={t('k8s.edit_cluster')}>
						<FaIcon icon={faGear} />
					</button>
				{/if}
				{#if connecting}
					<span class="state"><span class="spinner" aria-hidden="true"></span>{t('k8s.connecting')}</span>
				{:else if opened}
					<span class="state ok" title={t('k8s.connected_version', { version: opened.version })}>{opened.version}</span>
				{:else if connectError}
					<span class="state bad"><FaIcon icon={faTriangleExclamation} size={11} /> {t('k8s.not_connected')}</span>
				{/if}
				{#if cluster}
					<EnvBadge environment={cluster.environment} readOnly={ctxReadOnly} />
				{/if}
			</div>

			{#if opened}
				<div class="group grow">
					<select class="ns" aria-label={t('k8s.namespace')} value={namespace} onchange={(e) => onNamespacePick(e.currentTarget.value)}>
						<option value="">{t('k8s.all_namespaces')}</option>
						{#each nsOptions as n (n)}
							<option value={n}>{n}</option>
						{/each}
					</select>
					<input
						bind:this={searchBox}
						bind:value={search}
						class="search"
						type="search"
						placeholder={t('k8s.search_placeholder')}
						aria-label={t('k8s.search')}
						spellcheck="false"
						autocapitalize="off"
						onkeydown={(e) => e.key === 'Escape' && (search = '')}
					/>
					<button
						type="button"
						class="chip"
						class:on={problemsOnly}
						aria-pressed={problemsOnly}
						onclick={() => (problemsOnly = !problemsOnly)}
					>
						<FaIcon icon={faTriangleExclamation} size={12} />
						{t('k8s.problems_only')}
					</button>
					<button type="button" class="icon" onclick={refresh} aria-label={t('k8s.refresh')} title={t('k8s.refresh')}>
						<FaIcon icon={faRotateRight} />
					</button>
				</div>
			{/if}
		</div>

		{#if clustersError}
			<div class="center"><ErrorHelp error={clustersError} onretry={() => loadClusters(true)} /></div>
		{:else if !clusterId}
			<div class="center muted">{clustersLoaded ? t('k8s.pick_cluster_hint') : t('common.loading')}</div>
		{:else if connecting}
			<div class="center muted"><span class="spinner big" aria-hidden="true"></span>{t('k8s.connecting_to', { name: cluster?.name ?? '' })}</div>
		{:else if connectError}
			<div class="center">
				<ErrorHelp error={connectError} onretry={() => connect(clusterId)} />
				{#if cluster}
					<button type="button" class="link" onclick={() => (editor = { initial: cluster })}>{t('k8s.check_settings')}</button>
				{/if}
			</div>
		{:else if ctx}
			<div class="body">
				{#if narrow}
					{#if !selection}
						<nav class="chips" aria-label={t('k8s.sections')}>
							{#each NAV.flatMap((g) => g.items) as v (v)}
								<button type="button" class="navchip" class:on={view === v} aria-current={view === v ? 'page' : undefined} onclick={() => go(v)}>
									<FaIcon icon={ICONS[v]} size={12} />
									{t(`k8s.nav_${v}`)}
								</button>
							{/each}
						</nav>
					{/if}
				{:else}
					<nav class="nav" class:collapsed={navCollapsed} aria-label={t('k8s.sections')}>
						<div class="nav-scroll">
							{#each NAV as g (g.group || 'top')}
								{#if g.group && !navCollapsed}
									<div class="nav-group">{t(`k8s.group_${g.group}`)}</div>
								{:else if g.group}
									<div class="nav-sep"></div>
								{/if}
								{#each g.items as v (v)}
									<button
										type="button"
										class="nav-item"
										class:on={view === v}
										aria-current={view === v ? 'page' : undefined}
										title={navCollapsed ? t(`k8s.nav_${v}`) : undefined}
										aria-label={navCollapsed ? t(`k8s.nav_${v}`) : undefined}
										onclick={() => go(v)}
									>
										<FaIcon icon={ICONS[v]} />
										{#if !navCollapsed}<span>{t(`k8s.nav_${v}`)}</span>{/if}
									</button>
								{/each}
							{/each}
						</div>
						<button type="button" class="nav-toggle" onclick={toggleNav} aria-label={navCollapsed ? t('k8s.expand_nav') : t('k8s.collapse_nav')}>
							<FaIcon icon={navCollapsed ? faAnglesRight : faAnglesLeft} />
						</button>
					</nav>
				{/if}

				<div class="main">
					<!-- Kept mounted under a phone's full-screen detail: it keeps
					     refreshing the selected row, and its scroll position. -->
					<section class="list-pane" class:shrunk={!!selection && !narrow} class:hidden={narrow && !!selection}>
						{#if listView}
							<ResourceList
								{ctx}
								view={listView}
								{tick}
								{search}
								{problemsOnly}
								{selectedId}
								onselect={(s) => (selection = s)}
								onfresh={(s) => (selection = s)}
								onevent={openEvent}
							/>
						{:else}
							<Overview {ctx} {tick} onopen={open} ongo={go} onevent={openEvent} />
						{/if}
					</section>
					{#if selection}
						<section class="detail-pane" class:full={narrow}>
							{#if selection.type === 'pod'}
								<PodDetail
									{ctx}
									row={selection.row}
									{narrow}
									{tick}
									onclose={() => (selection = null)}
									onchanged={() => (tick += 1)}
									onediting={(v) => (editing = v)}
								/>
							{:else if selection.type === 'workload'}
								<WorkloadDetail
									{ctx}
									row={selection.row}
									{narrow}
									{tick}
									onclose={() => (selection = null)}
									onchanged={() => (tick += 1)}
									onediting={(v) => (editing = v)}
									onopenpod={(p) => open({ type: 'pod', row: p })}
								/>
							{:else if selection.type === 'object'}
								<ObjectDetail
									{ctx}
									kind={selection.kind}
									row={selection.row}
									{narrow}
									onclose={() => (selection = null)}
									onchanged={() => {
										tick += 1;
										if (selection?.type === 'object' && selection.kind === 'Namespace') loadNamespaces();
									}}
									onediting={(v) => (editing = v)}
								/>
							{:else}
								<HelmDetail {ctx} row={selection.row} {via} {narrow} {tick} onclose={() => (selection = null)} onchanged={() => (tick += 1)} />
							{/if}
						</section>
					{/if}
				</div>
			</div>
		{/if}
	{/if}
</div>

{#if editor}
	<ClusterEditor open initial={editor.initial} onclose={() => (editor = null)} onsaved={onSaved} ondeleted={onDeleted} />
{/if}

<style>
	.k8s {
		display: flex;
		flex-direction: column;
		height: 100%;
		min-height: 0;
		background: var(--color-bg-primary);
		border-top: 3px solid transparent;
	}

	/* Production is never one glance away from being missed. */
	.k8s.production {
		border-top-color: var(--color-danger);
	}

	.topbar {
		display: flex;
		align-items: center;
		gap: 8px 14px;
		flex-wrap: wrap;
		padding: 6px 10px;
		border-bottom: 1px solid var(--color-border);
		background: var(--color-bg-elevated);
		flex-shrink: 0;
	}

	.group {
		display: flex;
		align-items: center;
		gap: 6px;
		min-width: 0;
	}

	.group.grow {
		flex: 1;
		justify-content: flex-end;
	}

	select,
	.search {
		min-height: 30px;
		padding: 0 8px;
		border-radius: var(--radius-sm);
		border: 1px solid var(--color-border);
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		font: inherit;
		font-size: var(--text-sm);
		min-width: 0;
	}

	.cluster {
		max-width: 220px;
		font-weight: 600;
	}

	.ns {
		max-width: 200px;
		font-family: var(--font-mono);
		font-size: var(--text-xs);
	}

	.search {
		flex: 0 1 240px;
		width: 240px;
	}

	select:focus,
	.search:focus {
		outline: none;
		border-color: var(--color-accent);
	}

	.icon {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 30px;
		height: 30px;
		flex-shrink: 0;
		border: none;
		border-radius: var(--radius-btn);
		background: none;
		color: var(--color-text-secondary);
		cursor: pointer;
	}

	.icon:hover {
		background: var(--color-surface-hover);
		color: var(--color-text-primary);
	}

	.chip {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		min-height: 30px;
		padding: 0 10px;
		border-radius: 999px;
		border: 1px solid var(--color-border);
		background: none;
		color: var(--color-text-secondary);
		font: inherit;
		font-size: var(--text-xs);
		white-space: nowrap;
		cursor: pointer;
	}

	.chip.on {
		border-color: var(--color-warning);
		color: var(--color-warning);
		background: color-mix(in srgb, var(--color-warning) 10%, transparent);
	}

	.icon:focus-visible,
	.chip:focus-visible,
	.nav-item:focus-visible,
	.nav-toggle:focus-visible,
	.navchip:focus-visible,
	.link:focus-visible {
		outline: 2px solid var(--color-accent);
		outline-offset: 1px;
	}

	.state {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		padding: 1px 8px;
		border-radius: 999px;
		font-size: var(--text-xs);
		font-family: var(--font-mono);
		color: var(--color-text-secondary);
		background: var(--color-bg-primary);
		white-space: nowrap;
	}

	.state.ok {
		color: var(--color-success);
	}

	.state.bad {
		color: var(--color-danger);
		font-family: inherit;
	}

	.spinner {
		display: inline-block;
		width: 10px;
		height: 10px;
		border-radius: 50%;
		border: 2px solid var(--color-border);
		border-top-color: var(--color-accent);
		animation: spin 0.8s linear infinite;
	}

	.spinner.big {
		width: 18px;
		height: 18px;
		margin-right: 10px;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.spinner {
			animation: none;
		}
	}

	.center {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 12px;
		flex: 1;
		padding: 24px 16px;
		overflow: auto;
	}

	.muted {
		flex-direction: row;
		font-size: var(--text-sm);
		color: var(--color-text-secondary);
	}

	.link {
		min-height: 34px;
		padding: 0 14px;
		border-radius: var(--radius-btn);
		border: 1px solid var(--color-border);
		background: var(--color-bg-elevated);
		color: var(--color-text-primary);
		font: inherit;
		font-size: var(--text-sm);
		cursor: pointer;
	}

	.body {
		flex: 1;
		min-height: 0;
		display: flex;
	}

	.nav {
		display: flex;
		flex-direction: column;
		width: 190px;
		flex-shrink: 0;
		border-right: 1px solid var(--color-border);
		background: var(--color-bg-elevated);
	}

	.nav.collapsed {
		width: 48px;
	}

	.nav-scroll {
		flex: 1;
		overflow-y: auto;
		padding: 6px;
	}

	.nav-group {
		padding: 10px 8px 4px;
		font-size: 0.6875rem;
		font-weight: 600;
		letter-spacing: 0.04em;
		text-transform: uppercase;
		color: var(--color-text-tertiary);
	}

	.nav-sep {
		height: 1px;
		margin: 6px 4px;
		background: var(--color-border);
	}

	.nav-item {
		display: flex;
		align-items: center;
		gap: 10px;
		width: 100%;
		min-height: 30px;
		padding: 0 8px;
		border: none;
		border-radius: var(--radius-btn);
		background: none;
		color: var(--color-text-secondary);
		font: inherit;
		font-size: var(--text-sm);
		text-align: left;
		cursor: pointer;
	}

	.collapsed .nav-item {
		justify-content: center;
		padding: 0;
	}

	.nav-item:hover {
		background: var(--color-surface-hover);
		color: var(--color-text-primary);
	}

	.nav-item.on {
		background: color-mix(in srgb, var(--color-accent) 14%, transparent);
		color: var(--color-text-primary);
	}

	.nav-item.on :global(svg) {
		color: var(--color-accent);
	}

	.nav-toggle {
		height: 32px;
		border: none;
		border-top: 1px solid var(--color-border);
		background: none;
		color: var(--color-text-tertiary);
		cursor: pointer;
	}

	.nav-toggle:hover {
		color: var(--color-text-primary);
	}

	.main {
		flex: 1;
		min-width: 0;
		min-height: 0;
		display: flex;
	}

	.list-pane {
		flex: 1;
		min-width: 0;
		min-height: 0;
	}

	.list-pane.shrunk {
		flex: 1 1 50%;
	}

	.list-pane.hidden {
		display: none;
	}

	.detail-pane {
		flex: 0 0 clamp(380px, 48%, 760px);
		min-width: 0;
		min-height: 0;
		border-left: 1px solid var(--color-border);
		background: var(--color-bg-primary);
	}

	.detail-pane.full {
		flex: 1;
		border-left: none;
	}

	/* A phone: the sections become a row of chips under the bar. */
	.chips {
		display: flex;
		gap: 6px;
		padding: 8px 10px;
		overflow-x: auto;
		scrollbar-width: none;
		border-bottom: 1px solid var(--color-border);
		flex-shrink: 0;
	}

	.navchip {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		min-height: 44px;
		padding: 0 14px;
		border-radius: 999px;
		border: 1px solid var(--color-border);
		background: var(--color-bg-elevated);
		color: var(--color-text-secondary);
		font: inherit;
		font-size: var(--text-sm);
		white-space: nowrap;
		cursor: pointer;
	}

	.navchip.on {
		border-color: var(--color-accent);
		color: var(--color-text-primary);
		background: color-mix(in srgb, var(--color-accent) 14%, transparent);
	}

	@media (max-width: 759px) {
		.body {
			flex-direction: column;
		}

		.group.grow {
			flex-basis: 100%;
			flex-wrap: wrap;
			justify-content: flex-start;
		}

		.search {
			flex: 1 1 140px;
			width: auto;
		}

		select,
		.search,
		.chip {
			min-height: 44px;
		}

		.icon {
			width: 44px;
			height: 44px;
		}

		/* Which cluster you are on is never squeezed away: its name takes the
		   first line (the settings button beside it), version and badges wrap
		   below. On production, that name is what keeps you on the right one. */
		.topbar > .group:first-child {
			flex-basis: 100%;
			flex-wrap: wrap;
		}

		.cluster {
			flex: 1 1 calc(100% - 56px);
			min-width: 0;
			max-width: none;
		}

		.ns {
			max-width: none;
			flex: 1 1 120px;
		}
	}
</style>
