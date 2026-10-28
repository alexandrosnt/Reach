<script lang="ts">
	/**
	 * One pod: its state in words with what usually causes it, its logs, the
	 * events about it, and its YAML. Deleting a pod that a controller owns
	 * only replaces it, and the confirmation says so, since "delete and let
	 * it come back" is the usual way to restart one.
	 */
	import { untrack } from 'svelte';
	import StatusPill from '#lib/components/devops/StatusPill.svelte';
	import LogSources from '#lib/components/devops/LogSources.svelte';
	import ConfirmAction from '#lib/components/devops/ConfirmAction.svelte';
	import ErrorHelp from '#lib/components/devops/ErrorHelp.svelte';
	import DetailHeader from './DetailHeader.svelte';
	import ActionButton from './ActionButton.svelte';
	import Tabs from './Tabs.svelte';
	import EventList from './EventList.svelte';
	import YamlPane from './YamlPane.svelte';
	import { k8sDelete, k8sEvents, k8sLogs, k8sLogsFiles, k8sLogFiles, type EventRow, type PodRow } from '#lib/ipc/k8s.js';
	import type { LogView } from '#lib/ipc/containers.js';
	import { podStatus, ago } from '#lib/devops/status.js';
	import { busy } from '#lib/state/k8s.svelte.js';
	import { addToast } from '#lib/state/toasts.svelte.js';
	import { t } from '#lib/state/i18n.svelte.js';
	import type { Ctx } from './model';

	interface Props {
		ctx: Ctx;
		row: PodRow;
		narrow: boolean;
		tick: number;
		onclose: () => void;
		/** Something changed on the cluster: refresh the list. */
		onchanged: () => void;
		onediting: (editing: boolean) => void;
	}

	let { ctx, row, narrow, tick, onclose, onchanged, onediting }: Props = $props();

	let tab = $state('overview');
	let container = $state<string | null>(null);
	let events = $state<EventRow[]>([]);
	let eventsError = $state('');
	let confirming = $state(false);

	let status = $derived(podStatus(row.status, row.restarts));

	// Another pod: back to its overview, first container.
	// The list refreshes every few seconds with fresh row objects; only a
	// different object (a new identity string, so the effect re-runs) resets
	// the pane, never a refresh of the same one.
	let identity = $derived(`${row.namespace}/${row.name}`);
	$effect(() => {
		void identity;
		untrack(() => {
			tab = 'overview';
			container = row.containers[0] ?? null;
			events = [];
		});
	});

	/** What usually causes a state, in a sentence, for the people who have not met it yet. */
	let hint = $derived.by(() => {
		switch (row.status) {
			case 'CrashLoopBackOff':
			case 'Error':
				return t('k8s.hint_crash');
			case 'ImagePullBackOff':
			case 'ErrImagePull':
			case 'InvalidImageName':
				return t('k8s.hint_image');
			case 'OOMKilled':
				return t('k8s.hint_oom');
			case 'Pending':
				return t('k8s.hint_pending');
			case 'CreateContainerConfigError':
			case 'CreateContainerError':
				return t('k8s.hint_config');
			case 'Evicted':
				return t('k8s.hint_evicted');
			default:
				return '';
		}
	});

	// The stream restarts only when what it follows changes, not on every
	// refresh of the row.
	let logTarget = $derived(`${ctx.key}\n${row.namespace}\n${row.name}\n${container ?? ''}`);
	let logFns = $derived.by(() => {
		void logTarget;
		return untrack(() => {
			const { key } = ctx;
			const { namespace, name } = row;
			const c = container;
			return {
				output: (view: LogView) => (onData: (chunk: string) => void) => k8sLogs(key, namespace, name, c, view, onData),
				files: (paths: string[], lines: number) => (onData: (chunk: string) => void) => k8sLogsFiles(key, namespace, name, c, paths, lines, onData),
				listFiles: () => k8sLogFiles(key, namespace, name, c),
				target: { name, namespace, container: c ?? row.containers[0] ?? null }
			};
		});
	});

	async function loadEvents(): Promise<void> {
		try {
			const all = await k8sEvents(ctx.key, row.namespace);
			events = all.filter((e) => e.object === `Pod/${row.name}`);
			eventsError = '';
		} catch (e) {
			eventsError = String(e);
		}
	}

	$effect(() => {
		void tick;
		if (tab === 'events') untrack(loadEvents);
	});

	async function remove(): Promise<void> {
		try {
			await busy(() => k8sDelete(ctx.key, 'Pod', row.namespace, row.name));
			addToast(t('k8s.deleted', { name: row.name }), 'success');
			onclose();
			onchanged();
		} catch (e) {
			addToast(String(e), 'error', 8000);
		}
	}
</script>

<div class="detail">
	<DetailHeader kind="Pod" name={row.name} namespace={row.namespace} {status} {narrow} readOnly={ctx.readOnly} {onclose}>
		{#snippet actions()}
			<ActionButton variant="danger" locked={ctx.readOnly} onclick={() => (confirming = true)}>{t('common.delete')}</ActionButton>
		{/snippet}
	</DetailHeader>

	<Tabs
		tabs={[
			{ id: 'overview', label: t('k8s.tab_overview') },
			{ id: 'logs', label: t('k8s.tab_logs') },
			{ id: 'events', label: t('k8s.tab_events') },
			{ id: 'yaml', label: t('k8s.tab_yaml') }
		]}
		active={tab}
		onchange={(id) => (tab = id)}
	/>

	<div class="body">
		{#if tab === 'overview'}
			<div class="scroll">
				{#if hint}
					<div class="hint" class:bad={status.tone === 'bad'}>
						<StatusPill {status} />
						<p>{hint}</p>
					</div>
				{/if}
				<dl class="facts">
					<dt>{t('k8s.col_ready')}</dt>
					<dd>{row.ready}</dd>
					<dt>{t('k8s.col_restarts')}</dt>
					<dd class:warn={row.restarts > 0}>{row.restarts}</dd>
					<dt>{t('k8s.col_node')}</dt>
					<dd class="mono">{row.node || t('k8s.not_scheduled')}</dd>
					<dt>{t('k8s.owner')}</dt>
					<dd class="mono">{row.owner ?? t('k8s.no_owner')}</dd>
					<dt>{t('k8s.col_age')}</dt>
					<dd>{ago(row.created)}</dd>
					<dt>{t('k8s.containers')}</dt>
					<dd class="mono">{row.containers.join(', ')}</dd>
				</dl>
			</div>
		{:else if tab === 'logs'}
			<div class="logs">
				{#if row.containers.length > 1}
					<label class="picker">
						<span>{t('k8s.container')}</span>
						<select bind:value={container}>
							{#each row.containers as c (c)}
								<option value={c}>{c}</option>
							{/each}
						</select>
					</label>
				{/if}
				<div class="viewer">
					{#key logTarget}
						<LogSources
							name={container ? `${row.name}-${container}` : row.name}
							target={logFns.target}
							owner={ctx.clusterId}
							kind="k8s"
							output={logFns.output}
							files={logFns.files}
							listFiles={logFns.listFiles}
						/>
					{/key}
				</div>
			</div>
		{:else if tab === 'events'}
			<div class="scroll">
				{#if eventsError}
					<ErrorHelp error={eventsError} onretry={loadEvents} />
				{:else if events.length}
					<EventList {events} showObject={false} />
				{:else}
					<p class="muted">{t('k8s.no_events_pod')}</p>
				{/if}
			</div>
		{:else}
			<YamlPane {ctx} kind="Pod" namespace={row.namespace} name={row.name} {onediting} />
		{/if}
	</div>
</div>

<ConfirmAction
	open={confirming}
	title={t('k8s.delete_pod_title')}
	message={t('k8s.delete_pod_message')}
	items={[`${row.namespace}/${row.name}`]}
	where={ctx.clusterName}
	environment={ctx.environment}
	note={row.owner ? t('k8s.delete_pod_owned', { owner: row.owner }) : t('k8s.delete_pod_bare')}
	confirmLabel={t('common.delete')}
	typeToConfirm={ctx.environment === 'production' ? row.name : undefined}
	onconfirm={remove}
	onclose={() => (confirming = false)}
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
		gap: 12px;
	}

	.hint {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 6px;
		padding: 10px 12px;
		border-radius: var(--radius-sm);
		border: 1px solid color-mix(in srgb, var(--color-warning) 40%, var(--color-border));
		background: color-mix(in srgb, var(--color-warning) 7%, var(--color-bg-elevated));
	}

	.hint.bad {
		border-color: color-mix(in srgb, var(--color-danger) 40%, var(--color-border));
		background: color-mix(in srgb, var(--color-danger) 6%, var(--color-bg-elevated));
	}

	.hint p {
		margin: 0;
		font-size: var(--text-sm);
		line-height: 1.45;
		color: var(--color-text-secondary);
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

	.logs {
		flex: 1;
		min-height: 0;
		display: flex;
		flex-direction: column;
	}

	.picker {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 6px 10px;
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
		border-bottom: 1px solid var(--color-border);
	}

	.picker select {
		min-height: 30px;
		padding: 0 8px;
		border-radius: var(--radius-sm);
		border: 1px solid var(--color-border);
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		font-family: var(--font-mono);
		font-size: var(--text-xs);
	}

	.viewer {
		flex: 1;
		min-height: 0;
	}

	.muted {
		margin: 0;
		font-size: var(--text-sm);
		color: var(--color-text-tertiary);
	}
</style>
