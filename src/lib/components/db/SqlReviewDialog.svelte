<script lang="ts">
	/**
	 * The one place changes are looked at before they happen: grid edits,
	 * designer changes and risky statements from the editor all come here,
	 * as the exact SQL that will run. On a production connection a
	 * destructive change also asks for the connection's name, the way
	 * OpenTofu's Destroy asks for the project's.
	 */
	import Modal from '$lib/components/shared/Modal.svelte';
	import Button from '$lib/components/shared/Button.svelte';
	import type { Statement } from '$lib/ipc/db';
	import { t } from '$lib/state/i18n.svelte';

	interface Props {
		open: boolean;
		title: string;
		/** Plain SQL, or analysed statements with their risk. */
		statements: (string | Statement)[];
		intro?: string;
		confirmLabel: string;
		danger?: boolean;
		/** Type this to enable the button. */
		typeToConfirm?: string | null;
		busy?: boolean;
		error?: string | null;
		onconfirm: () => void;
		onclose: () => void;
	}

	let { open, title, statements, intro = '', confirmLabel, danger = false, typeToConfirm = null, busy = false, error = null, onconfirm, onclose }: Props = $props();

	let typed = $state('');
	$effect(() => {
		if (open) typed = '';
	});

	let ready = $derived(!busy && (!typeToConfirm || typed.trim() === typeToConfirm));

	function sqlOf(s: string | Statement): string {
		return typeof s === 'string' ? s : s.sql;
	}

	function reason(s: string | Statement): string | null {
		return typeof s !== 'string' && s.risk.kind === 'danger' ? s.risk.reason : null;
	}

	async function copyAll(): Promise<void> {
		await navigator.clipboard.writeText(statements.map((s) => sqlOf(s).trim().replace(/;$/, '') + ';').join('\n'));
	}
</script>

<Modal {open} {onclose} {title} maxWidth="720px">
	{#snippet children()}
		<div class="body">
			{#if intro}<p class="intro">{intro}</p>{/if}
			<div class="list">
				{#each statements as s, i (i)}
					<div class="stmt" class:risky={!!reason(s)}>
						{#if reason(s)}<div class="reason">⚠ {reason(s)}</div>{/if}
						<pre>{sqlOf(s)}</pre>
					</div>
				{/each}
			</div>
			{#if typeToConfirm}
				<label class="typed">
					<span>{t('db.type_to_confirm', { name: typeToConfirm })}</span>
					<input bind:value={typed} autocomplete="off" spellcheck="false" />
				</label>
			{/if}
			{#if error}<div class="error" role="alert">{error}</div>{/if}
		</div>
	{/snippet}
	{#snippet actions()}
		<Button variant="ghost" onclick={copyAll}>{t('db.copy_sql')}</Button>
		<span class="spacer"></span>
		<Button variant="secondary" onclick={onclose}>{t('db.cancel')}</Button>
		<Button variant={danger ? 'danger' : 'primary'} disabled={!ready} onclick={onconfirm}>
			{busy ? t('db.applying') : confirmLabel}
		</Button>
	{/snippet}
</Modal>

<style>
	.body {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.intro {
		margin: 0;
		font-size: var(--text-sm);
		color: var(--color-text-secondary);
	}

	.list {
		display: flex;
		flex-direction: column;
		gap: 6px;
		max-height: 50vh;
		overflow: auto;
	}

	.stmt {
		border-radius: var(--radius-sm);
		border: 1px solid var(--color-border);
		background: var(--color-surface-sunken, var(--color-bg-primary));
		overflow: hidden;
	}

	.stmt.risky {
		border-color: color-mix(in srgb, var(--color-danger) 60%, transparent);
	}

	.reason {
		padding: 5px 10px;
		font-size: var(--text-xs);
		color: var(--color-danger);
		background: color-mix(in srgb, var(--color-danger) 10%, transparent);
	}

	pre {
		margin: 0;
		padding: 8px 10px;
		font-family: var(--font-mono);
		font-size: var(--text-xs);
		white-space: pre-wrap;
		word-break: break-word;
		color: var(--color-text-primary);
	}

	.typed {
		display: flex;
		flex-direction: column;
		gap: 5px;
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
	}

	.typed input {
		padding: 7px 9px;
		border-radius: var(--radius-sm);
		border: 1px solid var(--color-border);
		background: var(--color-surface-sunken, var(--color-bg-primary));
		color: var(--color-text-primary);
		font-family: var(--font-mono);
	}

	.error {
		padding: 8px 10px;
		border-radius: var(--radius-sm);
		font-size: var(--text-xs);
		color: var(--color-danger);
		background: color-mix(in srgb, var(--color-danger) 12%, transparent);
		white-space: pre-wrap;
	}

	.spacer {
		flex: 1;
	}
</style>
