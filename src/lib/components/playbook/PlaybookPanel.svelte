<script lang="ts">
	import {
		getPlaybookContent,
		setPlaybookContent,
		getExtraVars,
		setExtraVars,
		getBecome,
		setBecome,
		getSelectedConnectionId,
		setSelectedConnectionId,
		getActiveRun,
		setActiveRun,
		getOutputBuffer,
		appendOutput,
		clearOutput,
		getIsRunning,
		getSavedProjects,
		setSavedProjects
	} from '$lib/state/playbook.svelte';
	import {
		playbookRun,
		playbookCancel,
		playbookValidate,
		playbookListProjects,
		playbookSaveProject,
		playbookDeleteProject,
		type PlaybookOutputEvent,
		type PlaybookCompleteEvent
	} from '$lib/ipc/playbook';
	import { sshListConnections, type ConnectionInfo } from '$lib/ipc/ssh';
	import { listen, type UnlistenFn } from '@tauri-apps/api/event';
	import { t } from '$lib/state/i18n.svelte';
	import { addToast } from '$lib/state/toasts.svelte';

	interface Props {
		connectionId?: string;
	}

	let { connectionId }: Props = $props();

	let connections = $state<ConnectionInfo[]>([]);
	let selectedProjectId = $state<string | null>(null);
	let outputEl: HTMLPreElement | undefined = $state();

	let playbookContent = $derived(getPlaybookContent());
	let extraVars = $derived(getExtraVars());
	let become = $derived(getBecome());
	let selectedConnId = $derived(getSelectedConnectionId());
	let isRunning = $derived(getIsRunning());
	let outputBuffer = $derived(getOutputBuffer());
	let savedProjects = $derived(getSavedProjects());

	let unlistenOutput: UnlistenFn | null = null;
	let unlistenComplete: UnlistenFn | null = null;

	$effect(() => {
		loadConnections();
		loadProjects();
	});

	$effect(() => {
		if (connectionId !== undefined) {
			setSelectedConnectionId(connectionId);
		}
	});

	$effect(() => {
		const _buffer = outputBuffer;
		if (outputEl) {
			requestAnimationFrame(() => {
				if (outputEl) {
					outputEl.scrollTop = outputEl.scrollHeight;
				}
			});
		}
	});

	$effect(() => {
		return () => {
			cleanupListeners();
		};
	});

	async function loadConnections(): Promise<void> {
		try {
			connections = await sshListConnections();
		} catch {
			connections = [];
		}
	}

	async function loadProjects(): Promise<void> {
		try {
			const projects = await playbookListProjects();
			setSavedProjects(projects);
		} catch {
			// Ignore
		}
	}

	function cleanupListeners(): void {
		if (unlistenOutput) {
			unlistenOutput();
			unlistenOutput = null;
		}
		if (unlistenComplete) {
			unlistenComplete();
			unlistenComplete = null;
		}
	}

	async function runPlaybook(): Promise<void> {
		const content = getPlaybookContent();
		if (!content.trim()) {
			addToast(t('playbook.playbook_placeholder'), 'error');
			return;
		}

		const connId = getSelectedConnectionId();
		if (!connId) {
			addToast(t('playbook.no_connection'), 'error');
			return;
		}

		clearOutput();
		cleanupListeners();

		const runId = crypto.randomUUID();

		unlistenOutput = await listen<PlaybookOutputEvent>(
			`playbook-output-${runId}`,
			(event) => {
				appendOutput(event.payload.data);
			}
		);

		unlistenComplete = await listen<PlaybookCompleteEvent>(
			`playbook-complete-${runId}`,
			(event) => {
				setActiveRun(null);
				cleanupListeners();
				if (event.payload.status === 'Completed') {
					addToast(
						`${t('playbook.completed')} - ${t('playbook.tasks_summary', { ok: String(event.payload.tasks_ok), failed: String(event.payload.tasks_failed) })}`,
						'success'
					);
				} else {
					addToast(t('playbook.failed'), 'error');
				}
			}
		);

		try {
			const run = await playbookRun(content, connId, {
				runId,
				useBecome: getBecome(),
				extraVars: getExtraVars() || undefined
			});
			setActiveRun(run);
		} catch (err) {
			setActiveRun(null);
			cleanupListeners();
			addToast(`${t('playbook.failed')}: ${err}`, 'error');
		}
	}

	async function validatePlaybook(): Promise<void> {
		const content = getPlaybookContent();
		if (!content.trim()) {
			addToast(t('playbook.playbook_placeholder'), 'error');
			return;
		}

		try {
			clearOutput();
			const result = await playbookValidate(content);
			if (result.valid) {
				appendOutput(`${t('playbook.validation_ok', { count: String(result.tasks.length) })}`);
				for (const task of result.tasks) {
					appendOutput(`  - ${task}`);
				}
			} else {
				appendOutput(`${t('playbook.validation_failed')}: ${result.error ?? ''}`);
			}
		} catch (err) {
			appendOutput(`${err}`);
			addToast(t('playbook.failed'), 'error');
		}
	}

	async function cancelRun(): Promise<void> {
		const run = getActiveRun();
		if (run) {
			try {
				await playbookCancel(run.id);
			} catch (err) {
				addToast(`Cancel failed: ${err}`, 'error');
			}
		}
	}

	async function saveProject(): Promise<void> {
		const content = getPlaybookContent();
		if (!content.trim()) return;

		const name = prompt(t('playbook.save_project'));
		if (!name) return;

		try {
			await playbookSaveProject(name, content, {
				id: selectedProjectId ?? undefined,
				connectionId: getSelectedConnectionId() ?? undefined,
				become: getBecome()
			});
			await loadProjects();
			addToast(t('playbook.project_saved'), 'success');
		} catch (err) {
			addToast(`Save failed: ${err}`, 'error');
		}
	}

	async function deleteProject(): Promise<void> {
		if (!selectedProjectId) return;
		try {
			await playbookDeleteProject(selectedProjectId);
			selectedProjectId = null;
			await loadProjects();
			addToast(t('playbook.project_deleted'), 'info');
		} catch (err) {
			addToast(`Delete failed: ${err}`, 'error');
		}
	}

	async function copyOutput(): Promise<void> {
		const text = getOutputBuffer();
		if (text) {
			await navigator.clipboard.writeText(text);
			addToast(t('common.copy'), 'success');
		}
	}

	function selectProject(id: string): void {
		selectedProjectId = id;
		const proj = getSavedProjects().find((p) => p.id === id);
		if (proj) {
			setPlaybookContent(proj.playbookContent);
			setSelectedConnectionId(proj.connectionId);
			setBecome(proj.become);
		}
	}
</script>

<div class="playbook-panel">
	<div class="section-body">
		<!-- Connection selector -->
		<div class="section-row">
			<select
				class="tf-select"
				value={selectedConnId ?? ''}
				onchange={(e) => setSelectedConnectionId(e.currentTarget.value || null)}
			>
				<option value="">{t('playbook.select_connection')}</option>
				{#each connections as conn (conn.id)}
					<option value={conn.id}>{conn.username}@{conn.host}:{conn.port}</option>
				{/each}
			</select>
		</div>

		<!-- YAML editor textarea -->
		<div class="section-row editor-row">
			<textarea
				class="yaml-editor"
				value={playbookContent}
				oninput={(e) => setPlaybookContent(e.currentTarget.value)}
				placeholder={t('playbook.playbook_placeholder')}
				spellcheck="false"
			></textarea>
		</div>

		<!-- Extra vars (optional) -->
		<div class="section-row">
			<input
				class="tf-input"
				type="text"
				value={extraVars}
				oninput={(e) => setExtraVars(e.currentTarget.value)}
				placeholder={t('playbook.extra_vars_placeholder')}
			/>
		</div>

		<!-- Become toggle -->
		<div class="section-row become-row">
			<label class="become-label">
				<input
					type="checkbox"
					checked={become}
					onchange={(e) => setBecome(e.currentTarget.checked)}
				/>
				{t('playbook.become')}
			</label>
		</div>

		<!-- Saved projects -->
		<div class="section-row workspace-row">
			<select
				class="tf-select workspace-select"
				value={selectedProjectId ?? ''}
				onchange={(e) => {
					const val = e.currentTarget.value;
					if (val) selectProject(val);
					else selectedProjectId = null;
				}}
			>
				<option value="">{t('playbook.select_project')}</option>
				{#each savedProjects as proj (proj.id)}
					<option value={proj.id}>{proj.name}</option>
				{/each}
			</select>
			<button class="icon-btn" onclick={saveProject} title={t('playbook.save_project')}>
				<svg width="14" height="14" viewBox="0 0 24 24" fill="none">
					<path d="M19 21H5a2 2 0 01-2-2V5a2 2 0 012-2h11l5 5v11a2 2 0 01-2 2z" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
					<polyline points="17 21 17 13 7 13 7 21" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
					<polyline points="7 3 7 8 15 8" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
				</svg>
			</button>
			<button
				class="icon-btn danger"
				onclick={deleteProject}
				disabled={!selectedProjectId}
				title="Delete project"
			>
				<svg width="14" height="14" viewBox="0 0 24 24" fill="none">
					<polyline points="3 6 5 6 21 6" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
					<path d="M19 6l-1 14a2 2 0 01-2 2H8a2 2 0 01-2-2L5 6" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
					<path d="M10 11v6" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
					<path d="M14 11v6" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
				</svg>
			</button>
		</div>

		<!-- Action buttons -->
		<div class="section-row action-row">
			<button class="action-btn accent" disabled={isRunning} onclick={runPlaybook}>
				{t('playbook.run')}
			</button>
			<button class="action-btn" disabled={isRunning} onclick={validatePlaybook}>
				{t('playbook.validate')}
			</button>
		</div>

		<!-- Output console -->
		<div class="output-section">
			<div class="output-bar">
				{#if isRunning}
					<span class="running-label">{t('playbook.running')}</span>
					<div class="output-bar-actions">
						<button class="cancel-btn" onclick={cancelRun}>
							{t('playbook.cancel')}
						</button>
					</div>
				{:else}
					<span class="output-bar-title"></span>
					<div class="output-bar-actions">
						{#if outputBuffer}
							<button class="icon-btn" onclick={copyOutput} title={t('common.copy')}>
								<svg width="13" height="13" viewBox="0 0 24 24" fill="none">
									<rect x="9" y="9" width="13" height="13" rx="2" stroke="currentColor" stroke-width="2" />
									<path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1" stroke="currentColor" stroke-width="2" />
								</svg>
							</button>
						{/if}
					</div>
				{/if}
			</div>
			<pre class="output-console" bind:this={outputEl}>{outputBuffer}</pre>
		</div>
	</div>
</div>

<style>
	.playbook-panel {
		display: flex;
		flex-direction: column;
		height: 100%;
		overflow: hidden;
	}

	.section-body {
		flex: 1;
		overflow-y: auto;
		display: flex;
		flex-direction: column;
		gap: 0;
	}

	.section-row {
		padding: 6px 8px;
		border-bottom: 1px solid var(--color-border);
	}

	.editor-row {
		padding: 0;
	}

	.yaml-editor {
		width: 100%;
		min-height: 120px;
		max-height: 300px;
		padding: 8px;
		font-family: var(--font-mono);
		font-size: 0.6875rem;
		line-height: 1.5;
		color: var(--color-text-primary);
		background-color: var(--color-bg-primary);
		border: none;
		border-bottom: 1px solid var(--color-border);
		resize: vertical;
		outline: none;
		box-sizing: border-box;
		tab-size: 2;
		white-space: pre;
		overflow-wrap: normal;
		overflow-x: auto;
	}

	.yaml-editor::placeholder {
		color: var(--color-text-secondary);
		opacity: 0.6;
	}

	.yaml-editor:focus {
		border-color: var(--color-accent);
	}

	.tf-input,
	.tf-select {
		width: 100%;
		padding: 5px 8px;
		font-size: 0.75rem;
		color: var(--color-text-primary);
		background-color: var(--color-bg-primary);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		outline: none;
		transition: border-color var(--duration-default) var(--ease-default);
		box-sizing: border-box;
	}

	.tf-input::placeholder {
		color: var(--color-text-secondary);
		opacity: 0.6;
	}

	.tf-input:focus,
	.tf-select:focus {
		border-color: var(--color-accent);
	}

	.tf-select {
		appearance: none;
		background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='12' viewBox='0 0 24 24' fill='none' stroke='%2386868b' stroke-width='2'%3E%3Cpolyline points='6 9 12 15 18 9'/%3E%3C/svg%3E");
		background-repeat: no-repeat;
		background-position: right 6px center;
		padding-right: 24px;
	}

	.become-row {
		display: flex;
		align-items: center;
	}

	.become-label {
		display: flex;
		align-items: center;
		gap: 6px;
		font-size: 0.75rem;
		color: var(--color-text-secondary);
		cursor: pointer;
	}

	.become-label input[type='checkbox'] {
		accent-color: var(--color-accent);
	}

	.workspace-row {
		display: flex;
		align-items: center;
		gap: 4px;
	}

	.workspace-select {
		flex: 1;
		min-width: 0;
	}

	.icon-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 5px;
		color: var(--color-text-secondary);
		background: transparent;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		cursor: pointer;
		transition: all var(--duration-default) var(--ease-default);
		flex-shrink: 0;
	}

	.icon-btn:hover:not(:disabled) {
		color: var(--color-text-primary);
		border-color: var(--color-accent);
		background-color: rgba(255, 255, 255, 0.04);
	}

	.icon-btn.danger:hover:not(:disabled) {
		color: var(--color-danger);
		border-color: var(--color-danger);
		background-color: rgba(255, 69, 58, 0.08);
	}

	.icon-btn:disabled {
		opacity: 0.35;
		cursor: not-allowed;
	}

	.action-row {
		display: flex;
		gap: 4px;
	}

	.action-btn {
		flex: 1;
		padding: 4px 10px;
		font-size: 0.6875rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.03em;
		color: var(--color-text-secondary);
		background: transparent;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		cursor: pointer;
		transition: all var(--duration-default) var(--ease-default);
	}

	.action-btn:hover:not(:disabled) {
		color: var(--color-text-primary);
		background-color: rgba(255, 255, 255, 0.04);
	}

	.action-btn:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}

	.action-btn.accent {
		color: var(--color-accent);
		border-color: var(--color-accent);
	}

	.action-btn.accent:hover:not(:disabled) {
		background-color: rgba(10, 132, 255, 0.12);
	}

	.output-section {
		display: flex;
		flex-direction: column;
		border-bottom: 1px solid var(--color-border);
	}

	.output-bar {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 4px 8px;
		border-bottom: 1px solid var(--color-border);
	}

	.output-bar-title {
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
	}

	.output-bar-actions {
		display: flex;
		align-items: center;
		gap: 4px;
		margin-left: auto;
	}

	.running-label {
		font-size: 0.6875rem;
		font-weight: 600;
		color: var(--color-accent);
		text-transform: uppercase;
		letter-spacing: 0.03em;
	}

	.cancel-btn {
		padding: 2px 8px;
		font-size: 0.625rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.03em;
		color: var(--color-danger);
		background: transparent;
		border: 1px solid var(--color-danger);
		border-radius: var(--radius-btn);
		cursor: pointer;
		transition: all var(--duration-default) var(--ease-default);
	}

	.cancel-btn:hover {
		background-color: rgba(255, 69, 58, 0.12);
	}

	.output-console {
		margin: 0;
		padding: 8px;
		font-family: var(--font-mono);
		font-size: 0.6875rem;
		line-height: 1.5;
		color: var(--color-text-primary);
		background-color: var(--color-bg-primary);
		max-height: 300px;
		overflow-y: auto;
		white-space: pre-wrap;
		word-break: break-all;
	}
</style>
