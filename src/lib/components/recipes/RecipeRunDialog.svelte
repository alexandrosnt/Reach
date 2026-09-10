<!--
	Fill in a recipe's parameters, see what it will do, then run it.

	This is the last screen before arbitrary bash reaches a production machine,
	so it is built around one rule: nothing is hidden. The target is named, the
	risk analysis is shown with the offending lines quoted, and the exact text
	that will be written to the session is available before anything is sent.

	Anything above "mutating" needs the confirmation typed, not clicked. A
	button you can hit by reflex is not a confirmation.
-->
<script lang="ts">
	import Modal from '$lib/components/shared/Modal.svelte';
	import Button from '$lib/components/shared/Button.svelte';
	import DangerBadge from './DangerBadge.svelte';
	import { t } from '$lib/state/i18n.svelte';
	import { addToast } from '$lib/state/toasts.svelte';
	import { recipePrepare, type PreparedRun, type Recipe } from '$lib/ipc/recipes';
	import { dangerRank } from '$lib/state/recipes.svelte';

	interface Props {
		recipe: Recipe | null;
		/** Human name of the session this will run against. */
		targetLabel: string;
		/** Null when no session is open — the run is blocked, not silently local. */
		canRun: boolean;
		onclose: () => void;
		onrun: (command: string) => void;
	}

	let { recipe, targetLabel, canRun, onclose, onrun }: Props = $props();

	let values = $state<Record<string, string>>({});
	let prepared = $state<PreparedRun | null>(null);
	let prepareError = $state<string | null>(null);
	let showCommand = $state(false);
	let typed = $state('');
	let busy = $state(false);

	/** Reset every time a different recipe opens, or defaults leak across runs. */
	$effect(() => {
		const current = recipe;
		if (!current) return;
		const seeded: Record<string, string> = {};
		for (const p of current.params) seeded[p.name] = p.default;
		values = seeded;
		prepared = null;
		prepareError = null;
		showCommand = false;
		typed = '';
	});

	let analysis = $derived(prepared?.analysis ?? recipe?.analysis ?? null);

	/** Typed confirmation for anything that changes system state or worse. */
	let needsTypedConfirm = $derived(dangerRank(analysis?.danger ?? 'benign') >= 2);

	let missing = $derived(
		(recipe?.params ?? []).filter((p) => !p.default && !(values[p.name] ?? '').trim())
	);

	let confirmed = $derived(!needsTypedConfirm || typed.trim() === recipe?.id);

	let blocked = $derived(!canRun || missing.length > 0 || !confirmed || busy);

	async function prepare(): Promise<void> {
		if (!recipe) return;
		prepareError = null;
		try {
			prepared = await recipePrepare(recipe.id, values);
			showCommand = true;
		} catch (e) {
			prepareError = String(e);
		}
	}

	async function run(): Promise<void> {
		if (!recipe || blocked) return;
		busy = true;
		try {
			// Prepared fresh rather than reusing whatever the preview built:
			// the values may have changed since, and the file may have too.
			const ready = await recipePrepare(recipe.id, values);
			onrun(ready.command);
			addToast(t('recipes.sent', { name: recipe.name, target: targetLabel }), 'success');
			onclose();
		} catch (e) {
			prepareError = String(e);
		} finally {
			busy = false;
		}
	}
</script>

<Modal open={!!recipe} {onclose} title={recipe?.name ?? ''} maxWidth="640px">
	{#if recipe}
		<div class="run">
			{#if recipe.description}
				<p class="description">{recipe.description}</p>
			{/if}

			<!-- The target, stated plainly. Running the right recipe on the wrong
			     machine is the mistake this whole dialog exists to prevent. -->
			<div class="target" class:unavailable={!canRun}>
				<span class="target-label">{t('recipes.runs_on')}</span>
				<code>{canRun ? targetLabel : t('recipes.no_session')}</code>
			</div>

			{#if analysis}
				<div class="risk" data-danger={analysis.danger}>
					<div class="risk-head">
						<DangerBadge danger={analysis.danger} />
						<span class="risk-summary">
							{analysis.findings.length === 0
								? t('recipes.risk_none')
								: t('recipes.risk_count', { count: analysis.findings.length })}
						</span>
					</div>

					{#if analysis.understated}
						<p class="risk-warn">
							{t('recipes.understated', {
								declared: t(`recipes.danger_${analysis.declared}`),
								actual: t(`recipes.danger_${analysis.danger}`)
							})}
						</p>
					{/if}

					{#if analysis.opaque.length > 0}
						<p class="risk-warn">
							{t('recipes.opaque', { lines: analysis.opaque.join(', ') })}
						</p>
					{/if}

					{#if analysis.findings.length > 0}
						<ul class="findings">
							{#each analysis.findings as f (f.line)}
								<li>
									<span class="line-no">{f.line}</span>
									<code class="line-text">{f.text}</code>
									<span class="line-reason">{f.reason}</span>
								</li>
							{/each}
						</ul>
					{/if}
				</div>
			{/if}

			{#if recipe.params.length > 0}
				<div class="params">
					{#each recipe.params as p (p.name)}
						<label class="param">
							<span class="param-label">
								{p.label}
								{#if !p.default}<span class="required">*</span>{/if}
							</span>
							<input
								type="text"
								bind:value={values[p.name]}
								placeholder={p.default || t('recipes.required')}
								spellcheck="false"
							/>
							<code class="param-name">${p.name}</code>
						</label>
					{/each}
				</div>
			{/if}

			<div class="preview">
				<button class="link" onclick={prepare}>
					{showCommand ? t('recipes.refresh_preview') : t('recipes.show_command')}
				</button>
				{#if prepared && showCommand}
					<pre class="command">{prepared.command}</pre>
				{/if}
			</div>

			{#if prepareError}
				<p class="error">{prepareError}</p>
			{/if}

			{#if needsTypedConfirm}
				<!-- Typed, not clicked. At this risk level a button is something
				     you hit by reflex on the way to somewhere else. -->
				<label class="confirm">
					<span>{t('recipes.type_to_confirm', { id: recipe.id })}</span>
					<input type="text" bind:value={typed} spellcheck="false" autocomplete="off" />
				</label>
			{/if}
		</div>
	{/if}

	{#snippet actions()}
		<Button variant="secondary" onclick={onclose}>{t('common.cancel')}</Button>
		<Button variant={needsTypedConfirm ? 'danger' : 'primary'} disabled={blocked} onclick={run}>
			{t('recipes.run')}
		</Button>
	{/snippet}
</Modal>

<style>
	.run {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
	}

	.description {
		margin: 0;
		font-size: var(--text-sm);
		color: var(--color-text-secondary);
	}

	.target {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-2) var(--space-3);
		font-size: var(--text-xs);
		background: var(--color-surface-sunken);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-sm);
	}

	.target.unavailable {
		border-color: var(--color-warning);
	}

	.target-label {
		color: var(--color-text-tertiary);
	}

	.target code {
		font-family: var(--font-mono);
		color: var(--color-text-primary);
	}

	.risk {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		padding: var(--space-3);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-card);
	}

	.risk[data-danger='sensitive'] {
		border-color: color-mix(in srgb, var(--color-warning) 60%, var(--color-border));
	}

	.risk[data-danger='destructive'] {
		border-color: var(--color-danger);
		background: color-mix(in srgb, var(--color-danger) 8%, transparent);
	}

	.risk-head {
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}

	.risk-summary {
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
	}

	.risk-warn {
		margin: 0;
		font-size: var(--text-xs);
		color: var(--color-warning);
	}

	.findings {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		margin: 0;
		padding: 0;
		max-height: 160px;
		overflow-y: auto;
		list-style: none;
	}

	.findings li {
		display: grid;
		grid-template-columns: 32px 1fr;
		gap: var(--space-2);
		align-items: baseline;
		font-size: var(--text-xs);
	}

	.line-no {
		font-family: var(--font-mono);
		color: var(--color-text-tertiary);
		text-align: right;
	}

	.line-text {
		font-family: var(--font-mono);
		color: var(--color-text-primary);
		overflow-wrap: anywhere;
	}

	.line-reason {
		grid-column: 2;
		color: var(--color-text-tertiary);
	}

	.params {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}

	.param {
		display: grid;
		grid-template-columns: 1fr 2fr auto;
		gap: var(--space-2);
		align-items: center;
	}

	.param-label {
		font-size: var(--text-sm);
		color: var(--color-text-primary);
	}

	.required {
		color: var(--color-danger);
	}

	.param input,
	.confirm input {
		padding: 6px 10px;
		font-family: var(--font-mono);
		font-size: var(--text-xs);
		color: var(--color-text-primary);
		background: var(--color-surface-sunken);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-sm);
	}

	.param-name {
		font-family: var(--font-mono);
		font-size: var(--text-2xs);
		color: var(--color-text-tertiary);
	}

	.preview {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}

	.link {
		align-self: flex-start;
		padding: 0;
		font-family: inherit;
		font-size: var(--text-xs);
		color: var(--color-accent);
		background: none;
		border: none;
		cursor: pointer;
		text-decoration: underline;
	}

	.command {
		margin: 0;
		padding: var(--space-2);
		max-height: 180px;
		overflow: auto;
		font-family: var(--font-mono);
		font-size: var(--text-2xs);
		line-height: 1.5;
		color: var(--color-text-secondary);
		background: var(--color-surface-sunken);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-sm);
		white-space: pre;
	}

	.confirm {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
	}

	.error {
		margin: 0;
		font-size: var(--text-xs);
		color: var(--color-danger);
	}
</style>
