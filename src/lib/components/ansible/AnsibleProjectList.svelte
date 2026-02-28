<script lang="ts">
	import { onMount } from 'svelte';
	import { t } from '$lib/state/i18n.svelte';
	import { getProjects, loadProjects, openProject, deleteProject } from '$lib/state/ansible.svelte';
	import Button from '$lib/components/shared/Button.svelte';
	import AnsibleProjectEditor from './AnsibleProjectEditor.svelte';

	let showEditor = $state(false);
	let deletingId = $state<string | null>(null);

	let projectList = $derived(getProjects());

	onMount(() => {
		loadProjects();
	});

	function handleCardClick(id: string) {
		openProject(id);
	}

	function handleCardKeydown(e: KeyboardEvent, id: string) {
		if (e.key === 'Enter' || e.key === ' ') {
			e.preventDefault();
			openProject(id);
		}
	}

	function handleDeleteClick(e: MouseEvent, id: string) {
		e.stopPropagation();
		deletingId = id;
	}

	async function confirmDelete() {
		if (!deletingId) return;
		await deleteProject(deletingId);
		deletingId = null;
	}

	function cancelDelete() {
		deletingId = null;
	}

	function truncatePath(path: string, maxLength: number = 50): string {
		if (path.length <= maxLength) return path;
		return '...' + path.slice(path.length - maxLength + 3);
	}

	function formatTimestamp(ts: string): string {
		try {
			// Handle both ISO 8601 strings and legacy Unix seconds
			let date = new Date(ts);
			if (isNaN(date.getTime()) && /^\d+$/.test(ts)) {
				date = new Date(Number(ts) * 1000);
			}
			if (isNaN(date.getTime())) return ts;
			return date.toLocaleDateString(undefined, {
				year: 'numeric',
				month: 'short',
				day: 'numeric',
				hour: '2-digit',
				minute: '2-digit'
			});
		} catch {
			return ts;
		}
	}
</script>

<div class="project-list">
	<header class="header">
		<h2 class="title">{t('ansible.projects')}</h2>
		<div class="header-actions">
			<Button variant="primary" size="sm" onclick={() => (showEditor = true)}>
				{t('ansible.new_project')}
			</Button>
		</div>
	</header>

	{#if projectList.length === 0}
		<div class="empty-state">
			<svg width="48" height="48" viewBox="0 0 24 24" fill="none" class="empty-icon">
				<circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="1.5" />
				<text x="12" y="16" text-anchor="middle" fill="currentColor" font-size="10" font-weight="bold">A</text>
			</svg>
			<p class="empty-text">{t('ansible.no_projects')}</p>
		</div>
	{:else}
		<div class="grid">
			{#each projectList as project (project.id)}
				<div
					class="card"
					role="button"
					tabindex="0"
					onclick={() => handleCardClick(project.id)}
					onkeydown={(e) => handleCardKeydown(e, project.id)}
				>
					<div class="card-header">
						<h3 class="card-name">{project.name}</h3>
						<button
							type="button"
							class="delete-btn"
							title={t('ansible.delete')}
							onclick={(e) => handleDeleteClick(e, project.id)}
						>
							<svg width="16" height="16" viewBox="0 0 24 24" fill="none">
								<path
									d="M3 6h18M8 6V4a2 2 0 012-2h4a2 2 0 012 2v2m3 0v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6h14"
									stroke="currentColor"
									stroke-width="1.5"
									stroke-linecap="round"
									stroke-linejoin="round"
								/>
							</svg>
						</button>
					</div>

					<p class="card-path" title={project.path}>{truncatePath(project.path)}</p>

					{#if project.description}
						<p class="card-description">{project.description}</p>
					{/if}

					{#if project.lastOpenedAt}
						<p class="card-timestamp">
							{t('ansible.last_opened')}: {formatTimestamp(project.lastOpenedAt)}
						</p>
					{/if}
				</div>
			{/each}
		</div>
	{/if}

	{#if deletingId !== null}
		<div class="confirm-overlay" role="presentation" onclick={cancelDelete}>
			<!-- svelte-ignore a11y_click_events_have_key_events -->
			<div class="confirm-dialog" role="alertdialog" tabindex="-1" onclick={(e) => e.stopPropagation()}>
				<p class="confirm-text">{t('ansible.delete_confirm')}</p>
				<div class="confirm-actions">
					<Button variant="secondary" size="sm" onclick={cancelDelete}>
						{t('ansible.cancel')}
					</Button>
					<Button variant="danger" size="sm" onclick={confirmDelete}>
						{t('ansible.delete')}
					</Button>
				</div>
			</div>
		</div>
	{/if}

	<AnsibleProjectEditor
		open={showEditor}
		onclose={() => { showEditor = false; }}
		oncreate={(project) => {
			showEditor = false;
			openProject(project.id);
		}}
	/>
</div>

<style>
	.project-list {
		width: 100%;
		height: 100%;
		overflow-y: auto;
		background: var(--color-bg-primary);
		padding: 24px;
	}

	.header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		margin-bottom: 24px;
	}

	.header-actions {
		display: flex;
		gap: 8px;
	}

	.title {
		margin: 0;
		font-size: 1.25rem;
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.empty-state {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 16px;
		padding: 64px 32px;
		text-align: center;
	}

	.empty-icon {
		color: var(--color-text-secondary);
		opacity: 0.4;
	}

	.empty-text {
		margin: 0;
		font-size: 0.875rem;
		color: var(--color-text-secondary);
		max-width: 400px;
		line-height: 1.5;
	}

	.grid {
		display: grid;
		grid-template-columns: repeat(3, 1fr);
		gap: 16px;
	}

	@media (max-width: 900px) {
		.grid {
			grid-template-columns: repeat(2, 1fr);
		}
	}

	@media (max-width: 600px) {
		.grid {
			grid-template-columns: 1fr;
		}
	}

	.card {
		display: flex;
		flex-direction: column;
		gap: 8px;
		padding: 16px;
		background: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		cursor: pointer;
		text-align: left;
		font-family: inherit;
		font-size: inherit;
		color: inherit;
		transition:
			border-color 0.15s ease,
			box-shadow 0.15s ease;
	}

	.card:hover {
		border-color: var(--color-accent);
		box-shadow: var(--shadow-elevated);
	}

	.card-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
	}

	.card-name {
		margin: 0;
		font-size: 0.9375rem;
		font-weight: 600;
		color: var(--color-text-primary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.delete-btn {
		flex-shrink: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		width: 28px;
		height: 28px;
		padding: 0;
		background: transparent;
		border: none;
		border-radius: var(--radius-btn);
		color: var(--color-text-secondary);
		cursor: pointer;
		transition:
			color 0.15s ease,
			background-color 0.15s ease;
	}

	.delete-btn:hover {
		color: var(--color-danger);
		background-color: rgba(255, 82, 82, 0.1);
	}

	.card-path {
		margin: 0;
		font-size: 0.75rem;
		color: var(--color-text-secondary);
		font-family: monospace;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.card-description {
		margin: 0;
		font-size: 0.8125rem;
		color: var(--color-text-secondary);
		line-height: 1.4;
		display: -webkit-box;
		-webkit-line-clamp: 2;
		line-clamp: 2;
		-webkit-box-orient: vertical;
		overflow: hidden;
	}

	.card-timestamp {
		margin: 0;
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
		opacity: 0.7;
		margin-top: auto;
	}

	.confirm-overlay {
		position: fixed;
		inset: 0;
		background: rgba(0, 0, 0, 0.5);
		display: flex;
		align-items: center;
		justify-content: center;
		z-index: 1000;
	}

	.confirm-dialog {
		background: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-modal);
		padding: 24px;
		max-width: 400px;
		width: 90%;
		box-shadow: var(--shadow-elevated);
	}

	.confirm-text {
		margin: 0 0 20px 0;
		font-size: 0.875rem;
		color: var(--color-text-primary);
		line-height: 1.5;
	}

	.confirm-actions {
		display: flex;
		justify-content: flex-end;
		gap: 8px;
	}
</style>
