<!--
	The recipe list: what is installed, and what could be.

	Installed and registry recipes are kept in separate tabs rather than merged.
	They answer different questions — "what can I run on this box right now"
	versus "what could I add" — and one blended list made it impossible to see
	at a glance which scripts had already been brought onto this machine.
-->
<script lang="ts">
	import { onMount } from 'svelte';
	import { getActiveTab } from '$lib/state/tabs.svelte';
	import { sshSend } from '$lib/ipc/ssh';
	import { ptyWrite } from '$lib/ipc/pty';
	import { addToast } from '$lib/state/toasts.svelte';
	import { t } from '$lib/state/i18n.svelte';
	import { recipeTemplate, type Recipe, type RecipeEntry } from '$lib/ipc/recipes';
	import {
		getRecipes,
		getRegistry,
		getRegistryError,
		isInstalled,
		isRegistryLoading,
		loadRecipes,
		loadRegistry,
		deleteRecipe,
		installRecipe
	} from '$lib/state/recipes.svelte';
	import DangerBadge from './DangerBadge.svelte';
	import RecipeEditorDialog from './RecipeEditorDialog.svelte';
	import RecipeRunDialog from './RecipeRunDialog.svelte';

	let recipes = $derived(getRecipes());
	let registry = $derived(getRegistry());
	let activeTab = $derived(getActiveTab());

	let view = $state<'installed' | 'registry'>('installed');
	let search = $state('');
	let running = $state<Recipe | null>(null);
	/** The file being edited, or null when the editor is closed. */
	let editorSource = $state<string | null>(null);
	let installing = $state<string | null>(null);

	onMount(loadRecipes);

	/** A session must exist before a recipe has anywhere to go. */
	let canRun = $derived(!!activeTab?.connectionId);
	let targetLabel = $derived(activeTab?.title ?? t('recipes.no_session'));

	let filteredRecipes = $derived.by(() => {
		const q = search.trim().toLowerCase();
		if (!q) return recipes;
		return recipes.filter(
			(r) =>
				r.name.toLowerCase().includes(q) ||
				r.description.toLowerCase().includes(q) ||
				r.id.includes(q) ||
				r.tags.some((tag) => tag.toLowerCase().includes(q))
		);
	});

	let filteredRegistry = $derived.by(() => {
		const q = search.trim().toLowerCase();
		if (!q) return registry;
		return registry.filter(
			(e) =>
				e.name.toLowerCase().includes(q) ||
				e.description.toLowerCase().includes(q) ||
				e.tags.some((tag) => tag.toLowerCase().includes(q))
		);
	});

	function showRegistry(): void {
		view = 'registry';
		if (registry.length === 0) loadRegistry();
	}

	/**
	 * Write the prepared command into the session the user is looking at.
	 *
	 * The same path their own typing takes, deliberately: a recipe can only
	 * ever reach a machine already open in front of them.
	 */
	function send(command: string): void {
		const connId = activeTab?.connectionId;
		if (!connId) return;
		const bytes = Array.from(new TextEncoder().encode(command));
		if (activeTab?.type === 'ssh') {
			sshSend(connId, bytes);
		} else {
			ptyWrite(connId, bytes);
		}
	}

	async function newRecipe(): Promise<void> {
		const stamp = Date.now().toString(36);
		editorSource = await recipeTemplate(`recipe-${stamp}`, 'New recipe');
	}

	function edit(recipe: Recipe): void {
		editorSource = recipe.source;
	}

	async function remove(recipe: Recipe): Promise<void> {
		try {
			await deleteRecipe(recipe.id);
			addToast(t('recipes.deleted', { name: recipe.name }), 'success');
		} catch (e) {
			addToast(String(e), 'error');
		}
	}

	async function install(entry: RecipeEntry): Promise<void> {
		installing = entry.id;
		try {
			await installRecipe(entry);
			addToast(t('recipes.install_done', { name: entry.name }), 'success');
		} catch (e) {
			addToast(String(e), 'error');
		} finally {
			installing = null;
		}
	}
</script>

<div class="panel">
	<div class="head">
		<div class="tabs">
			<button class:active={view === 'installed'} onclick={() => (view = 'installed')}>
				{t('recipes.installed')}
			</button>
			<button class:active={view === 'registry'} onclick={showRegistry}>
				{t('recipes.browse')}
			</button>
		</div>
		<button class="icon-btn" title={t('recipes.new')} onclick={newRecipe} aria-label={t('recipes.new')}>
			<svg width="14" height="14" viewBox="0 0 14 14" fill="none">
				<path d="M7 1v12M1 7h12" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
			</svg>
		</button>
	</div>

	<input class="search" type="text" placeholder={t('recipes.search')} bind:value={search} />

	{#if !canRun && view === 'installed'}
		<!-- Said once, up front, rather than discovered at the moment of running.
		     Only on this tab: there is nothing to run from the registry list. -->
		<p class="hint">{t('recipes.open_a_session')}</p>
	{/if}

	{#if view === 'installed'}
		{#if filteredRecipes.length === 0}
			<p class="empty">{search ? t('recipes.no_matches') : t('recipes.none_yet')}</p>
		{:else}
			<ul class="list">
				{#each filteredRecipes as r (r.id)}
					<li class="item">
						<span class="name">{r.name}</span>
						{#if r.description}
							<p class="desc">{r.description}</p>
						{/if}
						<div class="meta">
							<DangerBadge danger={r.analysis.danger} compact />
							{#if r.origin.kind === 'registry'}
								<span class="origin" title={r.origin.repo}>{t('recipes.from_registry')}</span>
							{/if}
							{#if r.version}<span class="dim">v{r.version}</span>{/if}
							{#if r.params.length > 0}
								<span class="dim">
									{r.params.length === 1
										? t('recipes.one_param')
										: t('recipes.n_params', { count: r.params.length })}
								</span>
							{/if}
						</div>
						<div class="actions">
							<button class="mini primary" disabled={!canRun} onclick={() => (running = r)}>
								{t('recipes.run')}
							</button>
							<button class="mini" onclick={() => edit(r)}>{t('common.edit')}</button>
							<button class="mini danger" onclick={() => remove(r)}>{t('common.delete')}</button>
						</div>
					</li>
				{/each}
			</ul>
		{/if}
	{:else if isRegistryLoading()}
		<p class="empty">{t('common.loading')}</p>
	{:else if getRegistryError()}
		<p class="error">{getRegistryError()}</p>
		<button class="mini" onclick={() => loadRegistry()}>{t('common.retry')}</button>
	{:else if filteredRegistry.length === 0}
		<p class="empty">{t('recipes.registry_empty')}</p>
	{:else}
		<ul class="list">
			{#each filteredRegistry as e (e.id)}
				<li class="item">
					<span class="name">{e.name}</span>
					{#if e.description}
						<p class="desc">{e.description}</p>
					{/if}
					<div class="meta">
						{#if e.danger}<DangerBadge danger={e.danger} compact />{/if}
						{#if e.author}<span class="dim">{e.author}</span>{/if}
						{#if e.version}<span class="dim">v{e.version}</span>{/if}
					</div>
					<div class="actions">
						{#if isInstalled(e.id)}
							<span class="installed">{t('recipes.already_installed')}</span>
						{:else}
							<button
								class="mini primary"
								disabled={installing === e.id}
								onclick={() => install(e)}
							>
								{installing === e.id ? t('common.loading') : t('recipes.install')}
							</button>
						{/if}
					</div>
				</li>
			{/each}
		</ul>
	{/if}
</div>

<RecipeRunDialog
	recipe={running}
	{targetLabel}
	{canRun}
	onclose={() => (running = null)}
	onrun={send}
/>

<!-- Mounted per edit rather than toggled, so the editor's initial document is
     the recipe rather than an empty string it can never be told about. -->
{#if editorSource !== null}
	<RecipeEditorDialog source={editorSource} onclose={() => (editorSource = null)} />
{/if}

<style>
	.panel {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		padding: var(--space-2);
		height: 100%;
		overflow: hidden;
	}

	.head {
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}

	.tabs {
		display: flex;
		flex: 1;
		gap: 2px;
		min-width: 0;
	}

	.tabs button {
		flex: 1;
		padding: 4px 8px;
		font-family: inherit;
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
		background: none;
		border: 1px solid transparent;
		border-radius: var(--radius-sm);
		cursor: pointer;
		white-space: nowrap;
	}

	.tabs button.active {
		color: var(--color-text-primary);
		background: var(--color-surface-active);
		border-color: var(--color-border);
	}

	.icon-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 24px;
		height: 24px;
		flex-shrink: 0;
		color: var(--color-text-secondary);
		background: none;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-sm);
		cursor: pointer;
	}

	.icon-btn:hover {
		color: var(--color-text-primary);
	}

	.search {
		padding: 5px 8px;
		font-family: inherit;
		font-size: var(--text-xs);
		color: var(--color-text-primary);
		background: var(--color-surface-sunken);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-sm);
	}

	.hint,
	.empty,
	.error {
		margin: 0;
		padding: var(--space-2);
		font-size: var(--text-xs);
		color: var(--color-text-tertiary);
	}

	.error {
		color: var(--color-danger);
	}

	.list {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		margin: 0;
		padding: 0;
		list-style: none;
		overflow-y: auto;
	}

	.item {
		display: flex;
		flex-direction: column;
		gap: 4px;
		padding: var(--space-2);
		background: var(--color-surface-sunken);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-sm);
	}

	.name {
		font-size: var(--text-sm);
		font-weight: 500;
		line-height: 1.3;
		color: var(--color-text-primary);
		/* Wraps rather than truncating. In a 240px sidebar the badge used to
		   win this fight and every real title became "Harden the SSH...".
		   The name is the one thing that has to survive a narrow panel. */
		overflow-wrap: anywhere;
	}

	.desc {
		margin: 0;
		font-size: var(--text-xs);
		line-height: 1.4;
		color: var(--color-text-secondary);
	}

	.meta {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 6px;
		margin-top: 2px;
		font-size: var(--text-2xs);
	}

	.dim {
		color: var(--color-text-tertiary);
	}

	.origin {
		color: var(--color-accent);
	}

	.actions {
		display: flex;
		flex-wrap: wrap;
		gap: 4px;
		margin-top: 2px;
	}

	.mini {
		padding: 3px 8px;
		font-family: inherit;
		font-size: var(--text-2xs);
		color: var(--color-text-secondary);
		background: none;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-sm);
		cursor: pointer;
	}

	.mini:hover:not(:disabled) {
		color: var(--color-text-primary);
	}

	.mini:disabled {
		opacity: 0.45;
		cursor: not-allowed;
	}

	.mini.primary {
		color: var(--color-accent);
		border-color: color-mix(in srgb, var(--color-accent) 50%, var(--color-border));
	}

	.mini.danger:hover {
		color: var(--color-danger);
		border-color: var(--color-danger);
	}

	.installed {
		font-size: var(--text-2xs);
		color: var(--color-success);
	}
</style>
