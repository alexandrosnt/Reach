<script lang="ts" module>
	export interface EditorApi {
		/** The selection if there is one, else null. */
		selection(): string | null;
		/** Cursor position in UTF-16 units, matching the backend's offsets. */
		cursor(): number;
		focus(): void;
		setValue(v: string): void;
		insert(text: string): void;
	}
</script>

<script lang="ts">
	/**
	 * The query editor. CodeMirror with the engine's SQL dialect, completion
	 * from the live schema, and the two run keys every SQL tool shares:
	 * Ctrl+Enter for the statement under the cursor (or the selection),
	 * Ctrl+Shift+Enter for everything.
	 */
	import { onMount, onDestroy } from 'svelte';
	import { EditorView, keymap, lineNumbers, highlightActiveLine, highlightActiveLineGutter, drawSelection, placeholder as cmPlaceholder } from '@codemirror/view';
	import { EditorState, Compartment } from '@codemirror/state';
	import { defaultKeymap, history, historyKeymap, indentWithTab } from '@codemirror/commands';
	import { syntaxHighlighting, defaultHighlightStyle, bracketMatching, indentOnInput } from '@codemirror/language';
	import { autocompletion, completionKeymap, closeBrackets, closeBracketsKeymap } from '@codemirror/autocomplete';
	import { searchKeymap, highlightSelectionMatches } from '@codemirror/search';
	import { oneDark } from '@codemirror/theme-one-dark';
	import { sql, PostgreSQL, MySQL, MariaSQL, SQLite, MSSQL, StandardSQL } from '@codemirror/lang-sql';
	import { getSettings } from '$lib/state/settings.svelte';

	interface Props {
		value: string;
		dialect: 'PostgreSQL' | 'MySQL' | 'MariaSQL' | 'SQLite' | 'MSSQL' | null;
		/** table → columns, for completion. */
		schema?: Record<string, string[]>;
		defaultSchema?: string | null;
		placeholder?: string;
		onchange: (v: string) => void;
		onrun: (mode: 'current' | 'all') => void;
		api?: EditorApi;
	}

	let { value, dialect, schema = {}, defaultSchema = null, placeholder = '', onchange, onrun, api = $bindable() }: Props = $props();

	let host: HTMLDivElement;
	let view: EditorView | undefined;
	const lang = new Compartment();

	const dialects = { PostgreSQL, MySQL, MariaSQL, SQLite, MSSQL };

	function language() {
		return sql({
			dialect: dialect ? dialects[dialect] : StandardSQL,
			schema,
			defaultSchema: defaultSchema ?? undefined,
			upperCaseKeywords: true
		});
	}

	const fontSize = getSettings().fontSize ?? 14;
	const theme = EditorView.theme({
		'&': { height: '100%', backgroundColor: 'var(--color-surface-sunken, #1c1c1e)' },
		'.cm-scroller': {
			fontFamily: "var(--font-mono, 'JetBrains Mono', monospace)",
			fontSize: `${fontSize}px`,
			lineHeight: '1.55'
		},
		'.cm-gutters': { backgroundColor: 'transparent', border: 'none', color: 'var(--color-text-tertiary)' },
		'.cm-activeLine': { backgroundColor: 'color-mix(in srgb, var(--color-accent) 6%, transparent)' },
		'.cm-activeLineGutter': { backgroundColor: 'transparent', color: 'var(--color-text-secondary)' },
		'&.cm-focused': { outline: 'none' },
		'.cm-tooltip-autocomplete': { fontFamily: 'var(--font-mono)' }
	});

	onMount(() => {
		const dark = !document.documentElement.classList.contains('light');
		view = new EditorView({
			parent: host,
			state: EditorState.create({
				doc: value,
				extensions: [
					lineNumbers(),
					highlightActiveLineGutter(),
					history(),
					drawSelection(),
					indentOnInput(),
					bracketMatching(),
					closeBrackets(),
					highlightActiveLine(),
					highlightSelectionMatches(),
					autocompletion({ activateOnTyping: true }),
					syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
					cmPlaceholder(placeholder),
					keymap.of([
						{ key: 'Mod-Enter', run: () => (onrun('current'), true) },
						{ key: 'Shift-Mod-Enter', run: () => (onrun('all'), true) },
						{ key: 'F5', run: () => (onrun('all'), true) },
						...closeBracketsKeymap,
						...defaultKeymap,
						...searchKeymap,
						...historyKeymap,
						...completionKeymap,
						indentWithTab
					]),
					lang.of(language()),
					theme,
					...(dark ? [oneDark] : []),
					EditorView.updateListener.of((u) => {
						if (u.docChanged) onchange(u.state.doc.toString());
					})
				]
			})
		});
		api = {
			selection() {
				const s = view!.state.selection.main;
				return s.empty ? null : view!.state.sliceDoc(s.from, s.to);
			},
			cursor() {
				return view!.state.selection.main.head;
			},
			focus() {
				view!.focus();
			},
			setValue(v: string) {
				view!.dispatch({ changes: { from: 0, to: view!.state.doc.length, insert: v } });
			},
			insert(text: string) {
				const s = view!.state.selection.main;
				view!.dispatch({ changes: { from: s.from, to: s.to, insert: text }, selection: { anchor: s.from + text.length } });
				view!.focus();
			}
		};
	});

	// New tables or a new dialect: swap the language, keep the text.
	$effect(() => {
		void schema;
		void dialect;
		void defaultSchema;
		view?.dispatch({ effects: lang.reconfigure(language()) });
	});

	onDestroy(() => view?.destroy());
</script>

<div class="editor" bind:this={host}></div>

<style>
	.editor {
		height: 100%;
		min-height: 0;
		overflow: hidden;
	}

	.editor :global(.cm-editor) {
		height: 100%;
	}
</style>
