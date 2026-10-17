<script lang="ts">
	/**
	 * The Containers workspace: Docker or Podman on a saved host.
	 *
	 * The host is always named at the top, with what it is for, because the
	 * costly mistakes happen on the wrong host. Stacks (Compose projects) come
	 * first: people think in apps, not single containers. A list on the left
	 * and the chosen container on the right; on a phone the container opens
	 * full screen. Lists refresh every few seconds while the page is in view,
	 * and not at all when it is not.
	 */
	import { onMount } from 'svelte';
	import StatusPill from '$lib/components/devops/StatusPill.svelte';
	import EnvBadge from '$lib/components/devops/EnvBadge.svelte';
	import ConfirmAction from '$lib/components/devops/ConfirmAction.svelte';
	import ErrorHelp from '$lib/components/devops/ErrorHelp.svelte';
	import HostEditor from '$lib/components/containers/HostEditor.svelte';
	import ContainerDetail from '$lib/components/containers/ContainerDetail.svelte';
	import DbMenu, { type MenuItem } from '$lib/components/db/DbMenu.svelte';
	import {
		ctrHosts,
		ctrOpen,
		ctrContainers,
		ctrProjects,
		ctrImages,
		ctrVolumes,
		ctrNetworks,
		ctrAct,
		ctrRemove,
		ctrCompose,
		type ContainerHost,
		type ContainerRow,
		type Project,
		type ImageRow,
		type VolumeRow,
		type NetworkRow,
		type Opened,
		type ContainerAction,
		type ComposeAction
	} from '$lib/ipc/containers';
	import { containerStatus, ago, bytes, type Status } from '$lib/devops/status';
	import { openContainerShell } from '$lib/devops/shell';
	import { busy } from '$lib/state/containers.svelte';
	import { getActivePage } from '$lib/state/navigation.svelte';
	import { addToast } from '$lib/state/toasts.svelte';
	import { t } from '$lib/state/i18n.svelte';

	type Section = 'stacks' | 'containers' | 'images' | 'volumes' | 'networks';

	const LAST_HOST = 'reach.containers.host';
	const REFRESH_MS = 5000;

	let hosts = $state<ContainerHost[]>([]);
	let hostsLoaded = $state(false);
	let hostId = $state('');
	let opened = $state<Opened | null>(null);
	let connecting = $state(false);
	let connectError = $state('');

	let section = $state<Section>('stacks');
	let containers = $state<ContainerRow[]>([]);
	let projects = $state<Project[]>([]);
	let images = $state<ImageRow[]>([]);
	let volumes = $state<VolumeRow[]>([]);
	let networks = $state<NetworkRow[]>([]);
	let listError = $state('');

	let query = $state('');
	let problemsOnly = $state(false);
	let selectedId = $state('');
	/** The row menu, where it was opened (#101 asked for "Log files…" there). */
	let rowMenu = $state<{ x: number; y: number; c: ContainerRow } | null>(null);
	let pickFiles = $state(false);
	let openStacks = $state<Record<string, boolean>>({});
	let working = $state<Record<string, boolean>>({});

	let editorOpen = $state(false);
	let editing = $state<ContainerHost | null>(null);
	let output = $state<{ title: string; text: string } | null>(null);

	interface Confirm {
		title: string;
		message: string;
		items: string[];
		note?: string;
		confirmLabel: string;
		typeToConfirm?: string;
		run: () => Promise<void>;
	}
	let confirm = $state<Confirm | null>(null);

	let narrow = $state(false);
	let searchEl: HTMLInputElement | undefined = $state();

	let host = $derived(hosts.find((h) => h.id === hostId) ?? null);
	let readOnly = $derived(opened?.readOnly ?? host?.readOnly ?? false);
	let selected = $derived(containers.find((c) => c.id === selectedId) ?? null);
	let needle = $derived(query.trim().toLowerCase());

	function isProblem(c: ContainerRow): boolean {
		const s = containerStatus(c.state, c.status);
		return s.tone === 'bad' || s.tone === 'warn';
	}

	let shownContainers = $derived(
		containers.filter((c) => (!problemsOnly || isProblem(c)) && (!needle || `${c.name} ${c.image} ${c.project ?? ''}`.toLowerCase().includes(needle)))
	);
	let looseContainers = $derived(shownContainers.filter((c) => !c.project));
	let shownProjects = $derived(
		projects.filter(
			(p) =>
				(!needle || p.name.toLowerCase().includes(needle) || shownContainers.some((c) => c.project === p.name)) &&
				(!problemsOnly || containers.some((c) => c.project === p.name && isProblem(c)))
		)
	);
	let problemCount = $derived(containers.filter(isProblem).length);

	function projectStatus(p: Project): Status {
		const bad = containers.some((c) => c.project === p.name && containerStatus(c.state, c.status).tone === 'bad');
		if (bad) return { tone: 'bad', label: t('ctr.stack_problem', { running: p.running, total: p.total }), raw: `${p.running}/${p.total}` };
		if (p.running === 0) return { tone: 'idle', label: t('status.stopped'), raw: `0/${p.total}` };
		if (p.running < p.total) return { tone: 'warn', label: t('ctr.running_of', { running: p.running, total: p.total }), raw: `${p.running}/${p.total}` };
		return { tone: 'ok', label: t('ctr.running_of', { running: p.running, total: p.total }), raw: `${p.running}/${p.total}` };
	}

	// ---------------------------------------------------------------------
	// Hosts and connecting

	async function loadHosts(): Promise<void> {
		try {
			hosts = await ctrHosts();
		} catch (e) {
			connectError = String(e);
		}
		hostsLoaded = true;
		if (!hostId) {
			let last = '';
			try {
				last = localStorage.getItem(LAST_HOST) ?? '';
			} catch {
				// Storage unavailable: start from the most recently used.
			}
			const pick = hosts.find((h) => h.id === last) ?? hosts[0];
			if (pick) void connect(pick.id);
		}
	}

	async function connect(id: string): Promise<void> {
		hostId = id;
		opened = null;
		connectError = '';
		listError = '';
		selectedId = '';
		containers = [];
		projects = [];
		images = [];
		volumes = [];
		networks = [];
		try {
			localStorage.setItem(LAST_HOST, id);
		} catch {
			// Not remembered; harmless.
		}
		connecting = true;
		try {
			const o = await ctrOpen(id);
			if (hostId !== id) return;
			opened = o;
			await refresh();
			if (section === 'stacks' && projects.length === 0) section = 'containers';
		} catch (e) {
			if (hostId === id) connectError = String(e);
		} finally {
			if (hostId === id) connecting = false;
		}
	}

	// ---------------------------------------------------------------------
	// Lists

	let refreshing = false;
	async function refresh(): Promise<void> {
		const o = opened;
		if (!o || refreshing) return;
		refreshing = true;
		try {
			// Containers always: stacks are counted from them.
			const [cs, ps] = await Promise.all([ctrContainers(o.key), ctrProjects(o.key)]);
			if (opened?.key !== o.key) return;
			containers = cs;
			// A few stacks start open, so their containers show at once;
			// many start closed, so the list stays a summary.
			for (const p of ps) if (!(p.name in openStacks)) openStacks[p.name] = ps.length <= 3;
			projects = ps;
			if (section === 'images') images = await ctrImages(o.key);
			if (section === 'volumes') volumes = await ctrVolumes(o.key);
			if (section === 'networks') networks = await ctrNetworks(o.key);
			listError = '';
		} catch (e) {
			listError = String(e);
		} finally {
			refreshing = false;
		}
	}

	// A new section loads at once.
	$effect(() => {
		void section;
		if (opened) void refresh();
	});

	// Refresh while visible; nothing while the page or the window is hidden.
	onMount(() => {
		void loadHosts();
		const timer = setInterval(() => {
			if (document.visibilityState === 'visible' && getActivePage() === 'containers' && !confirm && !editorOpen) void refresh();
		}, REFRESH_MS);
		const mq = window.matchMedia('(max-width: 760px)');
		const onMq = () => (narrow = mq.matches);
		onMq();
		mq.addEventListener('change', onMq);
		const onKey = (e: KeyboardEvent) => {
			if (getActivePage() !== 'containers') return;
			const typing = e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement || e.target instanceof HTMLSelectElement;
			if (e.key === '/' && !typing) {
				e.preventDefault();
				searchEl?.focus();
			}
		};
		window.addEventListener('keydown', onKey);
		return () => {
			clearInterval(timer);
			mq.removeEventListener('change', onMq);
			window.removeEventListener('keydown', onKey);
		};
	});

	// ---------------------------------------------------------------------
	// Actions

	const where = $derived(host?.name ?? '');
	const production = $derived(host?.environment === 'production');

	async function run(label: string, id: string, work: () => Promise<unknown>): Promise<void> {
		working = { ...working, [id]: true };
		try {
			await busy(work);
			await refresh();
		} catch (e) {
			addToast(`${label}: ${e}`, 'error', 10000);
		} finally {
			const { [id]: _, ...rest } = working;
			working = rest;
		}
	}

	function copy(text: string): void {
		navigator.clipboard.writeText(text).then(
			() => addToast(t('ctr.copied'), 'success'),
			(e) => addToast(String(e), 'error')
		);
	}

	function rowMenuItems(c: ContainerRow): MenuItem[] {
		const running = c.state === 'running';
		return [
			{ label: t('ctr.logs'), action: () => (selectedId = c.id) },
			{
				label: t('logsrc.menu_files'),
				action: () => {
					selectedId = c.id;
					pickFiles = true;
				}
			},
			{ label: t('ctr.shell'), disabled: !running, hint: running ? undefined : t('ctr.shell_needs_running'), action: () => containerAction(c, 'shell') },
			{ label: '', separator: true },
			{ label: t('ctr.copy_name'), action: () => copy(c.name) },
			{ label: t('ctr.copy_id'), action: () => copy(c.id) }
		];
	}

	function containerAction(c: ContainerRow, action: ContainerAction | 'shell'): void {
		if (!opened || !host) return;
		const key = opened.key;
		if (action === 'shell') {
			openContainerShell(host.route, host.engine, c.id, c.name).catch((e) => addToast(String(e), 'error'));
			return;
		}
		const go = () => run(c.name, c.id, () => ctrAct(key, c.id, action));
		if (action === 'remove') {
			confirm = {
				title: t('ctr.remove_container_title'),
				message: t('ctr.remove_container_message'),
				items: [c.name],
				note: t('ctr.remove_container_note'),
				confirmLabel: t('ctr.remove'),
				run: go
			};
		} else if (action === 'kill') {
			confirm = { title: t('ctr.kill_title'), message: t('ctr.kill_message'), items: [c.name], confirmLabel: t('ctr.kill'), run: go };
		} else if (production && (action === 'stop' || action === 'restart' || action === 'pause')) {
			confirm = {
				title: action === 'stop' ? t('ctr.stop_title') : action === 'restart' ? t('ctr.restart_title') : t('ctr.pause_title'),
				message: t('ctr.production_action_message'),
				items: [c.name],
				confirmLabel: action === 'stop' ? t('ctr.stop') : action === 'restart' ? t('ctr.restart') : t('ctr.pause'),
				typeToConfirm: '',
				run: go
			};
		} else {
			void go();
		}
	}

	function stackAction(p: Project, action: ComposeAction): void {
		if (!opened) return;
		const key = opened.key;
		const go = () =>
			run(p.name, `stack:${p.name}`, async () => {
				const out = await ctrCompose(key, p.name, action);
				const last = out.trim().split('\n').filter(Boolean).at(-1) ?? '';
				addToast(t('ctr.compose_done', { action: t(`ctr.compose_${action}`), name: p.name }) + (last ? ` — ${last}` : ''), 'success');
				output = { title: `${p.name}: ${t(`ctr.compose_${action}`)}`, text: out };
			});
		if (action === 'down') {
			confirm = {
				title: t('ctr.down_title'),
				message: t('ctr.down_message'),
				items: p.services.map((s) => `${p.name} / ${s}`),
				note: t('ctr.down_note'),
				confirmLabel: t('ctr.compose_down'),
				run: go
			};
		} else if (production && (action === 'restart' || action === 'stop')) {
			confirm = {
				title: action === 'stop' ? t('ctr.stop_title') : t('ctr.restart_title'),
				message: t('ctr.production_action_message'),
				items: p.services.map((s) => `${p.name} / ${s}`),
				confirmLabel: t(`ctr.compose_${action}`),
				typeToConfirm: '',
				run: go
			};
		} else {
			void go();
		}
	}

	function removeResource(kind: 'image' | 'volume' | 'network', id: string, label: string): void {
		if (!opened) return;
		const key = opened.key;
		const go = () => run(label, `${kind}:${id}`, () => ctrRemove(key, kind, id));
		confirm = {
			title: t(`ctr.remove_${kind}_title`),
			message: t(`ctr.remove_${kind}_message`),
			items: [label],
			note: kind === 'volume' ? t('ctr.remove_volume_note') : undefined,
			confirmLabel: t('ctr.remove'),
			// A volume is data: its name is typed, wherever it is.
			typeToConfirm: kind === 'volume' ? label : undefined,
			run: go
		};
	}

	const DEFAULT_NETWORKS = new Set(['bridge', 'host', 'none', 'podman']);
	const roTitle = $derived(readOnly ? t('env.read_only_hint') : undefined);
</script>

<div class="page" class:production>
	<!-- Where you are, always: host, engine, environment. -->
	<header class="bar">
		<div class="host-pick">
			<select
				aria-label={t('ctr.host')}
				value={hostId}
				onchange={(e) => {
					const v = e.currentTarget.value;
					if (v === '__add') {
						e.currentTarget.value = hostId;
						editing = null;
						editorOpen = true;
					} else void connect(v);
				}}
			>
				{#if !hostId}
					<option value="" disabled>{t('ctr.choose_host')}</option>
				{/if}
				{#each hosts as h (h.id)}
					<option value={h.id}>{h.name}</option>
				{/each}
				<option value="__add">＋ {t('ctr.add_host')}…</option>
			</select>
			{#if host}
				<button type="button" class="icon" title={t('ctr.edit_host')} aria-label={t('ctr.edit_host')} onclick={() => ((editing = host), (editorOpen = true))}>⚙</button>
			{/if}
		</div>
		{#if host}
			<span class="chip">{host.engine === 'docker' ? 'Docker' : 'Podman'}{opened ? ` ${opened.info.version}` : ''}</span>
			<EnvBadge environment={host.environment} {readOnly} />
		{/if}
		{#if connecting}
			<span class="muted">{t('ctr.connecting')}</span>
		{/if}
		<span class="grow"></span>
		{#if opened}
			<input class="search" type="search" bind:this={searchEl} bind:value={query} placeholder={t('ctr.search')} aria-label={t('ctr.search')} />
			<label class="problems" class:has={problemCount > 0}>
				<input type="checkbox" bind:checked={problemsOnly} />
				{t('ctr.problems_only')}{problemCount > 0 ? ` (${problemCount})` : ''}
			</label>
			<button type="button" class="icon" title={t('ctr.refresh')} aria-label={t('ctr.refresh')} onclick={refresh}>↻</button>
		{/if}
	</header>

	{#if !hostsLoaded}
		<div class="center muted">{t('common.loading')}</div>
	{:else if hosts.length === 0}
		<!-- First run: say what this is and how to start. -->
		<div class="center">
			<div class="welcome">
				<div class="big">🐳</div>
				<h2>{t('ctr.welcome_title')}</h2>
				<p>{t('ctr.welcome_body')}</p>
				<ul>
					<li>{t('ctr.welcome_point_ssh')}</li>
					<li>{t('ctr.welcome_point_engines')}</li>
					<li>{t('ctr.welcome_point_safe')}</li>
				</ul>
				<button type="button" class="cta" onclick={() => ((editing = null), (editorOpen = true))}>{t('ctr.add_first_host')}</button>
			</div>
		</div>
	{:else if connectError}
		<div class="center">
			<ErrorHelp error={connectError} onretry={() => connect(hostId)} />
		</div>
	{:else if connecting && !opened}
		<div class="center muted">{t('ctr.connecting_to', { name: host?.name ?? '' })}</div>
	{:else if opened}
		<nav class="sections" aria-label={t('ctr.sections')}>
			{#each [['stacks', t('ctr.stacks'), projects.length], ['containers', t('ctr.containers'), containers.length], ['images', t('ctr.images'), -1], ['volumes', t('ctr.volumes'), -1], ['networks', t('ctr.networks'), -1]] as [id, label, count] (id)}
				<button type="button" class:on={section === id} onclick={() => ((section = id as Section), (selectedId = ''))}>
					{label}{#if (count as number) >= 0}<span class="count">{count}</span>{/if}
				</button>
			{/each}
		</nav>

		{#if listError}
			<div class="list-error">{listError}</div>
		{/if}

		<div class="main" class:with-detail={!!selected && !narrow}>
			<div class="list">
				{#if section === 'stacks'}
					{#if shownProjects.length === 0}
						<div class="empty">
							<p>{needle || problemsOnly ? t('ctr.nothing_matches') : t('ctr.no_stacks')}</p>
							{#if !needle && !problemsOnly}<small>{t('ctr.no_stacks_hint')}</small>{/if}
						</div>
					{/if}
					{#each shownProjects as p (p.name)}
						{@const st = projectStatus(p)}
						{@const members = shownContainers.filter((c) => c.project === p.name)}
						<div class="stack">
							<div class="stack-head">
								<button type="button" class="expand" aria-expanded={!!openStacks[p.name]} onclick={() => (openStacks = { ...openStacks, [p.name]: !openStacks[p.name] })}>
									<span class="caret" class:open={openStacks[p.name]}>›</span>
									<span class="stack-name">{p.name}</span>
								</button>
								<StatusPill status={st} />
								<span class="grow"></span>
								{#if working[`stack:${p.name}`]}
									<span class="muted">{t('ctr.working')}</span>
								{:else}
									{#if p.running < p.total}
										<button type="button" class="act primary" disabled={readOnly} title={roTitle} onclick={() => stackAction(p, 'up')}>▶ {t('ctr.compose_up')}</button>
									{/if}
									{#if p.running > 0}
										<button type="button" class="act" disabled={readOnly} title={roTitle} onclick={() => stackAction(p, 'restart')}>↻ {t('ctr.compose_restart')}</button>
										<button type="button" class="act" disabled={readOnly} title={roTitle} onclick={() => stackAction(p, 'stop')}>■ {t('ctr.compose_stop')}</button>
									{/if}
									<button type="button" class="act quiet" disabled={readOnly} title={roTitle} onclick={() => stackAction(p, 'pull')}>{t('ctr.compose_pull')}</button>
									<button type="button" class="act quiet danger" disabled={readOnly} title={roTitle} onclick={() => stackAction(p, 'down')}>{t('ctr.compose_down')}</button>
								{/if}
							</div>
							{#if p.workingDir}
								<div class="stack-dir" title={p.configFiles.join('\n')}>{p.workingDir}</div>
							{/if}
							{#if openStacks[p.name]}
								<div class="rows">
									{#each members as c (c.id)}
										{@render containerRow(c)}
									{/each}
								</div>
							{/if}
						</div>
					{/each}
				{:else if section === 'containers'}
					{#if shownContainers.length === 0}
						<div class="empty"><p>{needle || problemsOnly ? t('ctr.nothing_matches') : t('ctr.no_containers')}</p></div>
					{/if}
					<div class="rows">
						{#each shownContainers as c (c.id)}
							{@render containerRow(c)}
						{/each}
					</div>
					{#if looseContainers.length < shownContainers.length && !needle}
						<p class="hint">{t('ctr.stacks_hint')}</p>
					{/if}
				{:else if section === 'images'}
					{@const list = images.filter((i) => !needle || i.tags.join(' ').toLowerCase().includes(needle) || i.id.includes(needle))}
					{#if list.length === 0}<div class="empty"><p>{t('ctr.no_images')}</p></div>{/if}
					<table class="table">
						<thead><tr><th>{t('ctr.image')}</th><th>{t('ctr.size')}</th><th>{t('ctr.created')}</th><th>{t('ctr.in_use')}</th><th></th></tr></thead>
						<tbody>
							{#each list as i (i.id)}
								{@const label = i.tags[0] ?? i.id.replace('sha256:', '').slice(0, 12)}
								<tr>
									<td class="mono" title={i.tags.join('\n') || i.id}>{label}{#if i.tags.length > 1}<span class="more"> +{i.tags.length - 1}</span>{/if}</td>
									<td>{bytes(i.size)}</td>
									<td>{ago(i.created)}</td>
									<td>{i.containers > 0 ? t('ctr.used_by', { count: i.containers }) : t('ctr.unused')}</td>
									<td class="right"><button type="button" class="act quiet danger" disabled={readOnly || !!working[`image:${i.id}`]} title={roTitle} onclick={() => removeResource('image', i.id, label)}>{t('ctr.remove')}</button></td>
								</tr>
							{/each}
						</tbody>
					</table>
				{:else if section === 'volumes'}
					{@const list = volumes.filter((v) => !needle || v.name.toLowerCase().includes(needle))}
					{#if list.length === 0}<div class="empty"><p>{t('ctr.no_volumes')}</p></div>{/if}
					<table class="table">
						<thead><tr><th>{t('ctr.volume')}</th><th>{t('ctr.driver')}</th><th>{t('ctr.created')}</th><th></th></tr></thead>
						<tbody>
							{#each list as v (v.name)}
								<tr>
									<td class="mono" title={v.mountpoint}>{v.name}</td>
									<td>{v.driver}</td>
									<td>{ago(v.created)}</td>
									<td class="right"><button type="button" class="act quiet danger" disabled={readOnly || !!working[`volume:${v.name}`]} title={roTitle} onclick={() => removeResource('volume', v.name, v.name)}>{t('ctr.remove')}</button></td>
								</tr>
							{/each}
						</tbody>
					</table>
				{:else}
					{@const list = networks.filter((n) => !needle || n.name.toLowerCase().includes(needle))}
					<table class="table">
						<thead><tr><th>{t('ctr.network')}</th><th>{t('ctr.driver')}</th><th>{t('ctr.scope')}</th><th></th></tr></thead>
						<tbody>
							{#each list as n (n.id)}
								<tr>
									<td class="mono">{n.name}</td>
									<td>{n.driver}</td>
									<td>{n.scope}</td>
									<td class="right">
										{#if DEFAULT_NETWORKS.has(n.name)}
											<span class="muted" title={t('ctr.builtin_network_hint')}>{t('ctr.builtin')}</span>
										{:else}
											<button type="button" class="act quiet danger" disabled={readOnly || !!working[`network:${n.id}`]} title={roTitle} onclick={() => removeResource('network', n.id, n.name)}>{t('ctr.remove')}</button>
										{/if}
									</td>
								</tr>
							{/each}
						</tbody>
					</table>
				{/if}
			</div>

			{#if selected && opened && host}
				<div class="detail-pane" class:sheet={narrow}>
					<ContainerDetail
						hostKey={opened.key}
						owner={host.id}
						bind:pickFiles
						engine={host.engine}
						container={selected}
						{readOnly}
						onaction={(a) => containerAction(selected, a)}
						onback={narrow ? () => (selectedId = '') : undefined}
					/>
				</div>
			{/if}
		</div>
	{/if}
</div>

{#snippet containerRow(c: ContainerRow)}
	{@const st = containerStatus(c.state, c.status)}
	{@const running = c.state === 'running'}
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div
		class="row"
		class:selected={c.id === selectedId}
		oncontextmenu={(e) => {
			e.preventDefault();
			rowMenu = { x: e.clientX, y: e.clientY, c };
		}}
	>
		<button type="button" class="row-main" onclick={() => (selectedId = c.id === selectedId && !narrow ? '' : c.id)}>
			<span class="row-name">{c.service ?? c.name}</span>
			<span class="row-sub mono">{c.image}</span>
		</button>
		<StatusPill status={st} />
		<span class="row-ports mono" title={c.ports.join('\n')}>{c.ports[0] ?? ''}{c.ports.length > 1 ? ` +${c.ports.length - 1}` : ''}</span>
		<span class="row-age">{ago(c.created)}</span>
		<span class="row-acts">
			{#if working[c.id]}
				<span class="muted">…</span>
			{:else if running}
				<button type="button" class="mini" disabled={readOnly} title={roTitle ?? t('ctr.stop')} aria-label={t('ctr.stop')} onclick={() => containerAction(c, 'stop')}>■</button>
				<button type="button" class="mini" disabled={readOnly} title={roTitle ?? t('ctr.restart')} aria-label={t('ctr.restart')} onclick={() => containerAction(c, 'restart')}>↻</button>
			{:else}
				<button type="button" class="mini start" disabled={readOnly} title={roTitle ?? t('ctr.start')} aria-label={t('ctr.start')} onclick={() => containerAction(c, c.state === 'paused' ? 'unpause' : 'start')}>▶</button>
			{/if}
			<button type="button" class="mini" title={t('ctr.logs')} aria-label={t('ctr.logs')} onclick={() => (selectedId = c.id)}>≡</button>
		</span>
	</div>
{/snippet}

{#if rowMenu}
	<DbMenu x={rowMenu.x} y={rowMenu.y} items={rowMenuItems(rowMenu.c)} onclose={() => (rowMenu = null)} />
{/if}

<HostEditor
	open={editorOpen}
	host={editing}
	onclose={() => (editorOpen = false)}
	onsaved={(h) => {
		hosts = [h, ...hosts.filter((x) => x.id !== h.id)];
		void connect(h.id);
	}}
	ondeleted={(id) => {
		hosts = hosts.filter((h) => h.id !== id);
		if (hostId === id) {
			hostId = '';
			opened = null;
			if (hosts[0]) void connect(hosts[0].id);
		}
	}}
/>

{#if confirm && host}
	<ConfirmAction
		open={!!confirm}
		title={confirm.title}
		message={confirm.message}
		items={confirm.items}
		where={host.name}
		environment={host.environment}
		note={confirm.note}
		confirmLabel={confirm.confirmLabel}
		typeToConfirm={confirm.typeToConfirm}
		onconfirm={() => void confirm?.run()}
		onclose={() => (confirm = null)}
	/>
{/if}

{#if output}
	<div class="output" role="status">
		<div class="output-head">
			<strong>{output.title}</strong>
			<button type="button" class="icon" aria-label={t('common.close')} onclick={() => (output = null)}>✕</button>
		</div>
		<pre>{output.text}</pre>
	</div>
{/if}

<style>
	.page {
		position: relative;
		display: flex;
		flex-direction: column;
		height: 100%;
		min-height: 0;
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		border-top: 3px solid transparent;
	}

	/* Production is unmistakable: a red line across the whole workspace. */
	.page.production {
		border-top-color: var(--color-danger);
	}

	.bar {
		display: flex;
		align-items: center;
		gap: 8px;
		flex-wrap: wrap;
		padding: 8px 12px;
		border-bottom: 1px solid var(--color-border);
		background: var(--color-bg-elevated);
	}

	.host-pick {
		display: flex;
		align-items: center;
		gap: 4px;
	}

	.host-pick select {
		min-height: 34px;
		max-width: 260px;
		padding: 0 10px;
		border-radius: var(--radius-btn);
		border: 1px solid var(--color-border);
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		font: inherit;
		font-size: var(--text-sm);
		font-weight: 600;
	}

	.chip {
		padding: 2px 8px;
		border-radius: 999px;
		background: var(--color-surface-hover);
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
		white-space: nowrap;
	}

	.grow {
		flex: 1;
	}

	.search {
		width: 200px;
		max-width: 40vw;
		min-height: 32px;
		padding: 0 10px;
		border-radius: var(--radius-btn);
		border: 1px solid var(--color-border);
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		font: inherit;
		font-size: var(--text-sm);
	}

	.search:focus,
	.host-pick select:focus {
		outline: none;
		border-color: var(--color-accent);
	}

	.problems {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
		white-space: nowrap;
		cursor: pointer;
	}

	.problems.has {
		color: var(--color-danger);
		font-weight: 500;
	}

	.icon {
		min-width: 34px;
		min-height: 34px;
		border: 1px solid transparent;
		border-radius: var(--radius-btn);
		background: none;
		color: var(--color-text-secondary);
		font-size: 1rem;
		cursor: pointer;
	}

	.icon:hover {
		color: var(--color-text-primary);
		background: var(--color-surface-hover);
	}

	.muted {
		color: var(--color-text-tertiary);
		font-size: var(--text-xs);
	}

	.center {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 24px 16px;
		overflow: auto;
	}

	.welcome {
		max-width: 460px;
		text-align: center;
		font-size: var(--text-sm);
		color: var(--color-text-secondary);
		line-height: 1.5;
	}

	.welcome .big {
		font-size: 2.6rem;
	}

	.welcome h2 {
		margin: 8px 0;
		font-size: 1.15rem;
		color: var(--color-text-primary);
	}

	.welcome ul {
		text-align: left;
		margin: 12px auto 18px;
		padding-left: 20px;
		list-style: disc;
	}

	.cta {
		min-height: 44px;
		padding: 0 22px;
		border: none;
		border-radius: var(--radius-btn);
		background: var(--color-accent);
		color: #fff;
		font: inherit;
		font-weight: 600;
		cursor: pointer;
	}

	.sections {
		display: flex;
		gap: 2px;
		padding: 0 10px;
		border-bottom: 1px solid var(--color-border);
		overflow-x: auto;
		scrollbar-width: none;
	}

	.sections button {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		min-height: 40px;
		padding: 0 12px;
		border: none;
		border-bottom: 2px solid transparent;
		background: none;
		color: var(--color-text-secondary);
		font: inherit;
		font-size: var(--text-sm);
		white-space: nowrap;
		cursor: pointer;
	}

	.sections button.on {
		color: var(--color-text-primary);
		border-bottom-color: var(--color-accent);
	}

	.count {
		padding: 0 6px;
		border-radius: 999px;
		background: var(--color-surface-hover);
		font-size: 0.6875rem;
	}

	.list-error {
		padding: 6px 12px;
		font-size: var(--text-xs);
		color: var(--color-danger);
		border-bottom: 1px solid var(--color-border);
	}

	.main {
		flex: 1;
		min-height: 0;
		display: grid;
		grid-template-columns: 1fr;
	}

	.main.with-detail {
		grid-template-columns: minmax(320px, 1fr) minmax(360px, 46%);
	}

	.list {
		min-width: 0;
		overflow: auto;
		padding: 8px 10px 24px;
	}

	.detail-pane {
		min-width: 0;
		min-height: 0;
		border-left: 1px solid var(--color-border);
	}

	/* On a phone the container opens over the list, full screen. */
	.detail-pane.sheet {
		position: absolute;
		inset: 0;
		z-index: 20;
		border-left: none;
	}

	.empty {
		padding: 32px 16px;
		text-align: center;
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
	}

	.empty small,
	.hint {
		display: block;
		margin-top: 6px;
		color: var(--color-text-tertiary);
		font-size: var(--text-xs);
		text-align: center;
	}

	.stack {
		margin-bottom: 8px;
		border: 1px solid var(--color-border);
		border-radius: 10px;
		background: var(--color-bg-elevated);
		overflow: hidden;
	}

	.stack-head {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: 6px;
		padding: 8px 10px;
	}

	.expand {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		min-height: 32px;
		border: none;
		background: none;
		color: var(--color-text-primary);
		font: inherit;
		cursor: pointer;
	}

	.caret {
		display: inline-block;
		width: 12px;
		transition: transform 120ms ease;
		color: var(--color-text-tertiary);
	}

	.caret.open {
		transform: rotate(90deg);
	}

	.stack-name {
		font-weight: 600;
	}

	.stack-dir {
		padding: 0 12px 8px 32px;
		font-family: var(--font-mono);
		font-size: var(--text-xs);
		color: var(--color-text-tertiary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.rows {
		display: flex;
		flex-direction: column;
	}

	.stack .rows {
		border-top: 1px solid var(--color-border);
	}

	.row {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto 140px 90px auto;
		align-items: center;
		gap: 10px;
		min-height: 48px;
		padding: 4px 8px;
		border-radius: 8px;
	}

	.row:hover {
		background: var(--color-surface-hover);
	}

	.row.selected {
		background: color-mix(in srgb, var(--color-accent) 12%, transparent);
	}

	.row-main {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 2px;
		min-width: 0;
		min-height: 40px;
		justify-content: center;
		border: none;
		background: none;
		color: inherit;
		font: inherit;
		text-align: left;
		cursor: pointer;
	}

	.row-name {
		max-width: 100%;
		font-weight: 500;
		font-size: var(--text-sm);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.row-sub,
	.row-ports,
	.row-age {
		max-width: 100%;
		font-size: var(--text-xs);
		color: var(--color-text-tertiary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.row-acts {
		display: inline-flex;
		gap: 2px;
	}

	.mini {
		min-width: 32px;
		min-height: 32px;
		border: 1px solid transparent;
		border-radius: var(--radius-btn);
		background: none;
		color: var(--color-text-secondary);
		cursor: pointer;
	}

	.mini:hover:not(:disabled) {
		border-color: var(--color-border);
		color: var(--color-text-primary);
	}

	.mini.start {
		color: var(--color-success);
	}

	.mini:disabled,
	.act:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}

	.act {
		min-height: 30px;
		padding: 0 10px;
		border-radius: var(--radius-btn);
		border: 1px solid var(--color-border);
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		font: inherit;
		font-size: var(--text-xs);
		font-weight: 500;
		white-space: nowrap;
		cursor: pointer;
	}

	.act.primary {
		background: var(--color-accent);
		border-color: var(--color-accent);
		color: #fff;
	}

	.act.quiet {
		border-color: transparent;
		background: none;
		color: var(--color-text-secondary);
	}

	.act.danger {
		color: var(--color-danger);
	}

	.table {
		width: 100%;
		border-collapse: collapse;
		font-size: var(--text-sm);
	}

	.table th {
		position: sticky;
		top: 0;
		padding: 8px;
		text-align: left;
		font-size: var(--text-xs);
		font-weight: 500;
		color: var(--color-text-secondary);
		background: var(--color-bg-primary);
		border-bottom: 1px solid var(--color-border);
	}

	.table td {
		padding: 8px;
		border-bottom: 1px solid color-mix(in srgb, var(--color-border) 60%, transparent);
		max-width: 360px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.right {
		text-align: right;
	}

	.mono {
		font-family: var(--font-mono);
		font-size: var(--text-xs);
	}

	.more {
		color: var(--color-text-tertiary);
	}

	.output {
		position: absolute;
		right: 16px;
		bottom: 16px;
		z-index: 30;
		width: min(560px, calc(100% - 32px));
		max-height: 40%;
		display: flex;
		flex-direction: column;
		border-radius: 10px;
		border: 1px solid var(--color-border);
		background: var(--color-bg-elevated);
		box-shadow: var(--shadow-elevated);
	}

	.output-head {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 4px 4px 4px 12px;
		font-size: var(--text-sm);
		border-bottom: 1px solid var(--color-border);
	}

	.output pre {
		margin: 0;
		padding: 10px 12px;
		overflow: auto;
		font-family: var(--font-mono);
		font-size: var(--text-xs);
		white-space: pre-wrap;
		user-select: text;
	}

	/* With a container open beside the list, its name gets the room. */
	.with-detail .row {
		grid-template-columns: minmax(0, 1fr) auto auto;
	}

	.with-detail .row-ports,
	.with-detail .row-age {
		display: none;
	}

	/* Phones: a two-line card per container, no columns to squeeze. */
	@media (max-width: 760px) {
		.row {
			grid-template-columns: minmax(0, 1fr) auto;
			grid-template-areas: 'main acts' 'pill pill';
			row-gap: 2px;
			padding: 8px;
		}

		.row-main {
			grid-area: main;
		}

		.row-acts {
			grid-area: acts;
		}

		.row :global(.pill) {
			grid-area: pill;
			justify-self: start;
		}

		.row-ports,
		.row-age {
			display: none;
		}

		.mini {
			min-width: 44px;
			min-height: 44px;
		}

		.search {
			width: 100%;
			max-width: none;
			order: 10;
		}

		.table th:nth-child(3),
		.table td:nth-child(3) {
			display: none;
		}
	}
</style>
