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
	 *
	 * A VNC desktop uses this same panel: its backend sends frames in the same
	 * format, and only the commands differ. VNC wants the character a key
	 * produced, not its position, so there keys go as X11 keysyms.
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
		rdpWindowFullscreen,
		rdpUnicode,
		type MouseAction,
		type RdpConnectParams,
		type RdpStatus,
	} from '$lib/ipc/rdp';
	import {
		vncAck,
		vncClipboardSync,
		vncConnect,
		vncKey,
		vncMouse,
		vncResize,
		type VncConnectParams,
	} from '$lib/ipc/vnc';
	import { keysymForChar, keysymForCode, keysymForEvent } from '$lib/vnc/keysym';
	import { t } from '$lib/state/i18n.svelte';
	import { untrustedRefsFromError } from '$lib/ipc/sessions';
	import { getSettings } from '$lib/state/settings.svelte';
	import { isMobile } from '$lib/platform';
	import MobileKeyBar, { type BarKey } from '$lib/components/shared/MobileKeyBar.svelte';

	interface Props {
		id: string;
		params: RdpConnectParams | VncConnectParams;
		/** Which protocol the desktop speaks. RDP unless said otherwise. */
		protocol?: 'rdp' | 'vnc';
		active: boolean;
	}

	let { id, params, protocol = 'rdp', active }: Props = $props();

	const isVnc = (): boolean => protocol === 'vnc';
	/** Who the desktop is, for the full-screen bar. */
	const label = $derived('username' in params && params.username ? `${params.username}@${params.host}` : params.host);

	/** The commands both protocols have, each sent the right way. */
	const remote = {
		ack: () => (isVnc() ? vncAck(id) : rdpAck(id)),
		mouse: (x: number, y: number, action: MouseAction, button = 0, delta = 0) =>
			isVnc() ? vncMouse(id, x, y, action, button, delta) : rdpMouse(id, x, y, action, button, delta),
		resize: (width: number, height: number) => (isVnc() ? vncResize(id, width, height) : rdpResize(id, width, height)),
		clipboardSync: () => (isVnc() ? vncClipboardSync(id) : rdpClipboardSync(id)),
	};

	/** Press or release a key named by its position (`KeyboardEvent.code`). */
	async function pressCode(code: string, down: boolean): Promise<void> {
		if (isVnc()) {
			const keysym = keysymForCode(code);
			if (keysym !== null) await vncKey(id, keysym, down);
			return;
		}
		const entry = SCANCODES[code];
		if (entry) await rdpKey(id, entry[0], entry[1], !down);
	}

	/** Type one character that came without a key: an IME commit, a phone keyboard. */
	async function typeChar(point: number): Promise<void> {
		if (isVnc()) {
			const keysym = keysymForChar(String.fromCodePoint(point));
			if (keysym === null) return;
			await vncKey(id, keysym, true);
			await vncKey(id, keysym, false);
			return;
		}
		await rdpUnicode(id, point, false);
		await rdpUnicode(id, point, true);
	}

	let host: HTMLDivElement;
	let canvas: HTMLCanvasElement;
	let ctx: CanvasRenderingContext2D | null = null;

	type Phase = 'connecting' | 'connected' | 'closed' | 'error';
	let phase = $state<Phase>('connecting');
	let message = $state('');

	let unlisten: UnlistenFn | null = null;
	let sizeObserver: ResizeObserver | null = null;
	let resizeTimer: ReturnType<typeof setTimeout> | null = null;
	/**
	 * No resize goes to the server before this. Asked for within the first
	 * second after login, a resize is accepted but never answered — the
	 * server's display-control channel is still coming up — and the client
	 * can only give up on it and reconnect. Holding the request that long
	 * costs nothing anyone sees; the latest size wins when it fires.
	 */
	let resizeNotBefore = 0;
	const SETTLE_MS = 1500;


	/* --- full screen ------------------------------------------------------- */

	/**
	 * The desktop takes the whole window: the panel goes fixed over
	 * everything and the window itself goes full screen, so the remote is
	 * resized to the real screen. A bar at the top edge, shown briefly on
	 * entry and whenever the mouse touches the edge, is the way back;
	 * Ctrl+Alt+Enter toggles either way, as in Microsoft's client.
	 */
	let fullscreen = $state(false);
	let barShown = $state(false);
	let barTimer: ReturnType<typeof setTimeout> | null = null;

	function showBar(): void {
		barShown = true;
		if (barTimer) clearTimeout(barTimer);
		barTimer = setTimeout(() => (barShown = false), 2500);
	}

	/**
	 * Where the panel sat before it was lifted to the document body. An
	 * ancestor of the panel clips fixed positioning to its own box — that is
	 * what `contain` and transforms do — so over-everything means out of
	 * that ancestor for the duration.
	 */
	let placeholder: Comment | null = null;

	function lift(on: boolean): void {
		if (on && !placeholder) {
			placeholder = document.createComment('rdp panel');
			host.parentNode?.insertBefore(placeholder, host);
			document.body.appendChild(host);
		} else if (!on && placeholder) {
			placeholder.parentNode?.insertBefore(host, placeholder);
			placeholder.remove();
			placeholder = null;
		}
	}

	async function setFullscreen(on: boolean): Promise<void> {
		if (fullscreen === on) return;
		fullscreen = on;
		lift(on);
		try {
			await rdpWindowFullscreen(on);
		} catch (err) {
			// The panel is over everything either way, which is most of the
			// point; the rest is said, not swallowed.
			console.error('full screen:', err);
		}
		if (on) showBar();
		else if (barTimer) clearTimeout(barTimer);
		// The panel's size just changed; the observer sends the new size.
		canvas?.focus();
	}

	function onHostMove(e: MouseEvent): void {
		if (fullscreen && e.clientY <= 4) showBar();
	}

	/* --- GPU path ---------------------------------------------------------- */

	/**
	 * WebGL when the setting allows and the webview can: each region is
	 * uploaded into one texture the size of the framebuffer, and the texture
	 * is drawn as a quad once per message. The 2D canvas path below stays as
	 * the fallback; the two never mix on one canvas, since a canvas gives out
	 * one kind of context for its lifetime.
	 */
	let gl: WebGLRenderingContext | null = null;
	let glW = 0;
	let glH = 0;

	function initGl(): void {
		if (!getSettings().rdpHardwareRendering) return;
		const g = canvas.getContext('webgl', { alpha: false, antialias: false, depth: false, stencil: false, preserveDrawingBuffer: true, premultipliedAlpha: false });
		if (!g) return;
		const compile = (type: number, src: string): WebGLShader | null => {
			const sh = g.createShader(type);
			if (!sh) return null;
			g.shaderSource(sh, src);
			g.compileShader(sh);
			return g.getShaderParameter(sh, g.COMPILE_STATUS) ? sh : null;
		};
		const vs = compile(g.VERTEX_SHADER, 'attribute vec2 p; varying vec2 t; void main() { t = vec2((p.x + 1.0) * 0.5, (1.0 - p.y) * 0.5); gl_Position = vec4(p, 0.0, 1.0); }');
		const fs = compile(g.FRAGMENT_SHADER, 'precision mediump float; varying vec2 t; uniform sampler2D s; void main() { gl_FragColor = texture2D(s, t); }');
		const prog = g.createProgram();
		if (!vs || !fs || !prog) return;
		g.attachShader(prog, vs);
		g.attachShader(prog, fs);
		g.linkProgram(prog);
		if (!g.getProgramParameter(prog, g.LINK_STATUS)) return;
		g.useProgram(prog);
		const quad = g.createBuffer();
		g.bindBuffer(g.ARRAY_BUFFER, quad);
		g.bufferData(g.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]), g.STATIC_DRAW);
		const p = g.getAttribLocation(prog, 'p');
		g.enableVertexAttribArray(p);
		g.vertexAttribPointer(p, 2, g.FLOAT, false, 0, 0);
		const tex = g.createTexture();
		g.bindTexture(g.TEXTURE_2D, tex);
		g.texParameteri(g.TEXTURE_2D, g.TEXTURE_MIN_FILTER, g.NEAREST);
		g.texParameteri(g.TEXTURE_2D, g.TEXTURE_MAG_FILTER, g.NEAREST);
		g.texParameteri(g.TEXTURE_2D, g.TEXTURE_WRAP_S, g.CLAMP_TO_EDGE);
		g.texParameteri(g.TEXTURE_2D, g.TEXTURE_WRAP_T, g.CLAMP_TO_EDGE);
		g.pixelStorei(g.UNPACK_ALIGNMENT, 1);
		gl = g;
	}

	function glSized(w: number, h: number): void {
		if (!gl) return;
		gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, w, h, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
		gl.viewport(0, 0, w, h);
		glW = w;
		glH = h;
	}

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
		let painted = false;

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
				if (!gl) ctx = canvas.getContext('2d', { alpha: false });
			}
			if (gl) {
				if (glW !== fbW || glH !== fbH) glSized(fbW, fbH);
				gl.texSubImage2D(gl.TEXTURE_2D, 0, x, y, w, h, gl.RGBA, gl.UNSIGNED_BYTE, new Uint8Array(buf, at, bytes));
				painted = true;
			} else {
				if (!ctx) ctx = canvas.getContext('2d', { alpha: false });
				if (ctx) {
					const pixels = new Uint8ClampedArray(buf, at, bytes);
					ctx.putImageData(new ImageData(pixels, w, h), x, y);
				}
			}
			at += bytes;
		}

		if (gl && painted) gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
		void remote.ack().catch(() => {});
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
			void remote.mouse(x, y, 'move');
		});
	}

	function onButton(e: MouseEvent, action: 'down' | 'up'): void {
		if (e.button > 2) return;
		const p = remotePoint(e);
		if (!p) return;
		if (action === 'down') canvas.focus();
		e.preventDefault();
		void remote.mouse(p.x, p.y, action, e.button);
	}

	function onWheel(e: WheelEvent): void {
		const p = remotePoint(e);
		if (!p || e.deltaY === 0) return;
		e.preventDefault();
		// A browser notch is deltaY ≈ +100 downwards; an RDP notch is 120 and
		// positive means towards the user. One notch per event keeps trackpads
		// from firing a flood of tiny steps.
		void remote.mouse(p.x, p.y, 'wheel', 0, e.deltaY < 0 ? 120 : -120);
	}

	/* --- phone ------------------------------------------------------------- */

	/**
	 * A phone has no keyboard to send keys from, and Android never opens its
	 * on-screen keyboard for a canvas. What it types lands in a hidden text
	 * field instead, and is sent on as it arrives. RDP never says where the
	 * focus is on the remote, so the keyboard is opened by a button, as in
	 * Microsoft's own mobile client.
	 */
	const onPhone = isMobile();
	/** The part of the panel the desktop is drawn in; the key bar sits under it. */
	let screen: HTMLDivElement | undefined = $state();
	let typeEl: HTMLTextAreaElement | undefined = $state();
	let barCtrl = $state(false);
	let barAlt = $state(false);
	/** What the keyboard's current composition has already sent. Gboard and
	 *  most keyboards compose a word as it is typed; the difference is sent
	 *  each time it changes, so letters appear on the remote as they are typed. */
	let composed = '';

	function tapKey(code: string): void {
		void pressCode(code, true).then(() => pressCode(code, false));
	}

	/** Hold Ctrl, Alt and Win as the sticky bar keys say, around `send`. */
	async function withModifiers(send: () => Promise<void>, win = false): Promise<void> {
		const mods = [barCtrl && 'ControlLeft', barAlt && 'AltLeft', win && 'MetaLeft'].filter(Boolean) as string[];
		barCtrl = false;
		barAlt = false;
		for (const m of mods) await pressCode(m, true);
		await send();
		for (const m of mods.reverse()) await pressCode(m, false);
	}

	/** The physical key for a letter or digit, so Ctrl+C is a real Ctrl+C. */
	function codeFor(ch: string): string | null {
		if (/^[a-z]$/i.test(ch)) return `Key${ch.toUpperCase()}`;
		if (/^[0-9]$/.test(ch)) return `Digit${ch}`;
		return null;
	}

	function sendText(text: string): void {
		for (const ch of text) {
			if (ch === '\n') {
				tapKey('Enter');
				continue;
			}
			const code = codeFor(ch);
			if ((barCtrl || barAlt) && code) {
				void withModifiers(async () => tapKey(code));
				continue;
			}
			void typeChar(ch.codePointAt(0) ?? 0);
		}
	}

	/** Send the change from `before` to `after`: backspaces, then new text. */
	function sendChange(before: string, after: string): void {
		let same = 0;
		while (same < before.length && same < after.length && before[same] === after[same]) same++;
		for (let i = same; i < before.length; i++) tapKey('Backspace');
		sendText(after.slice(same));
	}

	function onSinkBeforeInput(e: InputEvent): void {
		if (phase !== 'connected' || e.isComposing) return;
		if (e.inputType === 'deleteContentBackward') {
			e.preventDefault();
			tapKey('Backspace');
		} else if (e.inputType === 'insertLineBreak' || e.inputType === 'insertParagraph') {
			e.preventDefault();
			tapKey('Enter');
		} else if (e.inputType === 'insertText' && e.data) {
			e.preventDefault();
			sendText(e.data);
		}
	}

	function onSinkComposition(e: CompositionEvent): void {
		if (phase !== 'connected') return;
		if (e.type === 'compositionstart') {
			composed = '';
			return;
		}
		sendChange(composed, e.data ?? '');
		composed = e.data ?? '';
		if (e.type === 'compositionend') {
			composed = '';
			if (typeEl) typeEl.value = '';
		}
	}

	/** A hardware keyboard plugged into the phone still sends real keys: those
	 *  go the desktop's way. The on-screen keyboard's keys (code 229) do not. */
	function onSinkKeyDown(e: KeyboardEvent): void {
		if (e.keyCode !== 229 && isKey(e)) onKeyDown(e);
	}

	function onSinkKeyUp(e: KeyboardEvent): void {
		if (e.keyCode !== 229 && isKey(e)) onKeyUp(e);
	}

	function toggleKeyboard(): void {
		if (!typeEl) return;
		if (document.activeElement === typeEl) typeEl.blur();
		else typeEl.focus({ preventScroll: true });
	}

	function onBarKey(key: BarKey): void {
		if (phase !== 'connected') return;
		if (typeof key === 'object') {
			sendText(key.char);
			return;
		}
		if (key === 'ctrl-alt-del') {
			barCtrl = true;
			barAlt = true;
			void withModifiers(async () => tapKey('Delete'));
			return;
		}
		if (key === 'win') {
			void withModifiers(async () => tapKey('MetaLeft'));
			return;
		}
		const code = {
			esc: 'Escape', tab: 'Tab', up: 'ArrowUp', down: 'ArrowDown', left: 'ArrowLeft', right: 'ArrowRight',
			home: 'Home', end: 'End', pgup: 'PageUp', pgdn: 'PageDown', del: 'Delete'
		}[key];
		if (code) void withModifiers(async () => tapKey(code));
	}

	/** A long press is the right button, as on every touch screen. */
	function onLongPress(e: MouseEvent): void {
		e.preventDefault();
		if (!onPhone || phase !== 'connected') return;
		const p = remotePoint(e);
		if (!p) return;
		void remote.mouse(p.x, p.y, 'down', 2).then(() => remote.mouse(p.x, p.y, 'up', 2));
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

	/** VNC: the keysym each held key went down as, so it comes up as the same
	 *  one even if Shift was let go in between. */
	const heldKeysyms = new Map<string, number>();
	/** When Left Ctrl last went down. On Windows AltGr arrives as Left Ctrl
	 *  then Right Alt at the same instant; that Ctrl is not the user's. */
	let ctrlDownAt = 0;

	/** Whether a key event is one this desktop takes. */
	function isKey(e: KeyboardEvent): boolean {
		return isVnc() ? keysymForEvent(e) !== null || e.key === 'AltGraph' : !!SCANCODES[e.code];
	}

	function onVncKeyDown(e: KeyboardEvent): void {
		if (e.key === 'AltGraph') {
			// What AltGr types arrives as its own character; the remote needs
			// no modifier for it, and above all not the Ctrl Windows invented.
			e.preventDefault();
			const fake = heldKeysyms.get('ControlLeft');
			if (fake !== undefined && e.timeStamp - ctrlDownAt < 50) {
				heldKeysyms.delete('ControlLeft');
				void vncKey(id, fake, false);
			}
			return;
		}
		const keysym = keysymForEvent(e);
		if (keysym === null) return;
		e.preventDefault();
		if (e.code === 'ControlLeft') ctrlDownAt = e.timeStamp;
		heldKeysyms.set(e.code, keysym);
		void vncKey(id, keysym, true);
	}

	function onVncKeyUp(e: KeyboardEvent): void {
		const keysym = heldKeysyms.get(e.code);
		if (keysym === undefined) return;
		e.preventDefault();
		heldKeysyms.delete(e.code);
		void vncKey(id, keysym, false);
	}

	function onKeyDown(e: KeyboardEvent): void {
		if (e.ctrlKey && e.altKey && e.code === 'Enter') {
			e.preventDefault();
			void setFullscreen(!fullscreen);
			return;
		}
		if (phase !== 'connected') return;
		// The remote repeats a held key itself; forwarding the browser's
		// repeats as well would double the rate.
		if (e.repeat) return;
		if (isVnc()) {
			onVncKeyDown(e);
			return;
		}

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
			void typeChar(e.key.charCodeAt(0));
		}
	}

	function onKeyUp(e: KeyboardEvent): void {
		if (phase !== 'connected') return;
		if (isVnc()) {
			onVncKeyUp(e);
			return;
		}
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
		for (const keysym of heldKeysyms.values()) void vncKey(id, keysym, false);
		heldKeysyms.clear();
	}

	/* --- lifecycle --------------------------------------------------------- */

	/**
	 * The largest even size the panel can show, in device pixels, for the
	 * server to render at. Device pixels rather than CSS pixels: on a scaled
	 * display the panel is smaller in CSS pixels than on the screen, and a
	 * remote rendered at that size is stretched to fit. Rendered at the
	 * screen's own resolution and scaled down by CSS, it is pixel for pixel.
	 */
	function fitSize(): { width: number; height: number } {
		const r = (screen ?? host).getBoundingClientRect();
		const scale = window.devicePixelRatio || 1;
		const even = (v: number) => Math.min(8192, Math.max(200, Math.floor(v * scale) & ~1));
		return { width: even(r.width || 1024), height: even(r.height || 768) };
	}

	onMount(async () => {
		const size = fitSize();

		unlisten = await listen<RdpStatus>(`${isVnc() ? 'vnc' : 'rdp'}-status-${id}`, (event) => {
			const s = event.payload;
			if (s.state === 'connected' || s.state === 'loggedIn') {
				phase = 'connected';
				message = '';
				resizeNotBefore = Date.now() + SETTLE_MS;
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
			initGl();
			if (isVnc()) {
				// A VNC desktop comes at the size its server has; the panel
				// asks for its own size once connected, which some servers allow.
				await vncConnect(params as VncConnectParams, paint);
			} else {
				await rdpConnect({ ...(params as RdpConnectParams), ...size, graphicsPipeline: getSettings().rdpGraphicsPipeline }, paint);
			}
		} catch (err) {
			phase = 'error';
			// Unapproved password manager entries are asked about from the
			// session list, which this panel is not.
			message = untrustedRefsFromError(err) ? t('pwmanager.untrusted_reopen') : String(err);
			return;
		}

		// Ask the server to follow the panel when it is resized. Debounced:
		// a drag fires this continuously and each one is a renegotiation.
		sizeObserver = new ResizeObserver(() => {
			if (resizeTimer) clearTimeout(resizeTimer);
			resizeTimer = setTimeout(() => {
				if (Date.now() < resizeNotBefore) {
					resizeTimer = setTimeout(() => sizeObserver && host && requestResize(), resizeNotBefore - Date.now());
					return;
				}
				requestResize();
			}, 300);
		});
		sizeObserver.observe(screen ?? host);
	});

	/** Send the panel's current size, unless the desktop already has it. */
	function requestResize(): void {
		{
			if (phase !== 'connected') return;
			// A VNC desktop is usually shared and already laid out: asked to
			// take a phone's shape, the server cut its windows off at the new
			// edge, for everyone watching. It is scaled to fit here instead and
			// never resized, as noVNC does by default. RDP still resizes: each
			// RDP login is a session of its own.
			if (isVnc()) return;
			const s = fitSize();
			// The size the desktop already has is not a resize. Asking for
			// it anyway leaves a request the server never answers.
			if (s.width === canvas.width && s.height === canvas.height) return;
			void remote.resize(s.width, s.height);
		}
	}

	onDestroy(() => {
		unlisten?.();
		sizeObserver?.disconnect();
		if (resizeTimer) clearTimeout(resizeTimer);
		releaseAll();
		if (fullscreen) {
			lift(false);
			void rdpWindowFullscreen(false).catch(() => {});
		}
		if (barTimer) clearTimeout(barTimer);
		// Not a disconnect: the panel is re-created whenever the page it lives
		// on is left and returned to, and the session must outlive that. The
		// tab closing is what ends the session; see closeTab.
	});

	$effect(() => {
		if (active) canvas?.focus();
	});
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="rdp" class:fullscreen bind:this={host} onmousemove={onHostMove}>
	<!-- The desktop's area: sized and measured on its own, so a phone's key
	     bar below it is never drawn over the remote (its taskbar included). -->
	<div class="screen" bind:this={screen}>
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
		onfocus={() => { if (phase === 'connected') void remote.clipboardSync().catch(() => {}); }}
		oncontextmenu={onLongPress}
		class:dim={phase !== 'connected'}
	></canvas>

	{#if onPhone}
		<textarea
			bind:this={typeEl}
			class="type-sink"
			aria-hidden="true"
			tabindex="-1"
			autocomplete="off"
			autocapitalize="off"
			spellcheck="false"
			onbeforeinput={onSinkBeforeInput}
			oncompositionstart={onSinkComposition}
			oncompositionupdate={onSinkComposition}
			oncompositionend={onSinkComposition}
			onkeydown={onSinkKeyDown}
			onkeyup={onSinkKeyUp}
		></textarea>
	{/if}

	{#if phase === 'connected' && !fullscreen && !onPhone}
		<button type="button" class="fs-button" title={t('rdp.fullscreen_hint')} onclick={() => setFullscreen(true)}>
			<svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"><path fill="currentColor" d="M2 6V2h4v1.5H3.5V6zm8-4h4v4h-1.5V3.5H10zM2 10h1.5v2.5H6V14H2zm10.5 0H14v4h-4v-1.5h2.5z"/></svg>
			{t('rdp.fullscreen')}
		</button>
	{/if}

	{#if fullscreen && !onPhone}
		<div class="fs-bar" class:shown={barShown}>
			<span class="fs-host">{label}</span>
			<button type="button" class="fs-exit" onclick={() => setFullscreen(false)}>{t('rdp.exit_fullscreen')}</button>
		</div>
	{/if}

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

	{#if onPhone && phase === 'connected'}
		<MobileKeyBar desktop bind:ctrl={barCtrl} bind:alt={barAlt} onkey={onBarKey}>
			{#snippet leading()}
				<button
					type="button"
					class="bar-button"
					tabindex="-1"
					aria-label={t('rdp.keyboard')}
					onpointerdown={(e) => {
						e.preventDefault();
						toggleKeyboard();
					}}>⌨</button
				>
				<button
					type="button"
					class="bar-button"
					tabindex="-1"
					aria-label={fullscreen ? t('rdp.exit_fullscreen') : t('rdp.fullscreen')}
					onpointerdown={(e) => {
						e.preventDefault();
						void setFullscreen(!fullscreen);
					}}>{fullscreen ? '⤡' : '⤢'}</button
				>
			{/snippet}
		</MobileKeyBar>
	{/if}
</div>

<style>
	.rdp {
		position: relative;
		display: flex;
		flex-direction: column;
		width: 100%;
		height: 100%;
		overflow: hidden;
		background: #000;
	}

	.screen {
		position: relative;
		flex: 1;
		min-height: 0;
		width: 100%;
		display: flex;
		align-items: center;
		justify-content: center;
		overflow: hidden;
	}

	/* Where a phone's keyboard types: there, and invisible. 16px keeps the
	   webview from zooming in on it when it takes focus. */
	.type-sink {
		position: absolute;
		left: 0;
		bottom: 0;
		width: 1px;
		height: 1px;
		opacity: 0;
		font-size: 16px;
		border: none;
		padding: 0;
		resize: none;
		pointer-events: none;
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
	/* Full screen: over everything, the whole window, black behind. */
	.rdp.fullscreen {
		position: fixed;
		inset: 0;
		z-index: 1000;
		background: #000;
		/* The desktop stays clear of a phone's status and navigation bars;
		   the screen area inside is what gets measured, so scaling follows. */
		padding: env(safe-area-inset-top) env(safe-area-inset-right) env(safe-area-inset-bottom) env(safe-area-inset-left);
	}

	.fs-button {
		position: absolute;
		top: 8px;
		right: 8px;
		display: inline-flex;
		align-items: center;
		gap: 6px;
		padding: 5px 10px;
		font: inherit;
		font-size: 0.75rem;
		color: #fff;
		background: rgba(0, 0, 0, 0.55);
		border: 1px solid rgba(255, 255, 255, 0.25);
		border-radius: var(--radius-btn);
		cursor: pointer;
		opacity: 0;
		transition: opacity 150ms ease;
	}

	.rdp:hover .fs-button,
	.fs-button:focus-visible {
		opacity: 1;
	}

	/* The way back: a bar tucked above the top edge, shown when asked for. */
	.fs-bar {
		position: absolute;
		top: 0;
		left: 50%;
		transform: translate(-50%, -100%);
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 6px 14px;
		font-size: 0.75rem;
		color: #fff;
		background: rgba(0, 0, 0, 0.75);
		border: 1px solid rgba(255, 255, 255, 0.2);
		border-top: none;
		border-radius: 0 0 var(--radius-btn) var(--radius-btn);
		transition: transform 180ms ease;
		z-index: 1;
	}

	.fs-bar.shown,
	.fs-bar:hover {
		transform: translate(-50%, 0);
	}

	.fs-host {
		opacity: 0.8;
	}

	.fs-exit {
		padding: 3px 10px;
		font: inherit;
		font-size: 0.75rem;
		color: #fff;
		background: var(--color-accent);
		border: none;
		border-radius: var(--radius-btn);
		cursor: pointer;
	}
</style>
