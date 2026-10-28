<script lang="ts">
	/**
	 * Add or edit a cluster. The kubeconfig is pasted or read from a file
	 * here, sent once with Save, and kept in the encrypted vault; it never
	 * comes back to the page, so editing leaves the box empty and keeps the
	 * stored one unless a new one is given.
	 *
	 * The file is read in the page (FileReader) rather than by path: the
	 * kubeconfig goes to the backend the same way whether pasted or chosen.
	 */
	import { onMount, untrack } from 'svelte';
	import Modal from '#lib/components/shared/Modal.svelte';
	import Button from '#lib/components/shared/Button.svelte';
	import Toggle from '#lib/components/shared/Toggle.svelte';
	import ErrorHelp from '#lib/components/devops/ErrorHelp.svelte';
	import ConfirmAction from '#lib/components/devops/ConfirmAction.svelte';
	import { k8sClusterDelete, k8sClusterSave, k8sContexts, type ClusterView, type ContextInfo } from '#lib/ipc/k8s.js';
	import type { Environment, Route } from '#lib/ipc/containers.js';
	import { sessionList, sessionKind, type SessionConfig } from '#lib/ipc/sessions.js';
	import { addToast } from '#lib/state/toasts.svelte.js';
	import { t } from '#lib/state/i18n.svelte.js';

	interface Props {
		open: boolean;
		initial: ClusterView | null;
		onclose: () => void;
		onsaved: (c: ClusterView) => void;
		ondeleted: (id: string) => void;
	}

	let { open, initial, onclose, onsaved, ondeleted }: Props = $props();

	let name = $state('');
	let yaml = $state('');
	let contexts = $state<ContextInfo[]>([]);
	let context = $state('');
	let parseError = $state('');
	let route = $state<Route>({ kind: 'direct' });
	let environment = $state<Environment>('none');
	let readOnly = $state(false);
	let namespace = $state('');
	let sessions = $state<SessionConfig[]>([]);
	let saving = $state(false);
	let error = $state('');
	let confirmDelete = $state(false);
	let fileInput: HTMLInputElement | undefined = $state();

	let isEdit = $derived(!!initial?.id);

	$effect(() => {
		if (!open) return;
		untrack(() => {
			name = initial?.name ?? '';
			yaml = '';
			contexts = [];
			context = initial?.context ?? '';
			parseError = '';
			route = initial?.route.kind === 'session' ? initial.route : { kind: 'direct' };
			environment = initial?.environment ?? 'none';
			readOnly = initial?.readOnly ?? false;
			namespace = initial?.namespace ?? '';
			error = '';
		});
	});

	onMount(async () => {
		try {
			sessions = (await sessionList()).filter((s) => sessionKind(s) === 'ssh');
		} catch {
			sessions = [];
		}
	});

	// List the contexts as soon as there is something to read, a moment
	// after typing stops so a paste in progress is not parsed half-way.
	$effect(() => {
		const text = yaml;
		if (!text.trim()) {
			untrack(() => {
				contexts = [];
				parseError = '';
			});
			return;
		}
		const timer = setTimeout(async () => {
			try {
				const found = await k8sContexts(text);
				if (text !== yaml) return;
				contexts = found.contexts;
				parseError = found.contexts.length ? '' : t('k8s.kubeconfig_no_contexts');
				if (!found.contexts.some((c) => c.name === context)) context = found.current ?? found.contexts[0]?.name ?? '';
			} catch (e) {
				if (text !== yaml) return;
				contexts = [];
				parseError = String(e);
			}
		}, 300);
		return () => clearTimeout(timer);
	});

	let chosen = $derived(contexts.find((c) => c.name === context) ?? null);
	let routeValue = $derived(route.kind === 'session' ? `s:${route.sessionId}` : 'direct');

	function setRoute(v: string): void {
		route = v.startsWith('s:') ? { kind: 'session', sessionId: v.slice(2) } : { kind: 'direct' };
	}

	function setEnvironment(v: Environment): void {
		environment = v;
		// Production starts read-only; turning that off is a deliberate second step.
		if (v === 'production') readOnly = true;
	}

	function readFile(e: Event): void {
		const input = e.currentTarget as HTMLInputElement;
		const file = input.files?.[0];
		input.value = '';
		if (!file) return;
		if (file.size > 1_000_000) {
			parseError = t('k8s.kubeconfig_too_big');
			return;
		}
		const reader = new FileReader();
		reader.onload = () => {
			yaml = String(reader.result ?? '');
			if (!name.trim()) name = file.name.replace(/\.(ya?ml|conf|config)$/i, '');
		};
		reader.onerror = () => (parseError = String(reader.error));
		reader.readAsText(file);
	}

	let canSave = $derived(!saving && !!context && (isEdit || (!!yaml.trim() && contexts.length > 0)));

	async function submit(): Promise<void> {
		if (!canSave) return;
		saving = true;
		error = '';
		try {
			const saved = await k8sClusterSave({
				id: initial?.id ?? '',
				name: name.trim() || context,
				yaml: yaml.trim() ? yaml : null,
				context,
				route,
				environment,
				readOnly,
				namespace: namespace.trim() || null
			});
			yaml = '';
			addToast(t('k8s.cluster_saved', { name: saved.name }), 'success');
			onsaved(saved);
			onclose();
		} catch (e) {
			error = String(e);
		} finally {
			saving = false;
		}
	}

	async function remove(): Promise<void> {
		if (!initial) return;
		try {
			await k8sClusterDelete(initial.id);
			addToast(t('k8s.cluster_removed', { name: initial.name }), 'success');
			ondeleted(initial.id);
			onclose();
		} catch (e) {
			error = String(e);
		}
	}
</script>

<Modal {open} {onclose} title={isEdit ? t('k8s.edit_cluster') : t('k8s.add_cluster')} maxWidth="580px">
	{#snippet children()}
		<div class="form">
			<div class="field">
				<div class="label-row">
					<span>{t('k8s.kubeconfig')}</span>
					<span class="spacer"></span>
					<button type="button" class="link" onclick={() => fileInput?.click()}>{t('k8s.choose_file')}</button>
					<input bind:this={fileInput} type="file" class="hidden" onchange={readFile} tabindex="-1" aria-hidden="true" />
				</div>
				<textarea
					bind:value={yaml}
					rows="7"
					class="mono"
					spellcheck="false"
					autocomplete="off"
					autocapitalize="off"
					aria-label={t('k8s.kubeconfig')}
					placeholder={isEdit ? t('k8s.kubeconfig_keep') : t('k8s.kubeconfig_paste')}
				></textarea>
				<small>{isEdit ? t('k8s.kubeconfig_keep_hint') : t('k8s.kubeconfig_hint')}</small>
				{#if parseError}
					<small class="bad">{parseError}</small>
				{/if}
			</div>

			<label class="field">
				<span>{t('k8s.context')}</span>
				<select bind:value={context} disabled={contexts.length === 0 && !isEdit}>
					{#if contexts.length === 0}
						{#if isEdit}
							<option value={initial?.context}>{initial?.context}</option>
						{:else}
							<option value="">{t('k8s.context_paste_first')}</option>
						{/if}
					{/if}
					{#each contexts as c (c.name)}
						<option value={c.name}>{c.name}</option>
					{/each}
				</select>
				{#if chosen}
					<small class="mono">{chosen.server} · {chosen.user}{chosen.namespace ? ` · ${chosen.namespace}` : ''}</small>
				{:else if isEdit && initial}
					<small class="mono">{initial.server} · {initial.user}</small>
				{/if}
			</label>

			<label class="field">
				<span>{t('k8s.cluster_name')}</span>
				<input bind:value={name} placeholder={context || t('k8s.cluster_name_placeholder')} autocomplete="off" />
			</label>

			<label class="field">
				<span>{t('k8s.reach_through')}</span>
				<select value={routeValue} onchange={(e) => setRoute(e.currentTarget.value)}>
					<option value="direct">{t('k8s.route_direct')}</option>
					{#if sessions.length}
						<optgroup label={t('k8s.route_session_group')}>
							{#each sessions as s (s.id)}
								<option value="s:{s.id}">{s.name} ({s.username}@{s.host})</option>
							{/each}
						</optgroup>
					{/if}
				</select>
				<small>{t('k8s.route_hint')}</small>
			</label>

			<div class="grid2">
				<label class="field grow">
					<span>{t('k8s.environment')}</span>
					<select value={environment} onchange={(e) => setEnvironment(e.currentTarget.value as Environment)}>
						<option value="none">{t('k8s.env_none')}</option>
						<option value="development">{t('env.development')}</option>
						<option value="staging">{t('env.staging')}</option>
						<option value="production">{t('env.production')}</option>
					</select>
				</label>
				<label class="field grow">
					<span>{t('k8s.default_namespace')}</span>
					<input bind:value={namespace} class="mono" placeholder={chosen?.namespace ?? 'default'} autocomplete="off" autocapitalize="off" spellcheck="false" />
				</label>
			</div>

			<div class="toggle-row">
				<div>
					<strong>{t('env.read_only')}</strong>
					<small>{environment === 'production' ? t('k8s.read_only_prod') : t('k8s.read_only_desc')}</small>
				</div>
				<Toggle hideLabel label={t('env.read_only')} checked={readOnly} onchange={(v) => (readOnly = v)} />
			</div>

			{#if error}
				<ErrorHelp {error} />
			{/if}
		</div>
	{/snippet}
	{#snippet actions()}
		{#if isEdit}
			<Button variant="ghost" onclick={() => (confirmDelete = true)}>{t('k8s.remove_cluster')}</Button>
		{/if}
		<span class="spacer"></span>
		<Button variant="ghost" onclick={onclose}>{t('common.cancel')}</Button>
		<Button variant="primary" disabled={!canSave} onclick={submit}>{saving ? t('k8s.saving') : isEdit ? t('common.save') : t('k8s.add_and_connect')}</Button>
	{/snippet}
</Modal>

{#if initial}
	<ConfirmAction
		open={confirmDelete}
		title={t('k8s.remove_cluster_title')}
		message={t('k8s.remove_cluster_message')}
		items={[initial.name]}
		where={initial.name}
		environment={initial.environment}
		note={t('k8s.remove_cluster_note')}
		confirmLabel={t('k8s.remove_cluster')}
		onconfirm={remove}
		onclose={() => (confirmDelete = false)}
	/>
{/if}

<style>
	.form {
		display: flex;
		flex-direction: column;
		gap: 14px;
	}

	.field {
		display: flex;
		flex-direction: column;
		gap: 5px;
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
		min-width: 0;
	}

	.field small {
		color: var(--color-text-tertiary);
		line-height: 1.4;
		overflow-wrap: anywhere;
	}

	.field small.bad {
		color: var(--color-danger);
	}

	.label-row {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.spacer {
		flex: 1;
	}

	.link {
		min-height: 28px;
		padding: 0 10px;
		border-radius: var(--radius-btn);
		border: 1px solid var(--color-border);
		background: var(--color-bg-elevated);
		color: var(--color-text-primary);
		font: inherit;
		cursor: pointer;
	}

	.link:focus-visible {
		outline: 2px solid var(--color-accent);
	}

	.hidden {
		display: none;
	}

	.field input,
	.field select,
	.field textarea {
		width: 100%;
		padding: 7px 9px;
		border-radius: var(--radius-sm);
		border: 1px solid var(--color-border);
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		font-size: var(--text-sm);
	}

	.field textarea {
		resize: vertical;
		min-height: 110px;
		font-size: 12px;
		line-height: 1.5;
	}

	.field input:focus,
	.field select:focus,
	.field textarea:focus {
		outline: none;
		border-color: var(--color-accent);
	}

	.mono {
		font-family: var(--font-mono);
	}

	.grid2 {
		display: flex;
		gap: 10px;
	}

	.grow {
		flex: 1;
	}

	.toggle-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 16px;
		padding: 10px 0;
		border-top: 1px solid var(--color-border);
		border-bottom: 1px solid var(--color-border);
	}

	.toggle-row div {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.toggle-row strong {
		font-size: var(--text-sm);
		font-weight: 500;
		color: var(--color-text-primary);
	}

	.toggle-row small {
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
		line-height: 1.4;
	}

	@media (max-width: 560px) {
		.grid2 {
			flex-direction: column;
		}

		.field input,
		.field select,
		.link {
			min-height: 44px;
		}
	}
</style>
