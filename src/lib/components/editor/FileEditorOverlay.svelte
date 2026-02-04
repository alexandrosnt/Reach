<script lang="ts">
  import CodeEditor from '$lib/components/editor/CodeEditor.svelte';
  import {
    getEditorFile,
    closeEditor,
    updateEditorContent,
    updateEditorLanguage,
    isEditorDirty,
    markSaved
  } from '$lib/state/editor.svelte';
  import { sftpWriteFile } from '$lib/ipc/sftp';
  import { addToast } from '$lib/state/toasts.svelte';

  const availableLanguages = [
    'text', 'javascript', 'typescript', 'python', 'rust', 'go', 'java', 'c', 'cpp',
    'ruby', 'php', 'swift', 'kotlin', 'shell', 'css', 'scss', 'less', 'html', 'xml',
    'json', 'yaml', 'toml', 'markdown', 'sql', 'lua', 'r', 'dockerfile'
  ];

  let file = $derived(getEditorFile());
  let dirty = $derived(isEditorDirty());
  let saving = $state(false);

  async function handleSave() {
    if (!file) return;

    saving = true;
    try {
      await sftpWriteFile(file.connectionId, file.path, file.content);
      markSaved();
      addToast('File saved', 'success');
    } catch (err: unknown) {
      const message = err instanceof Error ? err.message : String(err);
      addToast(message, 'error');
    } finally {
      saving = false;
    }
  }

  function handleClose() {
    if (dirty) {
      const confirmed = window.confirm('You have unsaved changes. Close without saving?');
      if (!confirmed) return;
    }
    closeEditor();
  }

  $effect(() => {
    function onKeydown(e: KeyboardEvent) {
      if (e.key === 'Escape') {
        handleClose();
      }
    }

    window.addEventListener('keydown', onKeydown);

    return () => {
      window.removeEventListener('keydown', onKeydown);
    };
  });
</script>

{#if file}
  <div class="editor-overlay">
    <div class="editor-toolbar glass">
      <button class="close-btn" onclick={handleClose} aria-label="Close editor">
        <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
          <path d="M1 1l12 12M13 1L1 13" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
        </svg>
      </button>

      <span class="editor-filename">{file.filename}</span>

      {#if dirty}
        <span class="dirty-dot"></span>
      {/if}

      <div class="toolbar-spacer"></div>

      <select
        value={file.language}
        onchange={(e) => updateEditorLanguage(e.currentTarget.value)}
      >
        {#each availableLanguages as lang (lang)}
          <option value={lang}>{lang}</option>
        {/each}
      </select>

      <button class="save-btn" onclick={handleSave} disabled={saving || !dirty}>
        {saving ? 'Saving...' : 'Save'}
      </button>

      <span class="shortcut-hint">Ctrl+S</span>
    </div>

    <div class="editor-content">
      <CodeEditor
        content={file.content}
        language={file.language}
        onchange={updateEditorContent}
        onsave={handleSave}
      />
    </div>
  </div>
{/if}

<style>
  .editor-overlay {
    position: fixed;
    inset: 0;
    z-index: 200;
    display: flex;
    flex-direction: column;
    background: var(--color-bg-primary, #0a0a0a);
  }

  .editor-toolbar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .close-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    border: none;
    border-radius: 8px;
    background: transparent;
    color: var(--color-text-primary);
    cursor: pointer;
  }

  .close-btn:hover {
    background: rgba(255, 255, 255, 0.06);
  }

  .editor-filename {
    font-size: 0.8125rem;
    color: var(--color-text-primary);
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .dirty-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--color-warning, #ffd60a);
    flex-shrink: 0;
  }

  .toolbar-spacer {
    flex: 1;
  }

  select {
    background: var(--color-bg-elevated);
    border: 1px solid var(--color-border);
    color: var(--color-text-primary);
    border-radius: var(--radius-btn);
    padding: 4px 8px;
    font-size: 0.75rem;
    font-family: var(--font-sans);
  }

  .save-btn {
    background: var(--color-accent);
    color: white;
    border: none;
    border-radius: var(--radius-btn);
    padding: 6px 14px;
    font-size: 0.75rem;
    font-weight: 500;
    cursor: pointer;
  }

  .save-btn:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .shortcut-hint {
    font-size: 0.625rem;
    color: var(--color-text-secondary);
    opacity: 0.6;
  }

  .editor-content {
    flex: 1;
    overflow: hidden;
    min-height: 0;
  }
</style>
