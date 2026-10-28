<script lang="ts">
	/**
	 * Add or edit a container host: which server (a saved SSH session, or
	 * this computer), which engine, and what it is for. Production turns
	 * read-only on, which the backend enforces; the user can turn it off.
	 */
	import { onMount } from 'svelte';
	import Modal from '#lib/components/shared/Modal.svelte';
	import Button from '#lib/components/shared/Button.svelte';
	import Toggle from '#lib/components/shared/Toggle.svelte';
	import { ctrHostSave, ctrHostDelete, type ContainerHost, type Engine, type Environment, type Route } from '#lib/ipc/containers.js';
	import { sessionList, sessionKind, type SessionConfig } from '#lib/ipc/sessions.js';
	import { isMobile } from '#lib/platform.js';
	import { t } from '#lib/state/i18n.svelte.js';

	interface Props {
		open: boolean;
		/** The host to edit, or null for a new one. */
		host: ContainerHost | null;
		onclose: () => void;
		onsaved: (host: ContainerHost) => void;
		ondeleted: (id: string) => void;
	}

	let { open, host, onclose, onsaved, ondeleted }: Props = $props();

	let sessions = $state<SessionConfig[]>([]);
	let name = $state('');
	let via = $state('direct');
	let engine = $state<Engine>('docker');
	let environment = $state<Environment>('none');
	let readOnly = $state(false);
	let error = $state('');
	let saving = $state(false);
	let confirmDelete = $state(false);

	onMount(() => {
		sessionList()
			.then((all) => (sessions = all.filter((s) => sessionKind(s) === 'ssh')))
			.catch(() => {});
	});

	// Fill the form each time it opens.
	$effect(() => {
		if (!open) return;
		error = '';
		confirmDelete = false;
		name = host?.name ?? '';
		via = host?.route.kind === 'session' ? `s:${host.route.sessionId}` : isMobile() ? '' : 'direct';
		engine = host?.engine ?? 'docker';
		environment = host?.environment ?? 'none';
		readOnly = host?.readOnly ?? false;
	});

	function setEnvironment(e: Environment): void {
		// Production starts read-only; the user may still turn that off.
		if (e === 'production' && environment !== 'production') readOnly = true;
		environment = e;
	}

	function pickSession(v: string): void {
		via = v;
		if (!name.trim() && v.startsWith('s:')) name = sessions.find((s) => s.id === v.slice(2))?.name ?? '';
	}

	async function save(): Promise<void> {
		error = '';
		if (!via) {
			error = t('ctr.choose_server');
			return;
		}
		const route: Route = via === 'direct' ? { kind: 'direct' } : { kind: 'session', sessionId: via.slice(2) };
		const fallback = via === 'direct' ? t('ctr.this_computer') : (sessions.find((s) => s.id === via.slice(2))?.name ?? '');
		saving = true;
		try {
			const saved = await ctrHostSave({
				id: host?.id ?? '',
				name: name.trim() || fallback,
				route,
				engine,
				environment,
				readOnly,
				lastUsedAt: host?.lastUsedAt ?? 0
			});
			onsaved(saved);
			onclose();
		} catch (e) {
			error = String(e);
		} finally {
			saving = false;
		}
	}

	async function remove(): Promise<void> {
		if (!host) return;
		try {
			await ctrHostDelete(host.id);
			ondeleted(host.id);
			onclose();
		} catch (e) {
			error = String(e);
		}
	}
</script>

<Modal {open} {onclose} title={host ? t('ctr.edit_host') : t('ctr.add_host')} maxWidth="500px">
	<form class="form" onsubmit={(e) => (e.preventDefault(), save())}>
		<label class="field">
			<span>{t('ctr.server')}</span>
			<select value={via} onchange={(e) => pickSession(e.currentTarget.value)}>
				{#if isMobile()}
					<option value="" disabled>{t('ctr.choose_server')}</option>
				{:else}
					<option value="direct">{t('ctr.this_computer')}</option>
				{/if}
				{#if sessions.length > 0}
					<optgroup label={t('ctr.saved_sessions')}>
						{#each sessions as s (s.id)}
							<option value={`s:${s.id}`}>{s.name} ({s.username}@{s.host})</option>
						{/each}
					</optgroup>
				{/if}
			</select>
			<small>{via === 'direct' ? t('ctr.this_computer_hint') : t('ctr.session_hint')}</small>
		</label>

		<fieldset class="field">
			<legend>{t('ctr.engine')}</legend>
			<div class="engines">
				<label class="engine" class:on={engine === 'docker'}>
					<input type="radio" name="engine" value="docker" bind:group={engine} />
					<strong>Docker</strong>
					<small>{t('ctr.docker_hint')}</small>
				</label>
				<label class="engine" class:on={engine === 'podman'}>
					<input type="radio" name="engine" value="podman" bind:group={engine} />
					<strong>Podman</strong>
					<small>{t('ctr.podman_hint')}</small>
				</label>
			</div>
		</fieldset>

		<label class="field">
			<span>{t('ctr.name')}</span>
			<input type="text" bind:value={name} placeholder={t('ctr.name_placeholder')} spellcheck="false" />
		</label>

		<label class="field">
			<span>{t('env.label')}</span>
			<select value={environment} onchange={(e) => setEnvironment(e.currentTarget.value as Environment)}>
				<option value="none">{t('env.none')}</option>
				<option value="development">{t('env.development')}</option>
				<option value="staging">{t('env.staging')}</option>
				<option value="production">{t('env.production')}</option>
			</select>
			<small>{t('env.hint')}</small>
		</label>

		<div class="row">
			<div>
				<div class="row-label">{t('env.read_only')}</div>
				<small>{t('env.read_only_hint')}</small>
			</div>
			<Toggle hideLabel label={t('env.read_only')} checked={readOnly} onchange={(v) => (readOnly = v)} />
		</div>

		{#if error}
			<p class="error" role="alert">{error}</p>
		{/if}
	</form>

	{#snippet actions()}
		{#if host}
			{#if confirmDelete}
				<span class="sure">{t('ctr.remove_host_sure')}</span>
				<Button variant="danger" onclick={remove}>{t('ctr.remove_host')}</Button>
			{:else}
				<Button variant="ghost" onclick={() => (confirmDelete = true)}>{t('ctr.remove_host')}</Button>
			{/if}
			<span class="gap"></span>
		{/if}
		<Button variant="ghost" onclick={onclose}>{t('common.cancel')}</Button>
		<Button disabled={saving} onclick={save}>{host ? t('common.save') : t('ctr.add_host')}</Button>
	{/snippet}
</Modal>

<style>
	.form {
		display: flex;
		flex-direction: column;
		gap: 16px;
		font-size: var(--text-sm);
		color: var(--color-text-primary);
	}

	.field {
		display: flex;
		flex-direction: column;
		gap: 6px;
		margin: 0;
		padding: 0;
		border: none;
		min-width: 0;
	}

	.field > span,
	legend,
	.row-label {
		font-weight: 500;
		margin-bottom: 2px;
	}

	small {
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
		line-height: 1.45;
	}

	select,
	input[type='text'] {
		min-height: 38px;
		padding: 0 10px;
		border-radius: var(--radius-btn);
		border: 1px solid var(--color-border);
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		font: inherit;
	}

	select:focus,
	input[type='text']:focus {
		outline: none;
		border-color: var(--color-accent);
	}

	.engines {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 8px;
	}

	.engine {
		display: flex;
		flex-direction: column;
		gap: 4px;
		padding: 10px 12px;
		border-radius: 8px;
		border: 1px solid var(--color-border);
		cursor: pointer;
	}

	.engine.on {
		border-color: var(--color-accent);
		background: color-mix(in srgb, var(--color-accent) 8%, transparent);
	}

	.engine input {
		position: absolute;
		opacity: 0;
		pointer-events: none;
	}

	.engine:focus-within {
		outline: 2px solid var(--color-accent);
		outline-offset: 1px;
	}

	.row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
	}

	.row > div {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.error {
		margin: 0;
		font-size: var(--text-xs);
		color: var(--color-danger);
		white-space: pre-wrap;
	}

	.sure {
		font-size: var(--text-xs);
		color: var(--color-danger);
	}

	.gap {
		flex: 1;
	}

	@media (max-width: 480px) {
		.engines {
			grid-template-columns: 1fr;
		}
	}
</style>
