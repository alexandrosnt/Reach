<script lang="ts">
	import Modal from '$lib/components/shared/Modal.svelte';
	import Button from '$lib/components/shared/Button.svelte';
	import { generatePlaybook, fixPlaybook } from '$lib/state/ai-chat.svelte';
	import { getAISettings } from '$lib/state/ai.svelte';
	import { playbookSave } from '$lib/ipc/playbook';
	import { addToast } from '$lib/state/toasts.svelte';
	import { t } from '$lib/state/i18n.svelte';
	import { EditorView } from '@codemirror/view';
	import { EditorState } from '@codemirror/state';
	import { basicSetup } from 'codemirror';
	import { yaml } from '@codemirror/lang-yaml';
	import { oneDark } from '@codemirror/theme-one-dark';

	interface Props {
		open: boolean;
		editId?: string;
		initialYaml?: string;
		onclose: () => void;
		onsave?: () => void;
		onrun?: (yaml: string) => void;
	}

	let { open, editId, initialYaml, onclose, onsave, onrun }: Props = $props();

	const defaultTemplate = `name: "My Playbook"
description: "Description here"
variables:
  key: "value"
steps:
  - name: "Step 1"
    command: "echo hello"
    timeout: 30`;

	// Plain variable — NOT reactive. CodeMirror owns the content.
	let yamlContent = defaultTemplate;

	let saving = $state(false);
	let prevOpen = false;
	let editorView: EditorView | undefined;

	const editorTheme = EditorView.theme({
		'&': {
			fontSize: '0.8125rem',
			height: '100%',
		},
		'.cm-content': {
			fontFamily: 'var(--font-mono, monospace)',
			lineHeight: '1.6',
		},
		'.cm-gutters': {
			backgroundColor: 'transparent',
			borderRight: '1px solid var(--color-border)',
			color: 'var(--color-text-secondary)',
			opacity: '0.5',
		},
		'.cm-activeLineGutter': {
			backgroundColor: 'transparent',
			color: 'var(--color-text-primary)',
			opacity: '1',
		},
		'.cm-activeLine': {
			backgroundColor: 'rgba(255, 255, 255, 0.03)',
		},
		'.cm-cursor': {
			borderLeftColor: 'var(--color-accent)',
		},
		'.cm-selectionBackground': {
			backgroundColor: 'rgba(0, 122, 255, 0.2) !important',
		},
		'&.cm-focused': {
			outline: 'none',
		},
		'.cm-scroller': {
			overflow: 'auto',
		},
	});

	function getContent(): string {
		if (editorView) {
			return editorView.state.doc.toString();
		}
		return yamlContent;
	}

	function createEditor(container: HTMLDivElement, content: string): void {
		editorView?.destroy();
		editorView = undefined;
		const state = EditorState.create({
			doc: content,
			extensions: [
				basicSetup,
				yaml(),
				oneDark,
				editorTheme,
				EditorView.lineWrapping,
				EditorView.updateListener.of((update) => {
					if (update.docChanged) {
						yamlContent = update.state.doc.toString();
					}
				}),
			],
		});
		editorView = new EditorView({ state, parent: container });
	}

	function setEditorContent(content: string): void {
		yamlContent = content;
		if (editorView) {
			editorView.dispatch({
				changes: { from: 0, to: editorView.state.doc.length, insert: content },
			});
		}
	}

	function mountEditor(node: HTMLDivElement): { destroy: () => void } {
		const content = initialYaml ?? (editId ? yamlContent : defaultTemplate);
		yamlContent = content;
		createEditor(node, content);
		return {
			destroy() {
				editorView?.destroy();
				editorView = undefined;
			}
		};
	}

	$effect(() => {
		const isOpen = open;
		if (isOpen && !prevOpen) {
			yamlContent = initialYaml ?? (editId ? yamlContent : defaultTemplate);
		}
		prevOpen = isOpen;
	});

	let aiSettings = $derived(getAISettings());
	let aiEnabled = $derived(aiSettings.enabled && aiSettings.apiKey && aiSettings.selectedModel);
	let aiPrompt = $state('');
	let aiGenerating = $state(false);
	let aiFixing = $state(false);
	let showAIPrompt = $state(false);

	async function handleGenerate(): Promise<void> {
		if (!aiPrompt.trim()) return;
		aiGenerating = true;
		try {
			const result = await generatePlaybook(aiPrompt.trim());
			setEditorContent(result);
			showAIPrompt = false;
			aiPrompt = '';
		} catch {
			// Error handled by toast or AI state
		} finally {
			aiGenerating = false;
		}
	}

	async function handleFix(): Promise<void> {
		const content = getContent();
		if (!content.trim()) return;
		aiFixing = true;
		try {
			const result = await fixPlaybook(content);
			setEditorContent(result);
		} catch {
			// Error handled by toast or AI state
		} finally {
			aiFixing = false;
		}
	}

	function handleRun(): void {
		onrun?.(getContent());
	}

	async function handleSave(): Promise<void> {
		const content = getContent();
		if (!content.trim() || saving) return;
		saving = true;
		try {
			await playbookSave(content, editId);
			addToast(t('playbook.saved_toast'), 'success');
			onsave?.();
			onclose();
		} catch (e) {
			addToast(String(e), 'error');
		} finally {
			saving = false;
		}
	}

	function handleClose(): void {
		onclose();
	}
</script>

<Modal {open} onclose={handleClose} title={t('playbook.editor_title')}>
	{#snippet children()}
		<div class="editor-container">
			{#if showAIPrompt}
				<div class="ai-prompt-area">
					<input
						class="ai-prompt-input"
						bind:value={aiPrompt}
						placeholder={t('playbook.generate_prompt')}
						onkeydown={(e) => { if (e.key === 'Enter') handleGenerate(); }}
						disabled={aiGenerating}
					/>
					<div class="ai-prompt-actions">
						<Button variant="ghost" onclick={() => { showAIPrompt = false; aiPrompt = ''; }}>{t('common.cancel')}</Button>
						<Button variant="primary" onclick={handleGenerate} disabled={aiGenerating || !aiPrompt.trim()}>
							{aiGenerating ? t('playbook.generating') : t('playbook.generate')}
						</Button>
					</div>
				</div>
			{/if}
			<span class="editor-label">{t('playbook.yaml_definition')}</span>
			<div class="yaml-editor" use:mountEditor></div>
		</div>
	{/snippet}

	{#snippet actions()}
		<div class="ai-actions">
			<Button variant="ghost" onclick={() => { showAIPrompt = !showAIPrompt; }} disabled={!aiEnabled}>
				<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
					<polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2" />
				</svg>
				{t('playbook.generate_ai')}
			</Button>
			<Button variant="ghost" onclick={handleFix} disabled={!aiEnabled || aiFixing}>
				{aiFixing ? t('playbook.fixing') : t('playbook.fix_ai')}
			</Button>
		</div>
		<div class="spacer"></div>
		<Button variant="ghost" onclick={handleClose}>{t('common.cancel')}</Button>
		<Button variant="secondary" onclick={handleSave} disabled={saving}>{saving ? t('playbook.saving') : t('playbook.save')}</Button>
		<Button variant="primary" onclick={handleRun}>
			<svg width="14" height="14" viewBox="0 0 24 24" fill="none">
				<path
					d="M5 3l14 9-14 9V3z"
					stroke="currentColor"
					stroke-width="2"
					stroke-linecap="round"
					stroke-linejoin="round"
				/>
			</svg>
			{t('playbook.run_short')}
		</Button>
	{/snippet}
</Modal>

<style>
	.editor-container {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.editor-label {
		font-size: 0.6875rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--color-text-secondary);
	}

	.yaml-editor {
		width: 100%;
		min-height: 320px;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		overflow: hidden;
		box-sizing: border-box;
		transition: border-color var(--duration-default) var(--ease-default);
	}

	.yaml-editor:focus-within {
		border-color: var(--color-accent);
	}

	.yaml-editor :global(.cm-editor) {
		height: 100%;
		min-height: 320px;
		background: transparent;
	}

	.ai-prompt-area {
		display: flex;
		flex-direction: column;
		gap: 8px;
		padding: 12px;
		background-color: var(--color-bg-secondary, rgba(255,255,255,0.04));
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
	}

	.ai-prompt-input {
		width: 100%;
		padding: 10px 12px;
		font-size: 0.8125rem;
		color: var(--color-text-primary);
		background-color: var(--color-bg-primary);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		outline: none;
		box-sizing: border-box;
		transition: border-color var(--duration-default) var(--ease-default);
	}

	.ai-prompt-input:focus {
		border-color: var(--color-accent);
	}

	.ai-prompt-input::placeholder {
		color: var(--color-text-secondary);
		opacity: 0.5;
	}

	.ai-prompt-actions {
		display: flex;
		justify-content: flex-end;
		gap: 6px;
	}

	.ai-actions {
		display: flex;
		gap: 6px;
	}

	.spacer {
		flex: 1;
	}
</style>
