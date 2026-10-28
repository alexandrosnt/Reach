<script lang="ts">
	/**
	 * The first screen: is anything wrong, and what. Counts first, then the
	 * pods and workloads that need attention, then the latest warnings, each
	 * one tap from its detail. Every part loads on its own, so a user who
	 * may not list nodes still sees their pods.
	 */
	import { untrack } from 'svelte';
	import StatusPill from '#lib/components/devops/StatusPill.svelte';
	import ErrorHelp from '#lib/components/devops/ErrorHelp.svelte';
	import EventList from './EventList.svelte';
	import { k8sEvents, k8sObjects, k8sPods, k8sWorkloads, type EventRow, type ObjectRow, type PodRow, type WorkloadRow } from '#lib/ipc/k8s.js';
	import { podStatus, replicaStatus } from '#lib/devops/status.js';
	import { t } from '#lib/state/i18n.svelte.js';
	import { nodeStatus, podProblem, workloadProblem, type Ctx, type Selection, type View } from './model';

	interface Props {
		ctx: Ctx;
		tick: number;
		onopen: (s: Selection) => void;
		ongo: (v: View) => void;
		onevent: (e: EventRow) => void;
	}

	let { ctx, tick, onopen, ongo, onevent }: Props = $props();

	let pods = $state<PodRow[] | null>(null);
	let workloads = $state<WorkloadRow[] | null>(null);
	let events = $state<EventRow[] | null>(null);
	let nodes = $state<ObjectRow[] | null>(null);
	let error = $state('');
	let nodesHidden = $state(false);
	let seq = 0;

	async function load(key: string, ns: string | null): Promise<void> {
		const mine = ++seq;
		const [p, w, e, n] = await Promise.allSettled([k8sPods(key, ns), k8sWorkloads(key, ns), k8sEvents(key, ns), k8sObjects(key, 'Node', null)]);
		if (mine !== seq) return;
		if (p.status === 'fulfilled') pods = p.value;
		if (w.status === 'fulfilled') workloads = w.value;
		if (e.status === 'fulfilled') events = e.value;
		// Nodes are cluster-wide; a namespaced user is often not allowed to list them.
		if (n.status === 'fulfilled') nodes = n.value;
		nodesHidden = n.status === 'rejected';
		error = p.status === 'rejected' ? String(p.reason) : '';
	}

	$effect(() => {
		const key = ctx.key;
		const ns = ctx.namespace;
		void tick;
		untrack(() => load(key, ns));
	});

	// Back to "loading" only when what is shown changes, not on every refresh.
	$effect(() => {
		void ctx.key;
		void ctx.namespace;
		untrack(() => {
			pods = workloads = events = nodes = null;
		});
	});

	let badPods = $derived((pods ?? []).filter(podProblem));
	let running = $derived((pods ?? []).filter((p) => p.status === 'Running' && !podProblem(p)).length);
	let badWorkloads = $derived((workloads ?? []).filter(workloadProblem));
	let warnings = $derived((events ?? []).filter((e) => e.kind === 'Warning'));
	let readyNodes = $derived((nodes ?? []).filter((n) => nodeStatus(n.summary).tone === 'ok').length);
	let issues = $derived(badPods.length + badWorkloads.length + ((nodes?.length ?? 0) - readyNodes));
	let loaded = $derived(pods !== null);
</script>

<div class="overview">
	{#if error && !pods}
		<ErrorHelp {error} onretry={() => load(ctx.key, ctx.namespace)} />
	{:else}
		<div class="headline" class:ok={loaded && issues === 0} class:bad={issues > 0}>
			{#if !loaded}
				<span>{t('common.loading')}</span>
			{:else if issues === 0}
				<strong>{t('k8s.all_good')}</strong>
				<span>{t('k8s.all_good_detail', { pods: running })}</span>
			{:else}
				<strong>{t('k8s.needs_attention', { count: issues })}</strong>
				<span>{t('k8s.needs_attention_detail')}</span>
			{/if}
		</div>

		<div class="cards">
			<button type="button" class="card" onclick={() => ongo('pods')}>
				<span class="label">{t('k8s.nav_pods')}</span>
				<span class="big">{pods?.length ?? '–'}</span>
				<span class="sub">
					<span class="ok">{t('k8s.count_running', { count: running })}</span>
					{#if badPods.length}<span class="bad">{t('k8s.count_problems', { count: badPods.length })}</span>{/if}
				</span>
			</button>
			<button type="button" class="card" onclick={() => ongo('workloads')}>
				<span class="label">{t('k8s.nav_workloads')}</span>
				<span class="big">{workloads?.length ?? '–'}</span>
				<span class="sub">
					{#if badWorkloads.length}
						<span class="warn">{t('k8s.count_not_ready', { count: badWorkloads.length })}</span>
					{:else if workloads}
						<span class="ok">{t('k8s.count_all_ready')}</span>
					{/if}
				</span>
			</button>
			<button type="button" class="card" onclick={() => ongo('nodes')}>
				<span class="label">{t('k8s.nav_nodes')}</span>
				<span class="big">{nodesHidden ? '–' : (nodes?.length ?? '–')}</span>
				<span class="sub">
					{#if nodesHidden}
						<span>{t('k8s.nodes_hidden')}</span>
					{:else if nodes}
						<span class:ok={readyNodes === nodes.length} class:bad={readyNodes < nodes.length}>
							{t('k8s.count_nodes_ready', { ready: readyNodes, total: nodes.length })}
						</span>
					{/if}
				</span>
			</button>
			<button type="button" class="card" onclick={() => ongo('events')}>
				<span class="label">{t('k8s.warnings')}</span>
				<span class="big">{events ? warnings.length : '–'}</span>
				<span class="sub"><span>{t('k8s.warnings_sub')}</span></span>
			</button>
		</div>

		{#if badPods.length || badWorkloads.length}
			<section>
				<h3>{t('k8s.attention_heading')}</h3>
				<ul class="list">
					{#each badPods.slice(0, 8) as p (`${p.namespace}/${p.name}`)}
						<li>
							<button type="button" onclick={() => onopen({ type: 'pod', row: p })}>
								<span class="kind">Pod</span>
								<span class="name">{p.name}</span>
								{#if !ctx.namespace}<span class="ns">{p.namespace}</span>{/if}
								<StatusPill status={podStatus(p.status, p.restarts)} />
							</button>
						</li>
					{/each}
					{#each badWorkloads.slice(0, 5) as w (`${w.kind}/${w.namespace}/${w.name}`)}
						<li>
							<button type="button" onclick={() => onopen({ type: 'workload', row: w })}>
								<span class="kind">{w.kind}</span>
								<span class="name">{w.name}</span>
								{#if !ctx.namespace}<span class="ns">{w.namespace}</span>{/if}
								<StatusPill status={replicaStatus(w.ready, w.desired)} />
							</button>
						</li>
					{/each}
				</ul>
				{#if badPods.length > 8}
					<button type="button" class="more" onclick={() => ongo('pods')}>{t('k8s.see_all_problems', { count: badPods.length })}</button>
				{/if}
			</section>
		{/if}

		<section>
			<h3>{t('k8s.recent_warnings')}</h3>
			{#if warnings.length}
				<EventList events={warnings.slice(0, 5)} showNamespace={!ctx.namespace} onopen={onevent} />
				{#if warnings.length > 5}
					<button type="button" class="more" onclick={() => ongo('events')}>{t('k8s.see_all_events')}</button>
				{/if}
			{:else if events}
				<p class="muted">{t('k8s.no_warnings')}</p>
			{/if}
		</section>
	{/if}
</div>

<style>
	.overview {
		display: flex;
		flex-direction: column;
		gap: 16px;
		padding: 16px;
		overflow: auto;
		height: 100%;
		max-width: 1000px;
	}

	.headline {
		display: flex;
		flex-direction: column;
		gap: 2px;
		padding: 14px 16px;
		border-radius: 10px;
		border: 1px solid var(--color-border);
		border-left: 4px solid var(--color-border);
		background: var(--color-bg-elevated);
		font-size: var(--text-sm);
		color: var(--color-text-secondary);
	}

	.headline strong {
		font-size: 15px;
		color: var(--color-text-primary);
	}

	.headline.ok {
		border-left-color: var(--color-success);
	}

	.headline.bad {
		border-left-color: var(--color-danger);
	}

	.cards {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(170px, 1fr));
		gap: 10px;
	}

	.card {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 4px;
		padding: 12px 14px;
		border-radius: 10px;
		border: 1px solid var(--color-border);
		background: var(--color-bg-elevated);
		color: var(--color-text-primary);
		font: inherit;
		text-align: left;
		cursor: pointer;
	}

	.card:hover {
		border-color: var(--color-accent);
	}

	.card:focus-visible,
	.list button:focus-visible,
	.more:focus-visible {
		outline: 2px solid var(--color-accent);
		outline-offset: 1px;
	}

	.label {
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
	}

	.big {
		font-size: 24px;
		font-weight: 600;
		line-height: 1.1;
	}

	.sub {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
		font-size: var(--text-xs);
		color: var(--color-text-tertiary);
	}

	.ok {
		color: var(--color-success);
	}

	.warn {
		color: var(--color-warning);
	}

	.bad {
		color: var(--color-danger);
	}

	section {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	h3 {
		margin: 0;
		font-size: var(--text-sm);
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.list {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.list button {
		display: flex;
		align-items: center;
		gap: 10px;
		width: 100%;
		min-height: 40px;
		padding: 6px 10px;
		border-radius: var(--radius-sm);
		border: 1px solid var(--color-border);
		background: var(--color-bg-elevated);
		color: var(--color-text-primary);
		font: inherit;
		font-size: var(--text-sm);
		text-align: left;
		cursor: pointer;
	}

	.list button:hover {
		background: var(--color-surface-hover);
	}

	.kind {
		width: 86px;
		flex-shrink: 0;
		font-size: var(--text-xs);
		color: var(--color-text-tertiary);
	}

	.name {
		flex: 1;
		min-width: 0;
		font-family: var(--font-mono);
		font-size: var(--text-xs);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.ns {
		font-size: var(--text-xs);
		color: var(--color-text-tertiary);
	}

	.more {
		align-self: flex-start;
		min-height: 32px;
		padding: 0 10px;
		border: none;
		border-radius: var(--radius-btn);
		background: none;
		color: var(--color-accent);
		font: inherit;
		font-size: var(--text-sm);
		cursor: pointer;
	}

	.muted {
		margin: 0;
		font-size: var(--text-sm);
		color: var(--color-text-tertiary);
	}

	@media (max-width: 759px) {
		.overview {
			padding: 12px;
		}

		.cards {
			grid-template-columns: repeat(2, 1fr);
		}

		.kind {
			width: auto;
		}

		.list button {
			min-height: 44px;
			flex-wrap: wrap;
		}
	}
</style>
