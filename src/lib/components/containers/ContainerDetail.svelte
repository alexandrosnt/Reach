<script lang="ts">
	/**
	 * One container: what it is, its live log, and what can be done with it.
	 * Logs come first, the most common reason to open a container. A shell
	 * opens as a terminal tab on the same host.
	 */
	import StatusPill from '$lib/components/devops/StatusPill.svelte';
	import LogSources from '$lib/components/devops/LogSources.svelte';
	import { ctrInspect, ctrLogs, ctrLogsFiles, ctrLogFiles, type ContainerRow, type Engine, type LogView } from '$lib/ipc/containers';
	import { containerStatus, ago } from '$lib/devops/status';
	import { t } from '$lib/state/i18n.svelte';

	interface Props {
		hostKey: string;
		engine: Engine;
		container: ContainerRow;
		readOnly: boolean;
		/** Run an action; the page confirms the ones that need it. */
		onaction: (action: 'start' | 'stop' | 'restart' | 'pause' | 'unpause' | 'kill' | 'remove' | 'shell') => void;
		onback?: () => void;
		/** The saved host's id, where log files are remembered. */
		owner: string | null;
		/** Open the log file picker (the row menu's "Log files…"). */
		pickFiles?: boolean;
	}

	let { hostKey, engine, container, readOnly, onaction, onback, owner, pickFiles = $bindable(false) }: Props = $props();

	let tab = $state<'logs' | 'details' | 'inspect'>('logs');
	let inspect = $state('');
	let inspectError = $state('');

	let status = $derived(containerStatus(container.state, container.status));
	let running = $derived(container.state === 'running' || container.state === 'restarting');
	let paused = $derived(container.state === 'paused');

	// A new stream for each container; the viewer stops the old one. Keyed
	// on the id alone: the list refreshes every few seconds with new but
	// equal rows, and following those restarted the log and threw the reader
	// back to the end (#94).
	let containerId = $derived(container.id);
	let containerName = $derived(container.name);
	let project = $derived(container.project);
	let service = $derived(container.service);
	let target = $derived({ name: containerName, project, service });
	let output = $derived.by(() => {
		const key = hostKey;
		const id = containerId;
		return (view: LogView) => (onData: (c: string) => void) => ctrLogs(key, id, view, onData);
	});
	let files = $derived.by(() => {
		const key = hostKey;
		const id = containerId;
		return (paths: string[], lines: number) => (onData: (c: string) => void) => ctrLogsFiles(key, id, paths, lines, onData);
	});
	let listFiles = $derived.by(() => {
		const key = hostKey;
		const id = containerId;
		return () => ctrLogFiles(key, id);
	});

	// "Log files…" from the row menu lands on the Logs tab.
	$effect(() => {
		if (pickFiles) tab = 'logs';
	});

	$effect(() => {
		if (tab !== 'inspect') return;
		inspect = '';
		inspectError = '';
		ctrInspect(hostKey, 'container', container.id)
			.then((json) => (inspect = json))
			.catch((e) => (inspectError = String(e)));
	});

	const roTitle = $derived(readOnly ? t('env.read_only_hint') : undefined);
</script>

<section class="detail">
	<header class="head">
		{#if onback}
			<button type="button" class="back" onclick={onback} aria-label={t('common.back')}>‹</button>
		{/if}
		<div class="title">
			<div class="name" title={container.name}>{container.name}</div>
			<div class="sub">
				<StatusPill {status} />
				<span class="image" title={container.image}>{container.image}</span>
			</div>
		</div>
	</header>

	<div class="actions">
		{#if running}
			<button type="button" disabled={readOnly} title={roTitle} onclick={() => onaction('stop')}>■ {t('ctr.stop')}</button>
			<button type="button" disabled={readOnly} title={roTitle} onclick={() => onaction('restart')}>↻ {t('ctr.restart')}</button>
		{:else if paused}
			<button type="button" disabled={readOnly} title={roTitle} onclick={() => onaction('unpause')}>▶ {t('ctr.resume')}</button>
		{:else}
			<button type="button" class="primary" disabled={readOnly} title={roTitle} onclick={() => onaction('start')}>▶ {t('ctr.start')}</button>
		{/if}
		<button type="button" disabled={!running} title={running ? undefined : t('ctr.shell_needs_running')} onclick={() => onaction('shell')}>›_ {t('ctr.shell')}</button>
		<span class="spacer"></span>
		{#if running && !paused}
			<button type="button" class="quiet" disabled={readOnly} title={roTitle} onclick={() => onaction('pause')}>{t('ctr.pause')}</button>
			<button type="button" class="quiet" disabled={readOnly} title={roTitle} onclick={() => onaction('kill')}>{t('ctr.kill')}</button>
		{/if}
		<button type="button" class="danger" disabled={readOnly} title={roTitle} onclick={() => onaction('remove')}>{t('ctr.remove')}</button>
	</div>

	<div class="tabs" role="tablist">
		<button type="button" role="tab" aria-selected={tab === 'logs'} class:on={tab === 'logs'} onclick={() => (tab = 'logs')}>{t('ctr.logs')}</button>
		<button type="button" role="tab" aria-selected={tab === 'details'} class:on={tab === 'details'} onclick={() => (tab = 'details')}>{t('ctr.details')}</button>
		<button type="button" role="tab" aria-selected={tab === 'inspect'} class:on={tab === 'inspect'} onclick={() => (tab = 'inspect')}>{t('ctr.inspect')}</button>
	</div>

	<div class="body">
		{#if tab === 'logs'}
			{#key containerId}
				<LogSources name={containerName} {target} {owner} kind="docker" {output} {files} {listFiles} bind:pick={pickFiles} />
			{/key}
		{:else if tab === 'details'}
			<dl class="facts">
				<dt>{t('ctr.status')}</dt>
				<dd>{container.status}</dd>
				<dt>{t('ctr.image')}</dt>
				<dd class="mono">{container.image}</dd>
				<dt>{t('ctr.created')}</dt>
				<dd>{ago(container.created)}</dd>
				<dt>{t('ctr.ports')}</dt>
				<dd class="mono">{container.ports.length ? container.ports.join('\n') : t('ctr.no_ports')}</dd>
				{#if container.project}
					<dt>{t('ctr.stack')}</dt>
					<dd>{container.project} / {container.service}</dd>
				{/if}
				<dt>{t('ctr.engine')}</dt>
				<dd>{engine === 'docker' ? 'Docker' : 'Podman'}</dd>
				<dt>ID</dt>
				<dd class="mono">{container.id}</dd>
			</dl>
		{:else}
			{#if inspectError}
				<p class="err">{inspectError}</p>
			{:else if !inspect}
				<p class="muted">{t('common.loading')}</p>
			{:else}
				<pre class="json">{inspect}</pre>
			{/if}
		{/if}
	</div>
</section>

<style>
	.detail {
		display: flex;
		flex-direction: column;
		min-height: 0;
		height: 100%;
		background: var(--color-bg-elevated);
	}

	.head {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 12px 14px 6px;
	}

	.back {
		min-width: 44px;
		min-height: 44px;
		border: none;
		background: none;
		color: var(--color-text-primary);
		font-size: 1.6rem;
		cursor: pointer;
	}

	.title {
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.name {
		font-size: 1rem;
		font-weight: 600;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.sub {
		display: flex;
		align-items: center;
		gap: 8px;
		min-width: 0;
	}

	.image {
		font-family: var(--font-mono);
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.actions {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 6px;
		padding: 6px 14px 10px;
	}

	.actions button {
		min-height: 32px;
		padding: 0 12px;
		border-radius: var(--radius-btn);
		border: 1px solid var(--color-border);
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		font: inherit;
		font-size: var(--text-xs);
		font-weight: 500;
		cursor: pointer;
		white-space: nowrap;
	}

	.actions button:hover:not(:disabled) {
		border-color: var(--color-accent);
	}

	.actions button:disabled {
		opacity: 0.45;
		cursor: not-allowed;
	}

	.actions .primary {
		background: var(--color-accent);
		border-color: var(--color-accent);
		color: #fff;
	}

	.actions .quiet {
		border-color: transparent;
		background: none;
		color: var(--color-text-secondary);
	}

	.actions .danger {
		color: var(--color-danger);
	}

	.spacer {
		flex: 1;
	}

	.tabs {
		display: flex;
		gap: 2px;
		padding: 0 10px;
		border-bottom: 1px solid var(--color-border);
		overflow-x: auto;
	}

	.tabs button {
		min-height: 36px;
		padding: 0 12px;
		border: none;
		border-bottom: 2px solid transparent;
		background: none;
		color: var(--color-text-secondary);
		font: inherit;
		font-size: var(--text-sm);
		cursor: pointer;
	}

	.tabs button.on {
		color: var(--color-text-primary);
		border-bottom-color: var(--color-accent);
	}

	.body {
		flex: 1;
		min-height: 0;
		overflow: auto;
	}

	.facts {
		display: grid;
		grid-template-columns: max-content 1fr;
		gap: 8px 16px;
		margin: 0;
		padding: 14px;
		font-size: var(--text-sm);
	}

	dt {
		color: var(--color-text-secondary);
	}

	dd {
		margin: 0;
		min-width: 0;
		overflow-wrap: anywhere;
		white-space: pre-wrap;
	}

	.mono,
	.json {
		font-family: var(--font-mono);
		font-size: var(--text-xs);
	}

	.json {
		margin: 0;
		padding: 12px 14px;
		white-space: pre;
		user-select: text;
	}

	.muted {
		padding: 14px;
		color: var(--color-text-tertiary);
	}

	.err {
		padding: 14px;
		color: var(--color-danger);
	}
</style>
