<script lang="ts">
	/**
	 * An object's YAML: read-only until Edit, then saved as a replace that
	 * carries the resourceVersion it was read with. If someone changed the
	 * object meanwhile the server refuses (409), and the user is told so and
	 * offered the fresh version, instead of a raw error.
	 *
	 * A Secret's YAML holds its values. They are fetched only after a warning,
	 * shown hidden, and revealed on request; Secrets are not edited here.
	 */
	import { onDestroy, untrack } from 'svelte';
	import { ask } from '#lib/state/ask.svelte.js';
	import { writeText } from '@tauri-apps/plugin-clipboard-manager';
	import ErrorHelp from '#lib/components/devops/ErrorHelp.svelte';
	import ActionButton from './ActionButton.svelte';
	import { k8sGetYaml, k8sReplaceYaml, type Kind } from '#lib/ipc/k8s.js';
	import { busy } from '#lib/state/k8s.svelte.js';
	import { addToast } from '#lib/state/toasts.svelte.js';
	import { t } from '#lib/state/i18n.svelte.js';
	import { isConflict, maskSecretYaml, decodeSecretData, type Ctx } from './model';

	interface Props {
		ctx: Ctx;
		kind: Kind;
		namespace: string | null;
		name: string;
		/** While true the page stops refreshing, so nothing moves under the cursor. */
		onediting: (editing: boolean) => void;
		onsaved?: () => void;
	}

	let { ctx, kind, namespace, name, onediting, onsaved }: Props = $props();

	let secret = $derived(kind === 'Secret');
	let yaml = $state('');
	let loading = $state(false);
	let error = $state('');
	let editing = $state(false);
	let draft = $state('');
	let saving = $state(false);
	let saveError = $state('');
	let conflict = $state(false);
	let acknowledged = $state(false);
	let revealed = $state(false);
	let seq = 0;

	async function load(): Promise<void> {
		const mine = ++seq;
		loading = true;
		error = '';
		try {
			const text = await k8sGetYaml(ctx.key, kind, namespace, name);
			if (mine === seq) yaml = text;
		} catch (e) {
			if (mine === seq) error = String(e);
		} finally {
			if (mine === seq) loading = false;
		}
	}

	// A different object: start over, read-only, values hidden again.
	$effect(() => {
		void ctx.key;
		void kind;
		void namespace;
		void name;
		untrack(() => {
			setEditing(false);
			yaml = '';
			revealed = false;
			acknowledged = false;
			conflict = false;
			saveError = '';
			if (!secret) load();
		});
	});

	onDestroy(() => {
		if (editing) onediting(false);
	});

	function setEditing(on: boolean): void {
		if (editing === on) return;
		editing = on;
		onediting(on);
	}

	function startEdit(): void {
		draft = yaml;
		saveError = '';
		conflict = false;
		setEditing(true);
	}

	async function cancel(): Promise<void> {
		if (draft !== yaml && !await ask(t('k8s.yaml_discard'))) return;
		setEditing(false);
	}

	async function save(): Promise<void> {
		saving = true;
		saveError = '';
		conflict = false;
		try {
			await busy(() => k8sReplaceYaml(ctx.key, kind, namespace, name, draft));
			addToast(t('k8s.yaml_saved', { name }), 'success');
			setEditing(false);
			await load();
			onsaved?.();
		} catch (e) {
			const msg = String(e);
			if (isConflict(msg)) conflict = true;
			else saveError = msg;
		} finally {
			saving = false;
		}
	}

	/** After a conflict: the latest version in the editor, the user's edit on the clipboard first. */
	async function reloadIntoEditor(): Promise<void> {
		await load();
		if (!error) {
			draft = yaml;
			conflict = false;
		}
	}

	async function copy(text: string): Promise<void> {
		try {
			await writeText(text);
			addToast(t('k8s.copied'), 'success');
		} catch (e) {
			addToast(String(e), 'error');
		}
	}

	function onTab(e: KeyboardEvent): void {
		if (e.key !== 'Tab' || e.shiftKey) return;
		// YAML is indented with spaces; Tab leaving the box would lose the place.
		e.preventDefault();
		const el = e.currentTarget as HTMLTextAreaElement;
		const at = el.selectionStart;
		draft = draft.slice(0, at) + '  ' + draft.slice(el.selectionEnd);
		queueMicrotask(() => el.setSelectionRange(at + 2, at + 2));
	}

	let shown = $derived(secret && !revealed ? maskSecretYaml(yaml) : yaml);
	let decoded = $derived(secret && revealed ? decodeSecretData(yaml) : []);
</script>

<div class="pane">
	{#if secret && !acknowledged}
		<div class="warn" role="note">
			<strong>{t('k8s.secret_warn_title')}</strong>
			<p>{t('k8s.secret_warn_body')}</p>
			<div>
				<ActionButton
					onclick={() => {
						acknowledged = true;
						load();
					}}>{t('k8s.secret_show_hidden')}</ActionButton
				>
			</div>
		</div>
	{:else}
		<div class="bar">
			{#if editing}
				<ActionButton variant="primary" disabled={saving} onclick={save}>{saving ? t('k8s.saving') : t('common.save')}</ActionButton>
				<ActionButton disabled={saving} onclick={cancel}>{t('common.cancel')}</ActionButton>
				<span class="hint">{t('k8s.yaml_edit_hint')}</span>
			{:else}
				<ActionButton disabled={loading} onclick={load}>{t('k8s.reload')}</ActionButton>
				<ActionButton disabled={!yaml} onclick={() => copy(shown)}>{t('common.copy')}</ActionButton>
				{#if secret}
					<ActionButton onclick={() => (revealed = !revealed)}>{revealed ? t('k8s.secret_hide') : t('k8s.secret_reveal')}</ActionButton>
				{:else}
					<ActionButton locked={ctx.readOnly} disabled={!yaml} onclick={startEdit}>{t('common.edit')}</ActionButton>
				{/if}
			{/if}
		</div>

		{#if conflict}
			<div class="conflict" role="alert">
				<strong>{t('k8s.conflict_title')}</strong>
				<p>{t('k8s.conflict_body')}</p>
				<div class="row">
					<ActionButton onclick={() => copy(draft)}>{t('k8s.copy_my_edit')}</ActionButton>
					<ActionButton variant="primary" onclick={reloadIntoEditor}>{t('k8s.reload_latest')}</ActionButton>
				</div>
			</div>
		{:else if saveError}
			<div class="err"><ErrorHelp error={saveError} /></div>
		{/if}

		{#if error}
			<div class="err"><ErrorHelp {error} onretry={load} /></div>
		{:else if editing}
			<textarea
				class="code"
				bind:value={draft}
				spellcheck="false"
				autocomplete="off"
				autocapitalize="off"
				aria-label={t('k8s.yaml_editor', { name })}
				onkeydown={onTab}
			></textarea>
		{:else if loading && !yaml}
			<div class="muted">{t('common.loading')}</div>
		{:else}
			{#if decoded.length}
				<div class="decoded">
					<div class="decoded-title">{t('k8s.secret_decoded')}</div>
					{#each decoded as d (d.key)}
						<div class="kv">
							<span class="k">{d.key}</span>
							<code class="v">{d.value}</code>
							<button type="button" class="mini" onclick={() => copy(d.value)} aria-label={t('k8s.copy_value', { key: d.key })}>{t('common.copy')}</button>
						</div>
					{/each}
				</div>
			{/if}
			<!-- Focusable so the keyboard can scroll a long manifest. -->
			<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
			<pre class="code" tabindex="0" aria-label={t('k8s.yaml_of', { name })}>{shown}</pre>
		{/if}
	{/if}
</div>

<style>
	.pane {
		display: flex;
		flex-direction: column;
		gap: 8px;
		height: 100%;
		min-height: 0;
		padding: 10px;
	}

	.bar,
	.row {
		display: flex;
		align-items: center;
		gap: 6px;
		flex-wrap: wrap;
	}

	.hint {
		font-size: var(--text-xs);
		color: var(--color-text-tertiary);
	}

	.code {
		flex: 1;
		min-height: 200px;
		margin: 0;
		padding: 10px 12px;
		overflow: auto;
		border-radius: var(--radius-sm);
		border: 1px solid var(--color-border);
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		font-family: var(--font-mono);
		font-size: 12px;
		line-height: 1.55;
		white-space: pre;
		tab-size: 2;
		resize: none;
		user-select: text;
	}

	textarea.code:focus,
	pre.code:focus-visible {
		outline: none;
		border-color: var(--color-accent);
	}

	.warn,
	.conflict {
		display: flex;
		flex-direction: column;
		gap: 8px;
		padding: 12px 14px;
		border-radius: var(--radius-sm);
		border: 1px solid color-mix(in srgb, var(--color-warning) 45%, var(--color-border));
		background: color-mix(in srgb, var(--color-warning) 8%, var(--color-bg-elevated));
		font-size: var(--text-sm);
		color: var(--color-text-primary);
	}

	.warn p,
	.conflict p {
		margin: 0;
		line-height: 1.45;
		color: var(--color-text-secondary);
	}

	.muted {
		padding: 12px;
		font-size: var(--text-sm);
		color: var(--color-text-tertiary);
	}

	.decoded {
		display: flex;
		flex-direction: column;
		gap: 4px;
		padding: 8px 10px;
		border-radius: var(--radius-sm);
		border: 1px solid var(--color-border);
		background: var(--color-bg-elevated);
		max-height: 40%;
		overflow: auto;
		flex-shrink: 0;
	}

	.decoded-title {
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
	}

	.kv {
		display: grid;
		grid-template-columns: minmax(80px, 30%) 1fr auto;
		gap: 8px;
		align-items: center;
		font-size: var(--text-xs);
	}

	.k {
		font-family: var(--font-mono);
		color: var(--color-text-secondary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.v {
		font-family: var(--font-mono);
		color: var(--color-text-primary);
		white-space: pre-wrap;
		overflow-wrap: anywhere;
		user-select: text;
	}

	.mini {
		min-height: 26px;
		padding: 0 8px;
		border-radius: var(--radius-btn);
		border: 1px solid var(--color-border);
		background: none;
		color: var(--color-text-primary);
		font: inherit;
		cursor: pointer;
	}

	.mini:focus-visible {
		outline: 2px solid var(--color-accent);
	}
</style>
