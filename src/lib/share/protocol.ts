/**
 * What travels over the DataChannel, once decrypted.
 *
 * One byte of type, then the payload. Terminal bytes go as bytes — base64
 * inside JSON would cost a third more bandwidth on the one message type that
 * carries nearly all of the traffic. Control messages are small and rare, so
 * they are JSON for the sake of being readable in a debugger.
 *
 * The protocol is symmetric. Each peer has a local terminal it may share and
 * a remote one it is watching. `out` always means "my terminal produced
 * this, show it"; `in` always means "I typed this, feed it to yours". Which
 * side is host and which is guest matters to the handshake and the nonces,
 * and to nothing here.
 */

import type { Bytes } from './crypto.ts';

export type Mode = 'read-only' | 'read-write';

export type Message =
	/** First thing each side sends: what it is sharing and on what terms. */
	| { type: 'hello'; mode: Mode; cols: number; rows: number; title: string; sharing: boolean }
	/** Terminal output from my session, for your remote pane. */
	| { type: 'out'; data: Bytes }
	/** Keystrokes from me, for your session. Honoured only if you allow it. */
	| { type: 'in'; data: Bytes }
	/** My terminal changed size. */
	| { type: 'resize'; cols: number; rows: number }
	/** I changed what you are allowed to do to my session. */
	| { type: 'mode'; mode: Mode }
	/** I am leaving. Cleaner than waiting for the channel to time out. */
	| { type: 'bye' };

const TYPE: Record<Message['type'], number> = {
	hello: 1,
	out: 2,
	in: 3,
	resize: 4,
	mode: 5,
	bye: 6
};

const BY_CODE: Record<number, Message['type']> = Object.fromEntries(
	Object.entries(TYPE).map(([k, v]) => [v, k as Message['type']])
);

/**
 * Larger frames are split by the peer before sealing. SCTP handles up to
 * 256 KiB but browsers disagree on what happens above 64 KiB, and a
 * terminal never needs a frame that big — one screen is a few kilobytes.
 */
export const MAX_PAYLOAD = 32 * 1024;

const text = new TextEncoder();
const untext = new TextDecoder();

function withType(code: number, payload: Uint8Array): Bytes {
	const frame = new Uint8Array(1 + payload.length);
	frame[0] = code;
	frame.set(payload, 1);
	return frame;
}

function json(code: number, value: unknown): Bytes {
	return withType(code, text.encode(JSON.stringify(value)));
}

export function encode(msg: Message): Bytes {
	switch (msg.type) {
		case 'hello':
			return json(TYPE.hello, {
				mode: msg.mode,
				cols: msg.cols,
				rows: msg.rows,
				title: msg.title,
				sharing: msg.sharing
			});
		case 'out':
		case 'in':
			if (msg.data.length > MAX_PAYLOAD) {
				throw new Error(`Payload of ${msg.data.length} bytes exceeds ${MAX_PAYLOAD}; split it first`);
			}
			return withType(TYPE[msg.type], msg.data);
		case 'resize':
			return json(TYPE.resize, { cols: msg.cols, rows: msg.rows });
		case 'mode':
			return json(TYPE.mode, { mode: msg.mode });
		case 'bye':
			return withType(TYPE.bye, new Uint8Array(0));
	}
}

function isMode(v: unknown): v is Mode {
	return v === 'read-only' || v === 'read-write';
}

function isDim(v: unknown): v is number {
	return typeof v === 'number' && Number.isInteger(v) && v > 0 && v <= 1000;
}

/**
 * Parse a frame from the other side.
 *
 * Every field is checked, because it was produced by a program on somebody
 * else's machine. A `cols` of 10⁹ must not reach the terminal, and a mode
 * that is not one of the two must not silently become read-write.
 */
export function decode(frame: Bytes): Message {
	if (frame.length === 0) throw new Error('Empty frame');
	const type = BY_CODE[frame[0]];
	if (!type) throw new Error(`Unknown frame type ${frame[0]}`);
	const payload = frame.subarray(1);
	if (payload.length > MAX_PAYLOAD + 256) {
		throw new Error(`Frame of ${payload.length} bytes is over the cap`);
	}

	switch (type) {
		case 'out':
		case 'in':
			return { type, data: payload };
		case 'bye':
			return { type };
		case 'hello': {
			const v = JSON.parse(untext.decode(payload));
			if (!isMode(v.mode) || !isDim(v.cols) || !isDim(v.rows) || typeof v.title !== 'string') {
				throw new Error('Malformed hello');
			}
			return {
				type,
				mode: v.mode,
				cols: v.cols,
				rows: v.rows,
				title: v.title.slice(0, 200),
				sharing: v.sharing === true
			};
		}
		case 'resize': {
			const v = JSON.parse(untext.decode(payload));
			if (!isDim(v.cols) || !isDim(v.rows)) throw new Error('Malformed resize');
			return { type, cols: v.cols, rows: v.rows };
		}
		case 'mode': {
			const v = JSON.parse(untext.decode(payload));
			if (!isMode(v.mode)) throw new Error('Malformed mode');
			return { type, mode: v.mode };
		}
	}
}

/** Split terminal bytes into frames the channel will accept. */
export function chunk(data: Bytes): Bytes[] {
	if (data.length <= MAX_PAYLOAD) return [data];
	const out: Bytes[] = [];
	for (let at = 0; at < data.length; at += MAX_PAYLOAD) {
		out.push(data.subarray(at, Math.min(at + MAX_PAYLOAD, data.length)));
	}
	return out;
}
