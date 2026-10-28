<script lang="ts">
	/**
	 * Any other object (a Service, ConfigMap, Secret, volume claim, Job,
	 * node, namespace): what it is, its YAML, and Delete.
	 *
	 * Deleting a namespace deletes everything in it, so it is always typed
	 * out, whatever the environment.
	 */
	import { untrack } from 'svelte';
	import ConfirmAction from '#lib/components/devops/ConfirmAction.svelte';
	import DetailHeader from './DetailHeader.svelte';
	import ActionButton from './ActionButton.svelte';
	import Tabs from './Tabs.svelte';
	import YamlPane from './YamlPane.svelte';
	import { k8sDelete, type Kind, type ObjectRow } from '#lib/ipc/k8s.js';
	import { ago } from '#lib/devops/status.js';
	import { busy } from '#lib/state/k8s.svelte.js';
	import { addToast } from '#lib/state/toasts.svelte.js';
	import { t } from '#lib/state/i18n.svelte.js';
	import { CLUSTER_SCOPED, nodeStatus, type Ctx } from './model';

	interface Props {
		ctx: Ctx;
		kind: Kind;
		row: ObjectRow;
		narrow: boolean;
		onclose: () => void;
		onchanged: () => void;
		onediting: (editing: boolean) => void;
	}

	let { ctx, kind, row, narrow, onclose, onchanged, onediting }: Props = $props();

	let tab = $state('overview');
	let confirming = $state(false);
	let scoped = $derived(CLUSTER_SCOPED.includes(kind));
	let ns = $derived(scoped ? null : row.namespace);

	// The list refreshes every few seconds with fresh row objects; only a
	// different object (a new identity string, so the effect re-runs) resets
	// the pane, never a refresh of the same one.
	let identity = $derived(`${kind}/${row.namespace}/${row.name}`);
	$effect(() => {
		void identity;
		untrack(() => (tab = 'overview'));
	});

	let note = $derived(
		kind === 'Namespace'
			? t('k8s.delete_namespace_note')
			: kind === 'Node'
				? t('k8s.delete_node_note')
				: kind === 'PersistentVolumeClaim'
					? t('k8s.delete_pvc_note')
					: t('k8s.delete_object_note')
	);

	async function remove(): Promise<void> {
		try {
			await busy(() => k8sDelete(ctx.key, kind, ns, row.name));
			addToast(t('k8s.deleted', { name: row.name }), 'success');
			onclose();
			onchanged();
		} catch (e) {
			addToast(String(e), 'error', 8000);
		}
	}
</script>

<div class="detail">
	<DetailHeader
		{kind}
		name={row.name}
		namespace={scoped ? '' : row.namespace}
		status={kind === 'Node' ? nodeStatus(row.summary) : null}
		{narrow}
		readOnly={ctx.readOnly}
		{onclose}
	>
		{#snippet actions()}
			<ActionButton variant="danger" locked={ctx.readOnly} onclick={() => (confirming = true)}>{t('common.delete')}</ActionButton>
		{/snippet}
	</DetailHeader>

	<Tabs
		tabs={[
			{ id: 'overview', label: t('k8s.tab_overview') },
			{ id: 'yaml', label: t('k8s.tab_yaml') }
		]}
		active={tab}
		onchange={(id) => (tab = id)}
	/>

	<div class="body">
		{#if tab === 'overview'}
			<div class="scroll">
				<dl class="facts">
					<dt>{t('k8s.kind')}</dt>
					<dd>{kind}</dd>
					{#if !scoped}
						<dt>{t('k8s.namespace')}</dt>
						<dd class="mono">{row.namespace}</dd>
					{/if}
					{#if row.summary}
						<dt>{t('k8s.col_details')}</dt>
						<dd class="mono">{row.summary}</dd>
					{/if}
					<dt>{t('k8s.col_age')}</dt>
					<dd>{ago(row.created)}</dd>
				</dl>
				{#if kind === 'Secret'}
					<p class="muted">{t('k8s.secret_overview')}</p>
				{/if}
			</div>
		{:else}
			<YamlPane {ctx} {kind} namespace={ns} name={row.name} {onediting} onsaved={onchanged} />
		{/if}
	</div>
</div>

<ConfirmAction
	open={confirming}
	title={t('k8s.delete_object_title', { kind })}
	message={t('k8s.delete_object_message', { kind })}
	items={[scoped ? row.name : `${row.namespace}/${row.name}`]}
	where={ctx.clusterName}
	environment={ctx.environment}
	{note}
	confirmLabel={t('common.delete')}
	typeToConfirm={kind === 'Namespace' || ctx.environment === 'production' ? row.name : undefined}
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

	.muted {
		margin: 0;
		font-size: var(--text-xs);
		line-height: 1.45;
		color: var(--color-text-tertiary);
	}
</style>
