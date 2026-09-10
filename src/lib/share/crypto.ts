/**
 * End-to-end encryption for a shared session.
 *
 * WebRTC already encrypts the DataChannel with DTLS, so why another layer?
 * Because DTLS protects against someone on the network, not against whoever
 * carried the offer and answer. Today that carrier is the user pasting codes
 * into a chat they trust. If a signalling service is ever added for
 * convenience, it becomes a party that could swap DTLS fingerprints and sit
 * in the middle. This layer makes the share token the security, not the
 * signalling: a peer that cannot derive the key from the token cannot read a
 * frame or forge one, whatever happened to the SDP on the way.
 *
 * It also means the token is what a user hands over, and the token alone.
 * There is no second secret to explain.
 */

/**
 * Bytes backed by a plain ArrayBuffer.
 *
 * WebCrypto refuses a view over a SharedArrayBuffer, and since TypeScript 5.7
 * the type system tracks the difference. Everything in the sharing modules
 * speaks this type so the distinction is settled once, here, rather than
 * with a cast at every call into `crypto.subtle`.
 */
export type Bytes = Uint8Array<ArrayBuffer>;

const text = new TextEncoder();
const SALT = text.encode('reach-share-v1') as Bytes;
const INFO = text.encode('aes-256-gcm') as Bytes;

/** 128 bits. Enough that guessing is not a strategy; short enough to paste. */
export const TOKEN_BYTES = 16;

/** Which way a frame is travelling. Baked into the nonce, see `Sealer`. */
export type Direction = 'host' | 'guest';

const DIRECTION_TAG: Record<Direction, number> = { host: 1, guest: 2 };

export function randomToken(): Bytes {
	const token = new Uint8Array(TOKEN_BYTES);
	crypto.getRandomValues(token);
	return token;
}

/** HKDF-SHA256 from the token. Same token, same key, on both sides. */
export async function deriveKey(token: Bytes): Promise<CryptoKey> {
	if (token.length !== TOKEN_BYTES) {
		throw new Error(`Share token must be ${TOKEN_BYTES} bytes, got ${token.length}`);
	}
	const ikm = await crypto.subtle.importKey('raw', token, 'HKDF', false, ['deriveKey']);
	return crypto.subtle.deriveKey(
		{ name: 'HKDF', hash: 'SHA-256', salt: SALT, info: INFO },
		ikm,
		{ name: 'AES-GCM', length: 256 },
		false,
		['encrypt', 'decrypt']
	);
}

const NONCE_BYTES = 12;
/** AES-GCM authentication tag. */
const TAG_BYTES = 16;

/**
 * Nonce layout: 4-byte direction tag, 8-byte big-endian counter.
 *
 * AES-GCM is catastrophically broken by a repeated nonce under the same key,
 * and both peers share one key. Two independent counters starting at zero
 * would collide on the very first frame. The direction tag partitions the
 * nonce space so a host frame and a guest frame can never share one.
 */
function nonceFor(direction: Direction, counter: bigint): Bytes {
	const nonce = new Uint8Array(NONCE_BYTES);
	const view = new DataView(nonce.buffer);
	view.setUint32(0, DIRECTION_TAG[direction]);
	view.setBigUint64(4, counter);
	return nonce;
}

function counterOf(nonce: Bytes): { direction: number; counter: bigint } {
	const view = new DataView(nonce.buffer, nonce.byteOffset, nonce.byteLength);
	return { direction: view.getUint32(0), counter: view.getBigUint64(4) };
}

/**
 * Encrypts outgoing frames and decrypts incoming ones for one peer.
 *
 * `seal` uses this peer's direction; `open` insists on the other's. A frame
 * this peer sent, reflected back by something in the middle, therefore fails
 * to open — the direction tag is wrong — rather than being accepted as if the
 * other side had said it.
 */
export class Sealer {
	#key: CryptoKey;
	#mine: Direction;
	#theirs: Direction;
	#sendCounter = 0n;
	#lastReceived = -1n;

	constructor(key: CryptoKey, mine: Direction) {
		this.#key = key;
		this.#mine = mine;
		this.#theirs = mine === 'host' ? 'guest' : 'host';
	}

	/** nonce || ciphertext || tag. */
	async seal(plaintext: Bytes): Promise<Bytes> {
		const nonce = nonceFor(this.#mine, this.#sendCounter);
		this.#sendCounter += 1n;
		const ct = new Uint8Array(
			await crypto.subtle.encrypt({ name: 'AES-GCM', iv: nonce }, this.#key, plaintext)
		);
		const frame = new Uint8Array(NONCE_BYTES + ct.length);
		frame.set(nonce, 0);
		frame.set(ct, NONCE_BYTES);
		return frame;
	}

	/**
	 * Decrypt a frame from the other peer.
	 *
	 * Rejects the wrong direction, and any counter at or below the last one
	 * accepted. The DataChannel is ordered and reliable, so a genuine frame
	 * always arrives with a higher counter; anything else is a replay or a
	 * reflection, and either is an attack, not a hiccup.
	 */
	async open(frame: Bytes): Promise<Bytes> {
		if (frame.length < NONCE_BYTES + TAG_BYTES) {
			throw new Error('Frame too short to be authentic');
		}
		const nonce = frame.subarray(0, NONCE_BYTES);
		const { direction, counter } = counterOf(nonce);
		if (direction !== DIRECTION_TAG[this.#theirs]) {
			throw new Error('Frame carries the wrong direction tag');
		}
		if (counter <= this.#lastReceived) {
			throw new Error('Frame counter did not advance (replay?)');
		}
		const plaintext = new Uint8Array(
			await crypto.subtle.decrypt(
				{ name: 'AES-GCM', iv: nonce },
				this.#key,
				frame.subarray(NONCE_BYTES)
			)
		);
		// Only after the tag verified. A forged frame must not advance the
		// window and lock out the genuine one behind it.
		this.#lastReceived = counter;
		return plaintext;
	}
}
