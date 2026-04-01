<script lang="ts">
	import { Terminal } from '@xterm/xterm';
	import { FitAddon } from '@xterm/addon-fit';
	import { WebglAddon } from '@xterm/addon-webgl';
	import { WebLinksAddon } from '@xterm/addon-web-links';
	import { Unicode11Addon } from '@xterm/addon-unicode11';
	import '@xterm/xterm/css/xterm.css';
	import { listen, type UnlistenFn } from '@tauri-apps/api/event';
	import { ptyWrite, ptyResize } from '$lib/ipc/pty';
	import { sshSend, sshResize } from '$lib/ipc/ssh';
	import { registerBufferReader, unregisterBufferReader } from '$lib/state/terminal-buffer.svelte';
	import { getSettings, updateSetting } from '$lib/state/settings.svelte';
	import { trieMatch } from '$lib/state/snippets.svelte';

	interface Props {
		ptyId: string;
		type: 'local' | 'ssh';
		connectionId?: string;
		active: boolean;
		onTitleChange?: (title: string) => void;
	}

	let { ptyId, type: termType, connectionId, active, onTitleChange }: Props = $props();

	let containerEl: HTMLDivElement | undefined = $state();
	let terminal: Terminal | undefined = $state();
	let fitAddon: FitAddon | undefined = $state();

	let unlistenData: UnlistenFn | undefined;
	let unlistenExit: UnlistenFn | undefined;
	let resizeObserver: ResizeObserver | undefined;

	// Snippet autocomplete (Trie-based, ghost text via xterm Decoration API)
	let inputBuffer = $state('');
	let suggestion = $state<{ command: string; name: string; ghost: string } | null>(null);
	let ghostDecoration: { dispose: () => void } | undefined;
	let ghostMarker: { dispose: () => void } | undefined;

	let ghostEl: HTMLDivElement | undefined;

	function showGhostDecoration(term: Terminal, ghost: string): void {
		clearGhostDecoration();
		if (!term.element) return;

		const cursorX = term.buffer.active.cursorX;
		const cursorY = term.buffer.active.cursorY - term.buffer.active.viewportY;

		// Get exact cell dimensions and canvas padding from xterm internals
		let cellW: number, cellH: number, padLeft = 0, padTop = 0;
		try {
			const dims = (term as any)._core._renderService.dimensions;
			cellW = dims.css.cell.width;
			cellH = dims.css.cell.height;
			padLeft = dims.css.canvas.left ?? 0;
			padTop = dims.css.canvas.top ?? 0;
		} catch {
			const screen = term.element.querySelector('.xterm-screen');
			if (!screen) return;
			cellW = screen.clientWidth / term.cols;
			cellH = screen.clientHeight / term.rows;
		}

		const screen = term.element.querySelector('.xterm-screen') as HTMLElement;
		if (!screen) return;

		const el = document.createElement('div');
		el.textContent = ghost;
		el.style.position = 'absolute';
		el.style.left = `${screen.offsetLeft + padLeft + cursorX * cellW}px`;
		el.style.top = `${screen.offsetTop + padTop + cursorY * cellH}px`;
		el.style.height = `${cellH}px`;
		el.style.lineHeight = `${cellH}px`;
		el.style.fontSize = `${term.options.fontSize ?? 14}px`;
		el.style.fontFamily = term.options.fontFamily || 'monospace';
		el.style.letterSpacing = '0px';
		el.style.color = 'rgba(255, 255, 255, 0.3)';
		el.style.pointerEvents = 'none';
		el.style.whiteSpace = 'pre';
		el.style.zIndex = '10';
		el.style.overflow = 'visible';

		term.element.appendChild(el);
		ghostEl = el;
	}

	function clearGhostDecoration(): void {
		if (ghostEl) {
			ghostEl.remove();
			ghostEl = undefined;
		}
	}

	function createTerminal(): Terminal {
		const appSettings = getSettings();
		return new Terminal({
			fontFamily: appSettings.fontFamily || 'monospace',
			fontSize: appSettings.fontSize ?? 14,
			cursorBlink: true,
			cursorStyle: 'bar',
			scrollback: 10000,
			allowProposedApi: true,
			theme: {
				background: '#0a0a0a',
				foreground: '#f5f5f7',
				cursor: '#0a84ff',
				cursorAccent: '#0a0a0a',
				selectionBackground: 'rgba(10, 132, 255, 0.3)',
				selectionForeground: '#f5f5f7',
				black: '#1d1f21',
				red: '#ff453a',
				green: '#30d158',
				yellow: '#ffd60a',
				blue: '#0a84ff',
				magenta: '#bf5af2',
				cyan: '#64d2ff',
				white: '#f5f5f7',
				brightBlack: '#6e6e73',
				brightRed: '#ff6961',
				brightGreen: '#4ae06a',
				brightYellow: '#ffe566',
				brightBlue: '#409cff',
				brightMagenta: '#da8fff',
				brightCyan: '#8be8ff',
				brightWhite: '#ffffff'
			}
		});
	}

	function loadAddons(term: Terminal, fit: FitAddon): void {
		term.loadAddon(fit);
		term.loadAddon(new WebLinksAddon());

		const unicode11 = new Unicode11Addon();
		term.loadAddon(unicode11);
		term.unicode.activeVersion = '11';

		try {
			const webgl = new WebglAddon();
			webgl.onContextLoss(() => {
				webgl.dispose();
			});
			term.loadAddon(webgl);
		} catch {
			console.warn('[Terminal] WebGL addon unavailable, using canvas renderer');
		}
	}

	function sendData(data: number[]): void {
		if (termType === 'ssh' && connectionId) {
			sshSend(connectionId, data).catch((err) => {
				console.error('[Terminal] Failed to write to SSH:', err);
			});
		} else {
			ptyWrite(ptyId, data).catch((err) => {
				console.error('[Terminal] Failed to write to PTY:', err);
			});
		}
	}

	function sendResize(cols: number, rows: number): void {
		if (termType === 'ssh' && connectionId) {
			sshResize(connectionId, cols, rows).catch((err) => {
				console.error('[Terminal] Failed to resize SSH:', err);
			});
		} else {
			ptyResize(ptyId, cols, rows).catch((err) => {
				console.error('[Terminal] Failed to resize PTY:', err);
			});
		}
	}

	let suggestionTimer: ReturnType<typeof setTimeout> | undefined;
	let lastCursorX = -1;

	function updateSuggestion(): void {
		clearTimeout(suggestionTimer);
		if (inputBuffer.length < 2 || !terminal) {
			suggestion = null;
			clearGhostDecoration();
			return;
		}
		const match = trieMatch(inputBuffer);
		if (match) {
			const ghost = match.command.slice(inputBuffer.length);
			suggestion = { command: match.command, name: match.name, ghost };
			const term = terminal;
			// Wait for cursor to move (echo arrived) instead of fixed delay
			lastCursorX = term.buffer.active.cursorX;
			let attempts = 0;
			const poll = () => {
				attempts++;
				const nowX = term.buffer.active.cursorX;
				if (nowX !== lastCursorX || attempts > 20) {
					// Cursor moved = echo arrived, or max 500ms reached
					clearGhostDecoration();
					showGhostDecoration(term, ghost);
				} else {
					suggestionTimer = setTimeout(poll, 25);
				}
			};
			suggestionTimer = setTimeout(poll, 25);
		} else {
			suggestion = null;
			clearGhostDecoration();
		}
	}

	function acceptSuggestion(): boolean {
		if (!suggestion) return false;
		const remaining = suggestion.command.slice(inputBuffer.length);
		if (remaining.length > 0) {
			const encoded = Array.from(new TextEncoder().encode(remaining));
			sendData(encoded);
		}
		inputBuffer = '';
		suggestion = null;
		clearGhostDecoration();
		return true;
	}

	function setupInputHandler(term: Terminal): void {
		term.onData((data: string) => {
			// Track input buffer for snippet autocomplete
			if (data === '\r' || data === '\n') {
				inputBuffer = '';
				suggestion = null;
				clearGhostDecoration();
			} else if (data === '\x7f' || data === '\b') {
				inputBuffer = inputBuffer.slice(0, -1);
				updateSuggestion();
			} else if (data === '\x03') {
				inputBuffer = '';
				suggestion = null;
				clearGhostDecoration();
			} else if (data.length === 1 && data.charCodeAt(0) >= 32) {
				inputBuffer += data;
				updateSuggestion();
			} else {
				inputBuffer = '';
				suggestion = null;
				clearGhostDecoration();
			}

			const encoded = Array.from(new TextEncoder().encode(data));
			sendData(encoded);
		});

		term.onBinary((data: string) => {
			const bytes = Array.from(data, (c) => c.charCodeAt(0));
			sendData(bytes);
		});

		// Ctrl+C copies when text is selected, otherwise sends SIGINT as normal.
		// Ctrl+V pastes from clipboard.
		// Tab accepts snippet suggestion.
		term.attachCustomKeyEventHandler((event: KeyboardEvent) => {
			if (event.type !== 'keydown') return true;

			// Tab accepts snippet autocomplete
			if (event.key === 'Tab' && suggestion) {
				event.preventDefault();
				acceptSuggestion();
				return false;
			}

			// Escape dismisses suggestion
			if (event.key === 'Escape' && suggestion) {
				suggestion = null;
				inputBuffer = '';
				clearGhostDecoration();
				return true;
			}

			if (event.ctrlKey && event.key === 'c' && term.hasSelection()) {
				navigator.clipboard.writeText(term.getSelection());
				term.clearSelection();
				return false;
			}

			if (event.ctrlKey && event.key === 'v') {
				event.preventDefault();
				navigator.clipboard.readText().then((text) => {
					if (text) term.paste(text);
				});
				return false;
			}

			return true;
		});
	}

	async function setupEventListeners(term: Terminal): Promise<void> {
		const eventId = termType === 'ssh' && connectionId ? connectionId : ptyId;
		const dataEventName = termType === 'ssh' ? `ssh-data-${eventId}` : `pty-data-${eventId}`;
		const exitEventName = termType === 'ssh' ? `ssh-exit-${eventId}` : `pty-exit-${eventId}`;

		unlistenData = await listen<number[]>(dataEventName, (event) => {
			const payload = event.payload;
			if (payload instanceof Uint8Array) {
				term.write(payload);
			} else if (Array.isArray(payload)) {
				term.write(new Uint8Array(payload));
			} else if (typeof payload === 'string') {
				term.write(payload);
			}
		});

		unlistenExit = await listen<{ code: number }>(exitEventName, (event) => {
			const code = event.payload?.code ?? 0;
			term.write(`\r\n\x1b[90m[Process exited with code ${code}]\x1b[0m\r\n`);
		});
	}

	function setupResizeObserver(term: Terminal, fit: FitAddon, el: HTMLDivElement): void {
		let resizeTimeout: ReturnType<typeof setTimeout> | undefined;

		resizeObserver = new ResizeObserver(() => {
			if (resizeTimeout) clearTimeout(resizeTimeout);
			resizeTimeout = setTimeout(() => {
				if (!term.element) return;
				try {
					fit.fit();
					sendResize(term.cols, term.rows);
				} catch (err) {
					console.error('[Terminal] Fit error:', err);
				}
			}, 50);
		});

		resizeObserver.observe(el);
	}

	$effect(() => {
		if (!containerEl) return;

		const term = createTerminal();
		const fit = new FitAddon();
		const font = getSettings().fontFamily || 'monospace';

		loadAddons(term, fit);

		// Wait for font to load before opening (canvas needs the font ready)
		const fontSize = getSettings().fontSize ?? 14;
		Promise.all([
			document.fonts.load(`${fontSize}px "${font}"`),
			document.fonts.load(`bold ${fontSize}px "${font}"`)
		]).catch(() => {}).finally(() => {
			if (!containerEl) return;
			term.open(containerEl);

			requestAnimationFrame(() => {
				try {
					fit.fit();
					sendResize(term.cols, term.rows);
				} catch (err) {
					console.error('[Terminal] Initial fit error:', err);
				}
			});

			setupInputHandler(term);
			setupEventListeners(term);
			setupResizeObserver(term, fit, containerEl);

			// Detect user changes (e.g. sudo su -) via OSC 2 terminal title updates
			term.onTitleChange((title: string) => {
				onTitleChange?.(title);
			});

			// Right-click pastes from clipboard
			const termEl = containerEl!;
			function onContextMenu(e: MouseEvent) {
				e.preventDefault();
				navigator.clipboard.readText().then((text) => {
					if (text) term.paste(text);
				});
			}
			termEl.addEventListener('contextmenu', onContextMenu);

			// Ctrl+Wheel zooms terminal font size
			function onWheel(e: WheelEvent) {
				if (!e.ctrlKey) return;
				e.preventDefault();
				const current = term.options.fontSize ?? 14;
				const next = e.deltaY < 0 ? Math.min(current + 1, 32) : Math.max(current - 1, 8);
				if (next !== current) {
					term.options.fontSize = next;
					term.clearTextureAtlas();
					fit.fit();
					sendResize(term.cols, term.rows);
					updateSetting('fontSize', next);
				}
			}
			termEl.addEventListener('wheel', onWheel, { passive: false });

			// Click to copy: if text is selected, clicking copies it
			function onClick(e: MouseEvent) {
				if (term.hasSelection()) {
					navigator.clipboard.writeText(term.getSelection());
					term.clearSelection();
				}
			}
			termEl.addEventListener('click', onClick);

			terminal = term;
			fitAddon = fit;

			const bufferId = termType === 'ssh' && connectionId ? connectionId : ptyId;
			registerBufferReader(bufferId, {
				read: (startLine?: number, maxLines?: number) => {
					const buf = term.buffer.active;
					const start = startLine ?? Math.max(0, buf.length - (maxLines ?? 50));
					const end = maxLines && startLine != null ? Math.min(buf.length, start + maxLines) : buf.length;
					const lines: string[] = [];
					for (let i = start; i < end; i++) {
						const line = buf.getLine(i);
						if (line) lines.push(line.translateToString(true));
					}
					return lines.join('\n').trim();
				},
				lineCount: () => term.buffer.active.length
			});
		});

		return () => {
			const bufferId = termType === 'ssh' && connectionId ? connectionId : ptyId;
			unregisterBufferReader(bufferId);
			unlistenData?.();
			unlistenExit?.();
			resizeObserver?.disconnect();
			term.dispose();
			terminal = undefined;
			fitAddon = undefined;
		};
	});

	// Live font family updates from settings (size is changed via Ctrl+Wheel directly)
	const appSettings = getSettings();
	$effect(() => {
		const family = appSettings.fontFamily;
		const term = terminal;
		const fit = fitAddon;
		if (!term || !fit || !family) return;
		if (term.options.fontFamily === family) return;

		const size = term.options.fontSize ?? 14;
		Promise.all([
			document.fonts.load(`${size}px "${family}"`),
			document.fonts.load(`bold ${size}px "${family}"`)
		]).catch(() => {}).finally(() => {
			term.options.fontFamily = family;
			term.clearTextureAtlas();
			fit.fit();
			sendResize(term.cols, term.rows);
		});
	});

</script>

<div
	class="terminal-wrapper"
	style:display={active ? 'block' : 'none'}
>
	<div bind:this={containerEl} class="terminal-container"></div>
</div>

<style>
	.terminal-wrapper {
		width: 100%;
		height: 100%;
		background: var(--bg-primary, #0a0a0a);
		position: relative;
	}

	.terminal-container {
		width: 100%;
		height: 100%;
	}


	.terminal-container :global(.xterm) {
		height: 100%;
		padding: 4px;
	}

	.terminal-container :global(.xterm-viewport) {
		scrollbar-width: thin;
		scrollbar-color: rgba(255, 255, 255, 0.15) transparent;
	}

	.terminal-container :global(.xterm-viewport::-webkit-scrollbar) {
		width: 6px;
	}

	.terminal-container :global(.xterm-viewport::-webkit-scrollbar-track) {
		background: transparent;
	}

	.terminal-container :global(.xterm-viewport::-webkit-scrollbar-thumb) {
		background-color: rgba(255, 255, 255, 0.15);
		border-radius: 3px;
	}

	.terminal-container :global(.xterm-viewport::-webkit-scrollbar-thumb:hover) {
		background-color: rgba(255, 255, 255, 0.25);
	}
</style>
