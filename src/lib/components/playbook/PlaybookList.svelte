<script lang="ts">
	import { playbookList, playbookListSaved, playbookDelete, type PlaybookRun, type SavedPlaybook } from '$lib/ipc/playbook';
	import { addToast } from '$lib/state/toasts.svelte';
	import PlaybookEditor from './PlaybookEditor.svelte';
	import PlaybookRunner from './PlaybookRunner.svelte';
	import { untrack } from 'svelte';

	interface Props {
		connectionId?: string;
	}

	let { connectionId }: Props = $props();

	let saved = $state<SavedPlaybook[]>([]);
	let runs = $state<PlaybookRun[]>([]);
	let loading = $state(true);
	let showEditor = $state(false);
	let showRunner = $state(false);
	let runnerYaml = $state('');
	let editId = $state<string | undefined>();
	let editYaml = $state<string | undefined>();

	async function loadPlaybooks(): Promise<void> {
		try {
			const [savedResult, runsResult] = await Promise.all([
				playbookListSaved(),
				playbookList()
			]);
			saved = savedResult.sort((a, b) => b.updated_at - a.updated_at);
			runs = runsResult;
		} catch (err) {
			console.error('Failed to load playbooks:', err);
		} finally {
			loading = false;
		}
	}

	function handleNewPlaybook(): void {
		editId = undefined;
		editYaml = undefined;
		showEditor = true;
	}

	function handleEditPlaybook(pb: SavedPlaybook): void {
		editId = pb.id;
		editYaml = pb.yaml_content;
		showEditor = true;
	}

	async function handleDeletePlaybook(e: MouseEvent, pb: SavedPlaybook): Promise<void> {
		e.stopPropagation();
		try {
			await playbookDelete(pb.id);
			addToast('Playbook deleted', 'info');
			loadPlaybooks();
		} catch (err) {
			addToast(String(err), 'error');
		}
	}

	function handleRunSaved(e: MouseEvent, pb: SavedPlaybook): void {
		e.stopPropagation();
		runnerYaml = pb.yaml_content;
		showRunner = true;
	}

	function handleRunFromEditor(yaml: string): void {
		showEditor = false;
		runnerYaml = yaml;
		showRunner = true;
	}

	function handleEditorSaved(): void {
		loadPlaybooks();
	}

	function handleRunnerClose(): void {
		showRunner = false;
		loadPlaybooks();
	}

	function statusColor(status: PlaybookRun['status']): string {
		switch (status) {
			case 'Running':
				return 'var(--color-accent)';
			case 'Completed':
				return 'var(--color-success)';
			case 'Failed':
				return 'var(--color-danger)';
			case 'Stopped':
				return 'var(--color-warning)';
			default:
				return 'var(--color-text-secondary)';
		}
	}

	$effect(() => {
		untrack(() => loadPlaybooks());
	});
</script>

<div class="playbook-list">
	<div class="actions-row">
		<button class="new-btn" onclick={handleNewPlaybook}>
			<svg width="11" height="11" viewBox="0 0 24 24" fill="none">
				<path
					d="M12 5v14M5 12h14"
					stroke="currentColor"
					stroke-width="2"
					stroke-linecap="round"
				/>
			</svg>
			New Playbook
		</button>
		<button class="run-btn" onclick={() => { runnerYaml = ''; showRunner = true; }}>
			<svg width="11" height="11" viewBox="0 0 24 24" fill="none">
				<path
					d="M5 3l14 9-14 9V3z"
					stroke="currentColor"
					stroke-width="2"
					stroke-linecap="round"
					stroke-linejoin="round"
				/>
			</svg>
			Run
		</button>
	</div>

	{#if loading}
		<div class="loading-state">
			<span class="spinner"></span>
			<span class="loading-text">Loading playbooks...</span>
		</div>
	{:else if saved.length === 0 && runs.length === 0}
		<p class="empty-state">No playbooks yet. Create one to get started.</p>
	{:else}
		{#if saved.length > 0}
			<div class="divider"></div>
			<div class="runs-scroll">
				{#each saved as pb (pb.id)}
					<!-- svelte-ignore a11y_no_static_element_interactions -->
					<div class="run-card" onclick={() => handleEditPlaybook(pb)} onkeydown={(e) => { if (e.key === 'Enter') handleEditPlaybook(pb); }} role="button" tabindex="0">
						<div class="run-info">
							<span class="run-name">{pb.name}</span>
							<span class="run-progress">Saved</span>
						</div>
						<div class="card-actions">
							<button class="card-action-btn play-btn" onclick={(e) => handleRunSaved(e, pb)} title="Run">
								<svg width="10" height="10" viewBox="0 0 24 24" fill="none">
									<path d="M5 3l14 9-14 9V3z" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
								</svg>
							</button>
							<button class="card-action-btn delete-btn" onclick={(e) => handleDeletePlaybook(e, pb)} title="Delete">
								<svg width="10" height="10" viewBox="0 0 10 10" fill="none">
									<path d="M1.5 1.5l7 7M8.5 1.5l-7 7" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" />
								</svg>
							</button>
						</div>
					</div>
				{/each}
			</div>
		{/if}
		{#if runs.length > 0}
			<div class="section-label">Recent Runs</div>
			<div class="runs-scroll">
				{#each runs as run (run.id)}
					<div class="run-card">
						<div class="run-info">
							<span class="run-name">{run.playbook_name}</span>
							<span class="run-progress">
								Step {run.current_step}/{run.total_steps}
							</span>
						</div>
						<span
							class="status-badge"
							style:color={statusColor(run.status)}
							style:border-color={statusColor(run.status)}
						>
							{#if run.status === 'Running'}
								<span class="status-dot running"></span>
							{/if}
							{run.status}
						</span>
					</div>
				{/each}
			</div>
		{/if}
	{/if}
</div>

<PlaybookEditor
	open={showEditor}
	editId={editId}
	initialYaml={editYaml}
	onclose={() => (showEditor = false)}
	onsave={handleEditorSaved}
	onrun={handleRunFromEditor}
/>

<PlaybookRunner
	open={showRunner}
	onclose={handleRunnerClose}
	connectionId={connectionId ?? ''}
	yaml={runnerYaml}
/>

<style>
	.playbook-list {
		display: flex;
		flex-direction: column;
		gap: 8px;
		padding: 4px 0;
	}

	.actions-row {
		display: flex;
		gap: 4px;
	}

	.new-btn,
	.run-btn {
		display: flex;
		align-items: center;
		gap: 4px;
		flex: 1;
		padding: 5px 8px;
		font-family: var(--font-sans);
		font-size: 0.6875rem;
		font-weight: 500;
		border-radius: 6px;
		cursor: pointer;
		transition:
			background-color var(--duration-default) var(--ease-default),
			color var(--duration-default) var(--ease-default);
	}

	.new-btn {
		color: var(--color-text-secondary);
		background: transparent;
		border: 1px solid var(--color-border);
	}

	.new-btn:hover {
		background-color: rgba(255, 255, 255, 0.06);
		color: var(--color-text-primary);
	}

	.run-btn {
		color: var(--color-accent);
		background: transparent;
		border: 1px solid var(--color-accent);
	}

	.run-btn:hover {
		background-color: rgba(0, 122, 255, 0.1);
	}

	.new-btn:active,
	.run-btn:active {
		transform: scale(0.98);
	}

	.divider {
		height: 1px;
		background-color: var(--color-border);
		opacity: 0.5;
		margin: 2px 0;
	}

	.runs-scroll {
		display: flex;
		flex-direction: column;
		gap: 2px;
		overflow-y: auto;
	}

	.run-card {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 8px 10px;
		border: none;
		border-radius: var(--radius-card, 8px);
		background: transparent;
		color: inherit;
		cursor: pointer;
		text-align: left;
		font-family: var(--font-sans);
		transition: background-color var(--duration-default) var(--ease-default);
		width: 100%;
	}

	.run-card:hover {
		background-color: rgba(255, 255, 255, 0.04);
	}

	.run-card:active {
		transform: scale(0.98);
	}

	.run-info {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.run-name {
		font-size: 0.8125rem;
		font-weight: 500;
		color: var(--color-text-primary);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.run-progress {
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
		font-family: var(--font-mono, monospace);
	}

	.status-badge {
		flex-shrink: 0;
		display: inline-flex;
		align-items: center;
		gap: 4px;
		padding: 2px 8px;
		font-size: 0.625rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.04em;
		border: 1px solid;
		border-radius: 4px;
		white-space: nowrap;
	}

	.status-dot {
		display: inline-block;
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background-color: currentColor;
	}

	.status-dot.running {
		animation: pulse 1.5s ease-in-out infinite;
	}

	.card-actions {
		display: flex;
		gap: 4px;
		flex-shrink: 0;
	}

	.card-action-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 22px;
		height: 22px;
		border: none;
		border-radius: 4px;
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
		transition: background-color var(--duration-default) var(--ease-default),
			color var(--duration-default) var(--ease-default);
		font-family: var(--font-sans);
	}

	.card-action-btn:hover {
		background-color: rgba(255, 255, 255, 0.08);
	}

	.play-btn:hover {
		color: var(--color-accent);
	}

	.delete-btn:hover {
		color: var(--color-danger);
	}

	.section-label {
		font-size: 0.625rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--color-text-secondary);
		padding: 6px 4px 2px;
		opacity: 0.7;
	}

	.loading-state {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 8px;
		padding: 16px 0;
	}

	.loading-text {
		font-size: 0.75rem;
		color: var(--color-text-secondary);
	}

	.empty-state {
		margin: 0;
		padding: 12px 4px;
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
		opacity: 0.7;
		text-align: center;
	}

	.spinner {
		display: inline-block;
		width: 14px;
		height: 14px;
		border: 2px solid rgba(255, 255, 255, 0.15);
		border-top-color: var(--color-accent);
		border-radius: 50%;
		animation: spin 0.6s linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}

	@keyframes pulse {
		0%,
		100% {
			opacity: 1;
		}
		50% {
			opacity: 0.3;
		}
	}
</style>
