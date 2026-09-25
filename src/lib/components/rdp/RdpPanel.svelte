<script lang="ts">
	/**
	 * A remote desktop in a tab.
	 *
	 * The canvas is the framebuffer at the server's size; CSS scales it to fit
	 * the panel without stretching, and every pointer coordinate is mapped back
	 * through that scale before it is sent. Frames arrive as raw RGBA regions
	 * and are painted with putImageData at the region's offset — no full-frame
	 * redraw unless the server sent one.
	 *
	 * Keyboard events are translated from KeyboardEvent.code to PC/AT set-1
	 * scancodes, which is what RDP wants: the key's position, not the character
	 * it produced. That is why Ctrl+C reaches the remote as Ctrl+C rather than
	 * being eaten here as a copy. Characters with no scancode on this keyboard
	 * (an IME commit, a pasted glyph) go as Unicode instead.
	 */
	import { onDestroy, onMount } from 'svelte';
	import { listen, type UnlistenFn } from '@tauri-apps/api/event';
	import {
		FRAME_FULL,
		FRAME_HEADER,
		FRAME_POINTER,
		FRAME_POINTER_DEFAULT,
		FRAME_POINTER_HIDDEN,
		FRAME_POINTER_LEN,
		FRAME_POINTER_SHAPE,
		FRAME_POINTER_SHAPE_HEADER,
		FRAME_REGION,
		rdpAck,
		rdpClipboardSync,
		rdpConnect,
		rdpDisconnect,
		rdpKey,
		rdpMouse,
		rdpResize,
		rdpUnicode,
		type RdpConnectParams,
		type RdpStatus,
	} from '$lib/ipc/rdp';
	import { t } from '$lib/state/i18n.svelte';

	interface Props {
		id: string;
		params: RdpConnectParams;
		active: boolean;
	}

	let { id, params, active }: Props = $props();

	let host: HTMLDivElement;
	let canvas: HTMLCanvasElement;
	let ctx: CanvasRenderingContext2D | null = null;

	type Phase = 'connecting' | 'connected' | 'closed' | 'error';
	let phase = $state<Phase>('connecting');
	let message = $state('');

	let unlisten: UnlistenFn | null = null;
	let sizeObserver: ResizeObserver | null = null;
	let resizeTimer: ReturnType<typeof setTimeout> | null = null;

	/* --- frames ------------------------------------------------------------ */

	/**
	 * One message from the backend: a pointer position and any number of
	 * regions, back to back. Painted in order, then acknowledged once — the
	 * backend holds the next message until it hears this, which is what keeps
	 * a busy screen from piling up messages faster than they can be drawn.
	 */
	function paint(buf: ArrayBuffer): void {
		const view = new DataView(buf);
		let at = 0;

		while (at < buf.byteLength) {
			const kind = view.getUint8(at);

			if (kind === FRAME_POINTER) {
				// The server moved the cursor. The local pointer is not
				// warped — that is not something a webview may do — so the
				// position is noted and nothing more.
				at += FRAME_POINTER_LEN;
				continue;
			}
			if (kind === FRAME_POINTER_HIDDEN) {
				canvas.style.cursor = 'none';
				at += 1;
				continue;
			}
			if (kind === FRAME_POINTER_DEFAULT) {
				canvas.style.cursor = 'default';
				at += 1;
				continue;
			}
			if (kind === FRAME_POINTER_SHAPE) {
				if (at + FRAME_POINTER_SHAPE_HEADER > buf.byteLength) break;
				const w = view.getUint16(at + 1, true);
				const h = view.getUint16(at + 3, true);
				const hx = view.getUint16(at + 5, true);
				const hy = view.getUint16(at + 7, true);
				const bytes = w * h * 4;
				at += FRAME_POINTER_SHAPE_HEADER;
				if (w === 0 || h === 0 || at + bytes > buf.byteLength) break;
				setCursorShape(new Uint8ClampedArray(buf, at, bytes), w, h, hx, hy);
				at += bytes;
				continue;
			}
			if (kind !== FRAME_REGION && kind !== FRAME_FULL) break;
			if (at + FRAME_HEADER > buf.byteLength) break;

			const x = view.getUint16(at + 1, true);
			const y = view.getUint16(at + 3, true);
			const w = view.getUint16(at + 5, true);
			const h = view.getUint16(at + 7, true);
			const fbW = view.getUint16(at + 9, true);
			const fbH = view.getUint16(at + 11, true);
			const bytes = w * h * 4;
			at += FRAME_HEADER;
			if (w === 0 || h === 0 || at + bytes > buf.byteLength) break;

			// The framebuffer grew, shrank, or was replaced: size the canvas
			// to it. Resizing clears a canvas, so only when it has to be.
			if (canvas.width !== fbW || canvas.height !== fbH) {
				canvas.width = fbW;
				canvas.height = fbH;
				ctx = canvas.getContext('2d', { alpha: false });
			}
			if (!ctx) ctx = canvas.getContext('2d', { alpha: false });
			if (ctx) {
				const pixels = new Uint8ClampedArray(buf, at, bytes);
				ctx.putImageData(new ImageData(pixels, w, h), x, y);
			}
			at += bytes;
		}

		void rdpAck(id).catch(() => {});
	}

	/**
	 * Show the server's cursor as the canvas's CSS cursor. Straight RGBA in,
	 * one small canvas to turn it into a PNG data URL, and the hotspot goes
	 * along so the click lands where the arrow's tip is. The mouse then moves
	 * at the speed of the local mouse, whatever the picture is doing.
	 */
	let cursorCanvas: HTMLCanvasElement | null = null;

	function setCursorShape(rgba: Uint8ClampedArray, w: number, h: number, hx: number, hy: number): void {
		cursorCanvas ??= document.createElement('canvas');
		cursorCanvas.width = w;
		cursorCanvas.height = h;
		const cctx = cursorCanvas.getContext('2d');
		if (!cctx) return;
		// A view into the frame message; ImageData wants its own buffer.
		cctx.putImageData(new ImageData(new Uint8ClampedArray(rgba), w, h), 0, 0);
		const url = cursorCanvas.toDataURL('image/png');
		// Hotspots are clamped by the engine to the image; `auto` is the
		// fallback for an image the engine will not use (over 128px).
		canvas.style.cursor = `url(${url}) ${Math.min(hx, w - 1)} ${Math.min(hy, h - 1)}, auto`;
	}

	/* --- pointer ----------------------------------------------------------- */

	/** Where on the remote desktop a pointer event landed. */
	function remotePoint(e: MouseEvent): { x: number; y: number } | null {
		const r = canvas.getBoundingClientRect();
		if (r.width === 0 || r.height === 0) return null;
		const x = Math.round(((e.clientX - r.left) / r.width) * canvas.width);
		const y = Math.round(((e.clientY - r.top) / r.height) * canvas.height);
		return {
			x: Math.max(0, Math.min(canvas.width - 1, x)),
			y: Math.max(0, Math.min(canvas.height - 1, y)),
		};
	}

	// Moves are coalesced to one per frame: a fast mouse produces hundreds a
	// second and the server only needs the newest.
	let pendingMove: { x: number; y: number } | null = null;
	let moveScheduled = false;

	function onMove(e: MouseEvent): void {
		const p = remotePoint(e);
		if (!p) return;
		pendingMove = p;
		if (moveScheduled) return;
		moveScheduled = true;
		requestAnimationFrame(() => {
			moveScheduled = false;
			if (!pendingMove) return;
			const { x, y } = pendingMove;
			pendingMove = null;
			void rdpMouse(id, x, y, 'move');
		});
	}

	function onButton(e: MouseEvent, action: 'down' | 'up'): void {
		if (e.button > 2) return;
		const p = remotePoint(e);
		if (!p) return;
		if (action === 'down') canvas.focus();
		e.preventDefault();
		void rdpMouse(id, p.x, p.y, action, e.button);
	}

	function onWheel(e: WheelEvent): void {
		const p = remotePoint(e);
		if (!p || e.deltaY === 0) return;
		e.preventDefault();
		// A browser notch is deltaY ≈ +100 downwards; an RDP notch is 120 and
		// positive means towards the user. One notch per event keeps trackpads
		// from firing a flood of tiny steps.
		void rdpMouse(id, p.x, p.y, 'wheel', 0, e.deltaY < 0 ? 120 : -120);
	}

	/* --- keyboard ---------------------------------------------------------- */

	/**
	 * KeyboardEvent.code → PC/AT set-1 scancode. `[code, extended]`.
	 *
	 * The extended (E0-prefixed) keys are the ones that were added after the
	 * original 83-key layout and reuse a scancode from it: right Ctrl shares
	 * 0x1D with left Ctrl, the arrow cluster shares codes with the numpad.
	 */
	const SCANCODES: Record<string, [number, boolean]> = {
		Escape: [0x01, false],
		Digit1: [0x02, false], Digit2: [0x03, false], Digit3: [0x04, false], Digit4: [0x05, false],
		Digit5: [0x06, false], Digit6: [0x07, false], Digit7: [0x08, false], Digit8: [0x09, false],
		Digit9: [0x0a, false], Digit0: [0x0b, false], Minus: [0x0c, false], Equal: [0x0d, false],
		Backspace: [0x0e, false], Tab: [0x0f, false],
		KeyQ: [0x10, false], KeyW: [0x11, false], KeyE: [0x12, false], KeyR: [0x13, false],
		KeyT: [0x14, false], KeyY: [0x15, false], KeyU: [0x16, false], KeyI: [0x17, false],
		KeyO: [0x18, false], KeyP: [0x19, false], BracketLeft: [0x1a, false], BracketRight: [0x1b, false],
		Enter: [0x1c, false], ControlLeft: [0x1d, false],
		KeyA: [0x1e, false], KeyS: [0x1f, false], KeyD: [0x20, false], KeyF: [0x21, false],
		KeyG: [0x22, false], KeyH: [0x23, false], KeyJ: [0x24, false], KeyK: [0x25, false],
		KeyL: [0x26, false], Semicolon: [0x27, false], Quote: [0x28, false], Backquote: [0x29, false],
		ShiftLeft: [0x2a, false], Backslash: [0x2b, false],
		KeyZ: [0x2c, false], KeyX: [0x2d, false], KeyC: [0x2e, false], KeyV: [0x2f, false],
		KeyB: [0x30, false], KeyN: [0x31, false], KeyM: [0x32, false], Comma: [0x33, false],
		Period: [0x34, false], Slash: [0x35, false], ShiftRight: [0x36, false],
		NumpadMultiply: [0x37, false], AltLeft: [0x38, false], Space: [0x39, false], CapsLock: [0x3a, false],
		F1: [0x3b, false], F2: [0x3c, false], F3: [0x3d, false], F4: [0x3e, false], F5: [0x3f, false],
		F6: [0x40, false], F7: [0x41, false], F8: [0x42, false], F9: [0x43, false], F10: [0x44, false],
		NumLock: [0x45, false], ScrollLock: [0x46, false],
		Numpad7: [0x47, false], Numpad8: [0x48, false], Numpad9: [0x49, false], NumpadSubtract: [0x4a, false],
		Numpad4: [0x4b, false], Numpad5: [0x4c, false], Numpad6: [0x4d, false], NumpadAdd: [0x4e, false],
		Numpad1: [0x4f, false], Numpad2: [0x50, false], Numpad3: [0x51, false],
		Numpad0: [0x52, false], NumpadDecimal: [0x53, false], IntlBackslash: [0x56, false],
		F11: [0x57, false], F12: [0x58, false],
		// Extended.
		NumpadEnter: [0x1c, true], ControlRight: [0x1d, true], NumpadDivide: [0x35, true],
		PrintScreen: [0x37, true], AltRight: [0x38, true],
		Home: [0x47, true], ArrowUp: [0x48, true], PageUp: [0x49, true],
		ArrowLeft: [0x4b, true], ArrowRight: [0x4d, true],
		End: [0x4f, true], ArrowDown: [0x50, true], PageDown: [0x51, true],
		Insert: [0x52, true], Delete: [0x53, true],
		MetaLeft: [0x5b, true], MetaRight: [0x5c, true], ContextMenu: [0x5d, true],
	};

	/** Keys currently down, so a lost focus can release them. A modifier stuck
	 *  down on the remote is the classic remote-desktop misery. */
	const held = new Set<string>();

	function onKeyDown(e: KeyboardEvent): void {
		if (phase !== 'connected') return;
		// The remote repeats a held key itself; forwarding the browser's
		// repeats as well would double the rate.
		if (e.repeat) return;

		const entry = SCANCODES[e.code];
		if (entry) {
			e.preventDefault();
			held.add(e.code);
			void rdpKey(id, entry[0], entry[1], false);
			return;
		}
		// No position for this key here (a dead-key result, an IME commit):
		// send what it produced.
		if (e.key.length === 1 && !e.ctrlKey && !e.altKey && !e.metaKey) {
			e.preventDefault();
			const code = e.key.charCodeAt(0);
			void rdpUnicode(id, code, false).then(() => rdpUnicode(id, code, true));
		}
	}

	function onKeyUp(e: KeyboardEvent): void {
		if (phase !== 'connected') return;
		const entry = SCANCODES[e.code];
		if (!entry) return;
		e.preventDefault();
		held.delete(e.code);
		void rdpKey(id, entry[0], entry[1], true);
	}

	function releaseAll(): void {
		for (const code of held) {
			const entry = SCANCODES[code];
			if (entry) void rdpKey(id, entry[0], entry[1], true);
		}
		held.clear();
	}

	/* --- lifecycle --------------------------------------------------------- */

	/** The largest even size the panel can show, for the server to render at. */
	function fitSize(): { width: number; height: number } {
		const r = host.getBoundingClientRect();
		const even = (v: number) => Math.max(200, Math.floor(v) & ~1);
		return { width: even(r.width || 1024), height: even(r.height || 768) };
	}

	onMount(async () => {
		const size = fitSize();

		unlisten = await listen<RdpStatus>(`rdp-status-${id}`, (event) => {
			const s = event.payload;
			if (s.state === 'connected' || s.state === 'loggedIn') {
				phase = 'connected';
				message = '';
			} else if (s.state === 'closed') {
				phase = 'closed';
				message = s.reason;
				releaseAll();
			} else if (s.state === 'error') {
				phase = 'error';
				message = s.message;
			}
		});

		try {
			await rdpConnect({ ...params, ...size }, paint);
		} catch (err) {
			phase = 'error';
			message = String(err);
			return;
		}

		// Ask the server to follow the panel when it is resized. Debounced:
		// a drag fires this continuously and each one is a renegotiation.
		sizeObserver = new ResizeObserver(() => {
			if (resizeTimer) clearTimeout(resizeTimer);
			resizeTimer = setTimeout(() => {
				if (phase !== 'connected') return;
				const s = fitSize();
				// The size the desktop already has is not a resize. Asking for
				// it anyway leaves a request the server never answers.
				if (s.width === canvas.width && s.height === canvas.height) return;
				void rdpResize(id, s.width, s.height);
			}, 300);
		});
		sizeObserver.observe(host);
	});

	onDestroy(() => {
		unlisten?.();
		sizeObserver?.disconnect();
		if (resizeTimer) clearTimeout(resizeTimer);
		releaseAll();
		// Not a disconnect: the panel is re-created whenever the page it lives
		// on is left and returned to, and the session must outlive that. The
		// tab closing is what ends the session; see closeTab.
	});

	$effect(() => {
		if (active) canvas?.focus();
	});
</script>

<div class="rdp" bind:this={host}>
	<!-- The canvas is the desktop: it takes focus and every key. The wrapper
	     only sizes and centres it. -->
	<canvas
		bind:this={canvas}
		width="1024"
		height="768"
		tabindex="0"
		aria-label="Remote desktop"
		onmousemove={onMove}
		onmousedown={(e) => onButton(e, 'down')}
		onmouseup={(e) => onButton(e, 'up')}
		onwheel={onWheel}
		onkeydown={onKeyDown}
		onkeyup={onKeyUp}
		onblur={releaseAll}
		onfocus={() => { if (phase === 'connected') void rdpClipboardSync(id).catch(() => {}); }}
		oncontextmenu={(e) => e.preventDefault()}
		class:dim={phase !== 'connected'}
	></canvas>

	{#if phase !== 'connected'}
		<div class="overlay" class:failed={phase === 'error'}>
			{#if phase === 'connecting'}
				<span class="spinner" aria-hidden="true"></span>
				<span>{t('rdp.connecting')}</span>
			{:else if phase === 'closed'}
				<span>{t('rdp.closed')}</span>
				{#if message}<small>{message}</small>{/if}
			{:else}
				<span>{t('rdp.error')}</span>
				{#if message}<small>{message}</small>{/if}
			{/if}
		</div>
	{/if}
</div>

<style>
	.rdp {
		position: relative;
		display: flex;
		align-items: center;
		justify-content: center;
		width: 100%;
		height: 100%;
		overflow: hidden;
		background: #000;
	}

	/* Scaled to fit, never stretched: the intrinsic size is the server's
	   framebuffer and CSS only ever shrinks it uniformly. */
	canvas {
		display: block;
		max-width: 100%;
		max-height: 100%;
		width: auto;
		height: auto;
		object-fit: contain;
		cursor: default;
		image-rendering: auto;
		outline: none;
	}

	canvas.dim {
		opacity: 0.35;
	}

	.overlay {
		position: absolute;
		inset: 0;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 8px;
		color: var(--color-text-secondary);
		font-size: 0.8125rem;
		pointer-events: none;
	}

	.overlay small {
		max-width: 60ch;
		text-align: center;
		color: var(--color-text-tertiary);
		font-family: var(--font-mono);
		font-size: 0.75rem;
		word-break: break-word;
	}

	.overlay.failed {
		color: var(--color-danger);
	}

	.spinner {
		width: 18px;
		height: 18px;
		border: 2px solid var(--color-border);
		border-top-color: var(--color-accent);
		border-radius: 50%;
		animation: spin 0.8s linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}
</style>
