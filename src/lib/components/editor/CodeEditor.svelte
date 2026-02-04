<script lang="ts">
	import {
		EditorView,
		keymap,
		lineNumbers,
		highlightActiveLineGutter,
		highlightSpecialChars,
		drawSelection,
		highlightActiveLine,
		rectangularSelection,
		crosshairCursor,
		dropCursor
	} from '@codemirror/view';
	import { EditorState, Compartment } from '@codemirror/state';
	import {
		defaultKeymap,
		history,
		historyKeymap,
		indentWithTab
	} from '@codemirror/commands';
	import {
		syntaxHighlighting,
		defaultHighlightStyle,
		indentOnInput,
		bracketMatching,
		foldGutter,
		foldKeymap
	} from '@codemirror/language';
	import { languages } from '@codemirror/language-data';
	import { oneDark } from '@codemirror/theme-one-dark';
	import {
		closeBrackets,
		closeBracketsKeymap,
		autocompletion,
		completionKeymap
	} from '@codemirror/autocomplete';
	import { searchKeymap, highlightSelectionMatches } from '@codemirror/search';
	import { lintKeymap } from '@codemirror/lint';
	import { LanguageDescription } from '@codemirror/language';

	interface Props {
		content: string;
		language: string;
		onchange: (content: string) => void;
		onsave: () => void;
	}

	let { content, language, onchange, onsave }: Props = $props();

	let editorView: EditorView | undefined;
	let currentLanguage = '';
	const languageCompartment = new Compartment();

	const appleDarkTheme = EditorView.theme(
		{
			'&': {
				backgroundColor: '#1c1c1e',
				height: '100%'
			},
			'.cm-scroller': {
				overflow: 'auto',
				fontFamily: "'JetBrains Mono', 'SF Mono', 'Cascadia Code', monospace",
				fontSize: '13px',
				lineHeight: '1.6'
			},
			'.cm-gutters': {
				backgroundColor: '#0a0a0a',
				color: '#86868b',
				border: 'none'
			},
			'.cm-activeLineGutter': {
				backgroundColor: 'rgba(255, 255, 255, 0.03)'
			},
			'&.cm-focused .cm-selectionBackground, .cm-selectionBackground': {
				backgroundColor: 'rgba(10, 132, 255, 0.25) !important'
			},
			'.cm-activeLine': {
				backgroundColor: 'rgba(255, 255, 255, 0.03)'
			},
			'.cm-cursor': {
				borderLeftColor: '#0a84ff'
			}
		},
		{ dark: true }
	);

	function getLanguageExtension(lang: string) {
		const extMap: Record<string, string> = {
			javascript: 'js',
			typescript: 'ts',
			python: 'py',
			rust: 'rs',
			golang: 'go',
			ruby: 'rb',
			kotlin: 'kt',
			shell: 'sh',
			bash: 'sh',
			zsh: 'sh',
			markdown: 'md',
			yaml: 'yml'
		};

		const ext = extMap[lang.toLowerCase()] ?? lang.toLowerCase();
		return LanguageDescription.matchFilename(languages, `file.${ext}`);
	}

	function loadLanguage(lang: string): void {
		const langDesc = getLanguageExtension(lang);
		if (langDesc) {
			langDesc.load().then((loaded) => {
				if (editorView) {
					editorView.dispatch({
						effects: languageCompartment.reconfigure(loaded)
					});
				}
			});
		} else if (editorView) {
			editorView.dispatch({
				effects: languageCompartment.reconfigure([])
			});
		}
	}

	function mountEditor(node: HTMLDivElement): { destroy: () => void } {
		const state = EditorState.create({
			doc: content,
			extensions: [
				lineNumbers(),
				highlightActiveLineGutter(),
				highlightSpecialChars(),
				history(),
				foldGutter(),
				drawSelection(),
				dropCursor(),
				indentOnInput(),
				syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
				bracketMatching(),
				closeBrackets(),
				autocompletion(),
				rectangularSelection(),
				crosshairCursor(),
				highlightActiveLine(),
				highlightSelectionMatches(),
				appleDarkTheme,
				oneDark,
				languageCompartment.of([]),
				keymap.of([
					...defaultKeymap,
					...searchKeymap,
					...historyKeymap,
					...foldKeymap,
					...completionKeymap,
					...closeBracketsKeymap,
					...lintKeymap,
					indentWithTab,
					{
						key: 'Ctrl-s',
						mac: 'Cmd-s',
						run: () => {
							onsave();
							return true;
						}
					}
				]),
				EditorView.updateListener.of((update) => {
					if (update.docChanged) {
						onchange(update.state.doc.toString());
					}
				})
			]
		});

		editorView = new EditorView({ state, parent: node });
		currentLanguage = language;
		loadLanguage(language);

		return {
			destroy() {
				editorView?.destroy();
				editorView = undefined;
			}
		};
	}

	// React to language prop changes only
	$effect(() => {
		if (!editorView || language === currentLanguage) return;
		currentLanguage = language;
		loadLanguage(language);
	});
</script>

<div use:mountEditor class="code-editor-wrapper"></div>

<style>
	.code-editor-wrapper {
		width: 100%;
		height: 100%;
		overflow: hidden;
	}
</style>
