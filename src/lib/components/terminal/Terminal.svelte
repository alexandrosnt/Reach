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

	function createTerminal(): Terminal {
		return new Terminal({
			fontFamily: "'JetBrains Mono', 'SF Mono', 'Cascadia Code', monospace",
			fontSize: 14,
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

	function setupInputHandler(term: Terminal): void {
		term.onData((data: string) => {
			const encoded = Array.from(new TextEncoder().encode(data));
			sendData(encoded);
		});

		term.onBinary((data: string) => {
			const bytes = Array.from(data, (c) => c.charCodeAt(0));
			sendData(bytes);
		});

		// Ctrl+C copies when text is selected, otherwise sends SIGINT as normal.
		// Ctrl+V pastes from clipboard.
		term.attachCustomKeyEventHandler((event: KeyboardEvent) => {
			if (event.type !== 'keydown') return true;

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

		loadAddons(term, fit);
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
		const termEl = containerEl;
		function onContextMenu(e: MouseEvent) {
			e.preventDefault();
			navigator.clipboard.readText().then((text) => {
				if (text) term.paste(text);
			});
		}
		termEl.addEventListener('contextmenu', onContextMenu);

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

		return () => {
			unregisterBufferReader(bufferId);
			unlistenData?.();
			unlistenExit?.();
			resizeObserver?.disconnect();
			termEl.removeEventListener('contextmenu', onContextMenu);
			term.dispose();
			terminal = undefined;
			fitAddon = undefined;
		};
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
