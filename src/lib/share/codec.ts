/**
 * The codes two people paste to each other.
 *
 * Signalling with no server: the host produces one code, sends it over any
 * channel they already trust — a DM, a chat, a shouted string — and the guest
 * sends one back. That is the entire rendezvous. Nothing of ours is running,
 * nothing is hosted, and there is no account.
 *
 *   offer   REACH-<v1><token 16B><deflate(sdp)>
 *   answer  REACH-<v1><sealed(deflate(sdp))>
 *
 * The offer carries the token in the clear, because the code is the thing
 * being handed over and there is nothing else to protect it with. The answer
 * is sealed with the key derived from that token, which makes it proof: a
 * guest who did not have the token cannot produce an answer the host will
 * accept, and the host never has to trust the channel the answer came back
 * on. The sealed answer is frame zero of the guest's stream, so its counter
 * flows straight into the DataChannel frames that follow.
 */

import { TOKEN_BYTES, type Bytes, type Sealer } from './crypto.ts';

const PREFIX = 'REACH-';
const VERSION = 1;

/** Sanity cap. A real offer deflates to a few hundred bytes. */
const MAX_CODE_CHARS = 8192;

// ---------------------------------------------------------------------------
// bytes <-> text
// ---------------------------------------------------------------------------

/** base64url, unpadded. Safe in a URL, a chat message and a filename. */
export function toBase64Url(bytes: Uint8Array): string {
	let bin = '';
	for (const b of bytes) bin += String.fromCharCode(b);
	return btoa(bin).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

export function fromBase64Url(text: string): Bytes {
	const b64 = text.replace(/-/g, '+').replace(/_/g, '/');
	const padded = b64 + '='.repeat((4 - (b64.length % 4)) % 4);
	const bin = atob(padded);
	const out = new Uint8Array(bin.length);
	for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
	return out;
}

// ---------------------------------------------------------------------------
// compression
// ---------------------------------------------------------------------------

async function pump(bytes: Bytes, stream: CompressionStream | DecompressionStream): Promise<Bytes> {
	const writer = stream.writable.getWriter();

	// The read side is where a failure is reported. But when inflate rejects
	// damaged input, the write side's close() rejects too, and a rejection
	// nobody is holding is an unhandled one — which crashes Node and litters
	// the webview console on every mistyped code. So the write outcome is
	// captured as a value, never a rejection, and consulted only after the
	// read side finished cleanly.
	const writeOutcome: Promise<unknown> = writer
		.write(bytes)
		.then(() => writer.close())
		.then(
			() => null,
			(e: unknown) => e ?? new Error('stream write failed')
		);

	const reader = stream.readable.getReader();
	const chunks: Uint8Array[] = [];
	let total = 0;
	for (;;) {
		const { value, done } = await reader.read();
		if (done) break;
		chunks.push(value);
		total += value.length;
	}

	const failure = await writeOutcome;
	if (failure) throw failure;

	const out = new Uint8Array(total);
	let at = 0;
	for (const c of chunks) {
		out.set(c, at);
		at += c.length;
	}
	return out;
}

/** deflate-raw: no zlib header, no checksum. Every byte of a code is pasted. */
export function deflate(bytes: Bytes): Promise<Bytes> {
	return pump(bytes, new CompressionStream('deflate-raw'));
}

export function inflate(bytes: Bytes): Promise<Bytes> {
	return pump(bytes, new DecompressionStream('deflate-raw'));
}

// ---------------------------------------------------------------------------
// codes
// ---------------------------------------------------------------------------

const text = new TextEncoder();
const untext = new TextDecoder();

function wrap(payload: Bytes): string {
	const body = new Uint8Array(1 + payload.length);
	body[0] = VERSION;
	body.set(payload, 1);
	return PREFIX + toBase64Url(body);
}

/**
 * Accept what a chat app hands back.
 *
 * Long strings get wrapped, get a trailing space, get their prefix lowercased
 * by an eager autocorrect. None of that is the user's fault, so none of it is
 * an error: strip every whitespace character and compare the prefix without
 * caring about case.
 */
function unwrap(code: string): Bytes {
	const compact = code.replace(/\s+/g, '');
	if (compact.length > MAX_CODE_CHARS) {
		throw new Error('That is far too long to be a share code');
	}
	if (!compact.toUpperCase().startsWith(PREFIX)) {
		throw new Error(`A share code starts with ${PREFIX}`);
	}
	let body: Bytes;
	try {
		body = fromBase64Url(compact.slice(PREFIX.length));
	} catch {
		throw new Error('That does not look like a share code');
	}
	if (body.length < 2) {
		throw new Error('That does not look like a share code');
	}
	if (body[0] !== VERSION) {
		throw new Error(`Share code version ${body[0]} is not one this build understands`);
	}
	return body.subarray(1);
}

/** The host's code: token and offer, for the guest to paste. */
export async function encodeOffer(token: Bytes, sdp: string): Promise<string> {
	if (token.length !== TOKEN_BYTES) {
		throw new Error(`Token must be ${TOKEN_BYTES} bytes`);
	}
	const packed = await deflate(text.encode(sdp) as Bytes);
	const payload = new Uint8Array(TOKEN_BYTES + packed.length);
	payload.set(token, 0);
	payload.set(packed, TOKEN_BYTES);
	return wrap(payload);
}

export async function decodeOffer(code: string): Promise<{ token: Bytes; sdp: string }> {
	const payload = unwrap(code);
	if (payload.length <= TOKEN_BYTES) {
		throw new Error('Share code is missing its offer');
	}
	const token = payload.slice(0, TOKEN_BYTES);
	let sdp: string;
	try {
		sdp = untext.decode(await inflate(payload.subarray(TOKEN_BYTES)));
	} catch {
		throw new Error('Share code is damaged');
	}
	if (!sdp.includes('a=fingerprint:')) {
		throw new Error('Share code does not contain a valid offer');
	}
	return { token, sdp };
}

/**
 * The guest's reply. Sealed with the guest's sealer, so the same instance
 * must go on to carry the DataChannel — the answer is its frame zero.
 */
export async function encodeAnswer(sealer: Sealer, sdp: string): Promise<string> {
	const packed = await deflate(text.encode(sdp) as Bytes);
	return wrap(await sealer.seal(packed));
}

/** Throws unless the answer was sealed with the key this host derived. */
export async function decodeAnswer(sealer: Sealer, code: string): Promise<string> {
	const payload = unwrap(code);
	let packed: Bytes;
	try {
		packed = await sealer.open(payload);
	} catch {
		// Deliberately one message for every failure. Distinguishing "wrong
		// token" from "damaged" would tell a guesser which it was.
		throw new Error('That answer was not made for this share');
	}
	const sdp = untext.decode(await inflate(packed));
	if (!sdp.includes('a=fingerprint:')) {
		throw new Error('That answer was not made for this share');
	}
	return sdp;
}
