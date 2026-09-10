<!--
	Write a recipe.

	The editor holds the whole file, header included, rather than splitting
	metadata into form fields above a code box. The file is the artefact people
	share, review and run outside Reach, so editing anything else would be
	editing a projection of it.

	Parsing runs as you type and reports the same errors the parser will give
	on save, so a broken header is visible immediately instead of at the moment
	you try to keep your work.
-->
<script lang="ts">
	import CodeEditor from '$lib/components/editor/CodeEditor.svelte';
	import Modal from '$lib/components/shared/Modal.svelte';
	import Button from '$lib/components/shared/Button.svelte';
	import DangerBadge from './DangerBadge.svelte';
	import { t } from '$lib/state/i18n.svelte';
	import { addToast } from '$lib/state/toasts.svelte';
	import { recipePreview, type Recipe } from '$lib/ipc/recipes';
	import { saveRecipe } from '$lib/state/recipes.svelte';

	interface Props {
		/** The file to edit. A new recipe arrives here as a filled-in template. */
		source: string;
		onclose: () => void;
	}

	let { source, onclose }: Props = $props();

	/**
	 * Seeded at construction, not in an effect.
	 *
	 * CodeEditor builds its CodeMirror document once, when it mounts, and never
	 * syncs it again — that is the right contract for an editor, since a live
	 * binding would fight the cursor on every keystroke. But it means an effect
	 * that fills `text` after mount arrives too late: the editor has already
	 * taken the empty string, and you get a blank editor under a status bar
	 * cheerfully reporting the recipe it parsed.
	 *
	 * So the parent mounts this component fresh for each edit, and the value is
	 * correct before any child renders.
	 *
	 * Svelte warns that this captures only the initial value. That is the
	 * intent: after mount this is the user's buffer, and following the prop
	 * would overwrite their typing.
	 */
	// svelte-ignore state_referenced_locally
	let text = $state(source);
	let parsed = $state<Recipe | null>(null);
	let parseError = $state<string | null>(null);
	let saving = $state(false);

	// Debounced: parsing every keystroke over IPC makes typing feel sticky.
	let timer: ReturnType<typeof setTimeout> | undefined;
	$effect(() => {
		const current = text;
		if (!current) return;
		clearTimeout(timer);
		timer = setTimeout(async () => {
			try {
				parsed = await recipePreview(current);
				parseError = null;
			} catch (e) {
				parsed = null;
				parseError = String(e);
			}
		}, 250);
		return () => clearTimeout(timer);
	});

	async function save(): Promise<void> {
		saving = true;
		try {
			const saved = await saveRecipe(text);
			addToast(t('recipes.saved', { name: saved.name }), 'success');
			onclose();
		} catch (e) {
			parseError = String(e);
		} finally {
			saving = false;
		}
	}
</script>

<Modal open {onclose} title={t('recipes.edit_title')} maxWidth="860px">
	<div class="editor-wrap">
		<div class="code">
			<CodeEditor
				content={text}
				language="shell"
				onchange={(v) => (text = v)}
				onsave={save}
			/>
		</div>

		<div class="status" class:bad={!!parseError}>
			{#if parseError}
				<span class="msg">{parseError}</span>
			{:else if parsed}
				<span class="msg">
					<code>{parsed.id}</code>
					{parsed.name}
					{#if parsed.version}<span class="dim">v{parsed.version}</span>{/if}
				</span>
				<span class="meta">
					{#if parsed.params.length > 0}
						<span class="dim">
							{parsed.params.length === 1
								? t('recipes.one_param')
								: t('recipes.n_params', { count: parsed.params.length })}
						</span>
					{/if}
					<DangerBadge danger={parsed.analysis.danger} compact />
				</span>
			{:else}
				<span class="msg dim">{t('recipes.parsing')}</span>
			{/if}
		</div>
	</div>

	{#snippet actions()}
		<Button variant="secondary" onclick={onclose}>{t('common.cancel')}</Button>
		<Button variant="primary" disabled={!!parseError || saving} onclick={save}>
			{t('common.save')}
		</Button>
	{/snippet}
</Modal>

<style>
	.editor-wrap {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}

	.code {
		height: 52vh;
		min-height: 280px;
		overflow: hidden;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-sm);
	}

	.status {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2);
		min-height: 24px;
		padding: 0 var(--space-1);
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
	}

	.status.bad .msg {
		color: var(--color-danger);
	}

	.msg {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.msg code {
		font-family: var(--font-mono);
		color: var(--color-text-primary);
	}

	.meta {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		flex-shrink: 0;
	}

	.dim {
		color: var(--color-text-tertiary);
	}
</style>
