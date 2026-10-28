<script lang="ts">
	/**
	 * A Deployment, StatefulSet or DaemonSet: how many of its pods are ready,
	 * which images it runs, its pods, and its YAML; scale, restart, delete.
	 *
	 * Scaling to zero stops the service, so it is confirmed with the number
	 * to come back to, and offered back as "Restore" right after.
	 */
	import { untrack } from 'svelte';
	import StatusPill from '#lib/components/devops/StatusPill.svelte';
	import ConfirmAction from '#lib/components/devops/ConfirmAction.svelte';
	import ErrorHelp from '#lib/components/devops/ErrorHelp.svelte';
	import DetailHeader from './DetailHeader.svelte';
	import ActionButton from './ActionButton.svelte';
	import Tabs from './Tabs.svelte';
	import YamlPane from './YamlPane.svelte';
	import { k8sDelete, k8sPods, k8sRestart, k8sScale, type Kind, type PodRow, type WorkloadRow } from '#lib/ipc/k8s.js';
	import { podStatus, replicaStatus, ago } from '#lib/devops/status.js';
	import { busy } from '#lib/state/k8s.svelte.js';
	import { addToast } from '#lib/state/toasts.svelte.js';
	import { t } from '#lib/state/i18n.svelte.js';
	import type { Ctx } from './model';

	interface Props {
		ctx: Ctx;
		row: WorkloadRow;
		narrow: boolean;
		tick: number;
		onclose: () => void;
		onchanged: () => void;
		onediting: (editing: boolean) => void;
		onopenpod: (pod: PodRow) => void;
	}

	let { ctx, row, narrow, tick, onclose, onchanged, onediting, onopenpod }: Props = $props();

	let kind = $derived(row.kind as Kind);
	let scalable = $derived(kind === 'Deployment' || kind === 'StatefulSet');
	let tab = $state('overview');
	let replicas = $state(0);
	let touched = $state(false);
	let working = $state(false);
	let pending = $state<'scale' | 'restart' | 'delete' | null>(null);
	/** After scaling to zero: the count to put back. */
	let undo = $state<{ name: string; to: number } | null>(null);
	let pods = $state<PodRow[]>([]);
	let podsError = $state('');

	// The list refreshes every few seconds with fresh row objects; only a
	// different object (a new identity string, so the effect re-runs) resets
	// the pane, never a refresh of the same one.
	let identity = $derived(`${row.kind}/${row.namespace}/${row.name}`);
	$effect(() => {
		void identity;
		untrack(() => {
			tab = 'overview';
			touched = false;
			pods = [];
			if (undo && undo.name !== row.name) undo = null;
		});
	});

	// Follow the cluster's count until the user starts changing it.
	$effect(() => {
		void row.name;
		void row.namespace;
		const desired = row.desired;
		if (!untrack(() => touched)) replicas = desired;
	});

	function step(by: number): void {
		replicas = Math.max(0, (Number(replicas) || 0) + by);
		touched = true;
	}

	function applyScale(): void {
		const n = Math.max(0, Math.floor(Number(replicas) || 0));
		if (n === row.desired) return;
		if (n === 0 || ctx.environment === 'production') pending = 'scale';
		else scale(n, row.desired);
	}

	async function scale(n: number, from: number): Promise<void> {
		working = true;
		try {
			await busy(() => k8sScale(ctx.key, kind, row.namespace, row.name, n));
			touched = false;
			replicas = n;
			if (n === 0) {
				undo = { name: row.name, to: from };
				addToast(t('k8s.scaled_zero_toast', { name: row.name, n: from }), 'success', 6000);
			} else {
				undo = null;
				addToast(t('k8s.scaled_toast', { name: row.name, n }), 'success');
			}
			onchanged();
		} catch (e) {
			addToast(String(e), 'error', 8000);
		} finally {
			working = false;
		}
	}

	async function restart(): Promise<void> {
		working = true;
		try {
			await busy(() => k8sRestart(ctx.key, kind, row.namespace, row.name));
			addToast(t('k8s.restarted_toast', { name: row.name }), 'success');
			onchanged();
		} catch (e) {
			addToast(String(e), 'error', 8000);
		} finally {
			working = false;
		}
	}

	async function remove(): Promise<void> {
		try {
			await busy(() => k8sDelete(ctx.key, kind, row.namespace, row.name));
			addToast(t('k8s.deleted', { name: row.name }), 'success');
			onclose();
			onchanged();
		} catch (e) {
			addToast(String(e), 'error', 8000);
		}
	}

	function confirmed(): void {
		if (pending === 'scale') scale(Math.max(0, Math.floor(Number(replicas) || 0)), row.desired);
		else if (pending === 'restart') restart();
		else if (pending === 'delete') remove();
	}

	/** A Deployment's pods belong to its ReplicaSets, named after it. */
	function ownedBy(p: PodRow): boolean {
		if (!p.owner) return false;
		if (p.owner === `${row.kind}/${row.name}`) return true;
		return row.kind === 'Deployment' && p.owner.startsWith(`ReplicaSet/${row.name}-`);
	}

	async function loadPods(): Promise<void> {
		try {
			pods = (await k8sPods(ctx.key, row.namespace)).filter(ownedBy);
			podsError = '';
		} catch (e) {
			podsError = String(e);
		}
	}

	$effect(() => {
		void tick;
		if (tab === 'pods') untrack(loadPods);
	});

	let target = $derived(Math.max(0, Math.floor(Number(replicas) || 0)));

	let ask = $derived.by(() => {
		const item = `${row.kind} ${row.namespace}/${row.name}`;
		switch (pending) {
			case 'scale':
				return {
					title: t('k8s.scale_title'),
					message: target === 0 ? t('k8s.scale_zero_message') : t('k8s.scale_message', { from: row.desired, to: target }),
					note: t('k8s.scale_restore_note', { n: row.desired }),
					label: t('k8s.scale_to', { n: target }),
					item
				};
			case 'restart':
				return { title: t('k8s.restart_title'), message: t('k8s.restart_message'), note: t('k8s.restart_note'), label: t('k8s.restart'), item };
			default:
				return { title: t('k8s.delete_workload_title'), message: t('k8s.delete_workload_message'), note: t('k8s.delete_workload_note'), label: t('common.delete'), item };
		}
	});
</script>

<div class="detail">
	<DetailHeader kind={row.kind} name={row.name} namespace={row.namespace} status={replicaStatus(row.ready, row.desired)} {narrow} readOnly={ctx.readOnly} {onclose}>
		{#snippet actions()}
			<ActionButton locked={ctx.readOnly} disabled={working} onclick={() => (pending = 'restart')}>{t('k8s.restart')}</ActionButton>
			<ActionButton variant="danger" locked={ctx.readOnly} disabled={working} onclick={() => (pending = 'delete')}>{t('common.delete')}</ActionButton>
		{/snippet}
	</DetailHeader>

	{#if undo && undo.name === row.name}
		<div class="undo" role="status">
			<span>{t('k8s.scaled_zero_banner', { n: undo.to })}</span>
			<ActionButton variant="primary" locked={ctx.readOnly} disabled={working} onclick={() => undo && scale(undo.to, 0)}>{t('k8s.restore_to', { n: undo.to })}</ActionButton>
			<button type="button" class="dismiss" onclick={() => (undo = null)} aria-label={t('k8s.dismiss')}>×</button>
		</div>
	{/if}

	<Tabs
		tabs={[
			{ id: 'overview', label: t('k8s.tab_overview') },
			{ id: 'pods', label: t('k8s.nav_pods') },
			{ id: 'yaml', label: t('k8s.tab_yaml') }
		]}
		active={tab}
		onchange={(id) => (tab = id)}
	/>

	<div class="body">
		{#if tab === 'overview'}
			<div class="scroll">
				<dl class="facts">
					<dt>{t('k8s.desired')}</dt>
					<dd>{row.desired}</dd>
					<dt>{t('k8s.col_ready')}</dt>
					<dd class:warn={row.ready < row.desired}>{row.ready}</dd>
					<dt>{t('k8s.available')}</dt>
					<dd>{row.available}</dd>
					<dt>{t('k8s.col_age')}</dt>
					<dd>{ago(row.created)}</dd>
					<dt>{t('k8s.col_images')}</dt>
					<dd class="mono">
						{#each row.images as img (img)}<div>{img}</div>{/each}
					</dd>
				</dl>

				{#if scalable}
					<section class="scale">
						<h3>{t('k8s.scale_heading')}</h3>
						<p class="muted">{t('k8s.scale_explain')}</p>
						<div class="stepper">
							<button type="button" onclick={() => step(-1)} disabled={ctx.readOnly || target === 0} aria-label={t('k8s.fewer')}>−</button>
							<input
								type="number"
								min="0"
								inputmode="numeric"
								aria-label={t('k8s.replicas')}
								bind:value={replicas}
								oninput={() => (touched = true)}
								disabled={ctx.readOnly}
								onkeydown={(e) => e.key === 'Enter' && applyScale()}
							/>
							<button type="button" onclick={() => step(1)} disabled={ctx.readOnly} aria-label={t('k8s.more')}>+</button>
							<ActionButton variant="primary" locked={ctx.readOnly} disabled={working || target === row.desired} onclick={applyScale}>
								{t('k8s.scale_to', { n: target })}
							</ActionButton>
						</div>
					</section>
				{:else if row.kind === 'DaemonSet'}
					<p class="muted">{t('k8s.daemonset_explain')}</p>
				{/if}
			</div>
		{:else if tab === 'pods'}
			<div class="scroll">
				{#if podsError}
					<ErrorHelp error={podsError} onretry={loadPods} />
				{:else if pods.length === 0}
					<p class="muted">{t('k8s.no_pods_workload')}</p>
				{:else}
					<ul class="pods">
						{#each pods as p (p.name)}
							<li>
								<button type="button" onclick={() => onopenpod(p)}>
									<span class="pname">{p.name}</span>
									<StatusPill status={podStatus(p.status, p.restarts)} compact />
									<span class="muted">{p.ready}</span>
								</button>
							</li>
						{/each}
					</ul>
				{/if}
			</div>
		{:else}
			<YamlPane {ctx} {kind} namespace={row.namespace} name={row.name} {onediting} onsaved={onchanged} />
		{/if}
	</div>
</div>

<ConfirmAction
	open={pending !== null}
	title={ask.title}
	message={ask.message}
	items={[ask.item]}
	where={ctx.clusterName}
	environment={ctx.environment}
	note={ask.note}
	confirmLabel={ask.label}
	typeToConfirm={ctx.environment === 'production' ? row.name : undefined}
	onconfirm={confirmed}
	onclose={() => (pending = null)}
/>

<style>
	.detail {
		display: flex;
		flex-direction: column;
		height: 100%;
		min-height: 0;
	}

	.body {
		flex: 1;
		min-height: 0;
		display: flex;
		flex-direction: column;
	}

	.scroll {
		flex: 1;
		overflow: auto;
		padding: 12px;
		display: flex;
		flex-direction: column;
		gap: 14px;
	}

	.undo {
		display: flex;
		align-items: center;
		gap: 10px;
		flex-wrap: wrap;
		padding: 8px 12px;
		border-bottom: 1px solid var(--color-border);
		background: color-mix(in srgb, var(--color-accent) 8%, var(--color-bg-elevated));
		font-size: var(--text-sm);
		color: var(--color-text-primary);
	}

	.undo span {
		flex: 1;
	}

	.dismiss {
		width: 32px;
		height: 32px;
		border: none;
		border-radius: var(--radius-btn);
		background: none;
		color: var(--color-text-secondary);
		font-size: 18px;
		cursor: pointer;
	}

	.facts {
		display: grid;
		grid-template-columns: max-content 1fr;
		gap: 6px 16px;
		margin: 0;
		font-size: var(--text-sm);
	}

	dt {
		color: var(--color-text-secondary);
	}

	dd {
		margin: 0;
		color: var(--color-text-primary);
		overflow-wrap: anywhere;
	}

	.mono {
		font-family: var(--font-mono);
		font-size: var(--text-xs);
	}

	.warn {
		color: var(--color-warning);
		font-weight: 600;
	}

	.scale {
		display: flex;
		flex-direction: column;
		gap: 8px;
		padding-top: 12px;
		border-top: 1px solid var(--color-border);
	}

	h3 {
		margin: 0;
		font-size: var(--text-sm);
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.muted {
		margin: 0;
		font-size: var(--text-xs);
		line-height: 1.45;
		color: var(--color-text-tertiary);
	}

	.stepper {
		display: flex;
		align-items: center;
		gap: 6px;
		flex-wrap: wrap;
	}

	.stepper button {
		width: 36px;
		height: 36px;
		border-radius: var(--radius-btn);
		border: 1px solid var(--color-border);
		background: var(--color-bg-elevated);
		color: var(--color-text-primary);
		font-size: 18px;
		cursor: pointer;
	}

	.stepper button:disabled {
		opacity: 0.45;
		cursor: not-allowed;
	}

	.stepper input {
		width: 80px;
		height: 36px;
		padding: 0 8px;
		border-radius: var(--radius-sm);
		border: 1px solid var(--color-border);
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		font-size: var(--text-sm);
		text-align: center;
	}

	.stepper input:focus,
	.stepper button:focus-visible,
	.dismiss:focus-visible {
		outline: 2px solid var(--color-accent);
		outline-offset: 1px;
	}

	.pods {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.pods button {
		display: flex;
		align-items: center;
		gap: 10px;
		width: 100%;
		min-height: 36px;
		padding: 6px 10px;
		border-radius: var(--radius-sm);
		border: 1px solid var(--color-border);
		background: var(--color-bg-elevated);
		color: var(--color-text-primary);
		font: inherit;
		text-align: left;
		cursor: pointer;
	}

	.pods button:hover {
		background: var(--color-surface-hover);
	}

	.pods button:focus-visible {
		outline: 2px solid var(--color-accent);
	}

	.pname {
		flex: 1;
		min-width: 0;
		font-family: var(--font-mono);
		font-size: var(--text-xs);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	@media (pointer: coarse) {
		.stepper button,
		.stepper input,
		.dismiss {
			width: 44px;
			height: 44px;
		}

		.stepper input {
			width: 88px;
		}

		.pods button {
			min-height: 44px;
		}
	}
</style>
