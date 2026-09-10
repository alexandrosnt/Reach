/**
 * One end of a shared session.
 *
 * Wraps the webview's own RTCPeerConnection. There is no Rust in this path:
 * the browser engine already ships a complete WebRTC stack, and this class
 * is what stands between it and the rest of the app.
 *
 * Roles exist only for the handshake. The host makes the offer and owns the
 * DataChannel; the guest answers. Once connected the protocol is symmetric —
 * each side may share its own terminal and watch the other's.
 *
 * Nothing in this module runs until something constructs a `SharePeer`, and
 * nothing constructs one until the user clicks Share. Importing it does not
 * contact a STUN server. Creating an instance does not either — gathering
 * starts with `createOffer` or `acceptOffer`, and not before.
 */

import type { IceServer } from '$lib/ipc/share';
import { deriveKey, randomToken, Sealer, type Bytes, type Direction } from './crypto.ts';
import { decodeAnswer, decodeOffer, encodeAnswer, encodeOffer } from './codec.ts';
import { chunk, decode, encode, type Message } from './protocol.ts';

export type PeerState =
	| 'idle'
	| 'gathering'
	| 'awaiting-answer'
	| 'connecting'
	| 'connected'
	| 'closed'
	| 'failed';

/** Why a share did not work, in terms a person can act on. */
export type ShareFailure =
	/** ICE found no path. Usually symmetric NAT on one side; TURN or a shared network fixes it. */
	| { kind: 'no-path'; detail: string }
	/** The pasted code was not one of ours, or not for this share. */
	| { kind: 'bad-code'; detail: string }
	/** Connected, then lost. */
	| { kind: 'dropped'; detail: string }
	/** A frame arrived that did not verify. The link is no longer trusted. */
	| { kind: 'tampered'; detail: string }
	/** The other side never turned up. */
	| { kind: 'timeout'; detail: string };

export interface PeerCallbacks {
	onstate?: (state: PeerState) => void;
	onmessage?: (msg: Message) => void;
	onfailure?: (failure: ShareFailure) => void;
}

/** Some networks never report `complete`; after this long, go with what we have. */
const GATHER_TIMEOUT_MS = 10_000;
/** From "answer applied" to "channel open". Generous: two hole-punches and a DTLS handshake. */
const CONNECT_TIMEOUT_MS = 30_000;
/** Backpressure. A slow guest must not make the host's terminal stall. */
const BUFFER_HIGH = 1 * 1024 * 1024;
const BUFFER_LOW = 256 * 1024;

export class SharePeer {
	#pc: RTCPeerConnection;
	#dc: RTCDataChannel | null = null;
	#sealer: Sealer | null = null;
	#token: Bytes | null = null;
	#direction: Direction;
	#state: PeerState = 'idle';
	#cb: PeerCallbacks;
	#closed = false;
	#connectTimer: ReturnType<typeof setTimeout> | null = null;

	// Both directions are serialised. Sends, so frames leave in the order
	// they were queued even though sealing is async. Receives, because the
	// counter check in `Sealer.open` requires frames to be opened in the
	// order they arrived — two decrypts racing would make the second look
	// like a replay of the first.
	#sendChain: Promise<void> = Promise.resolve();
	#recvChain: Promise<void> = Promise.resolve();

	/** Candidate types gathered locally, for diagnostics. */
	gathered = new Set<string>();

	constructor(iceServers: IceServer[], direction: Direction, callbacks: PeerCallbacks = {}) {
		this.#direction = direction;
		this.#cb = callbacks;
		this.#pc = new RTCPeerConnection({
			iceServers: iceServers.map((s) => ({
				urls: s.urls,
				username: s.username,
				credential: s.credential
			})),
			// Candidates only for the one channel; nothing here does media.
			bundlePolicy: 'max-bundle'
		});

		this.#pc.addEventListener('iceconnectionstatechange', () => {
			const s = this.#pc.iceConnectionState;
			if (s === 'failed') {
				this.#fail({
					kind: 'no-path',
					detail:
						'No route between the two machines was found. This usually means a ' +
						'symmetric NAT on one side. A TURN server, or both machines on one ' +
						'network or VPN, will fix it.'
				});
			}
		});

		this.#pc.addEventListener('connectionstatechange', () => {
			const s = this.#pc.connectionState;
			if (s === 'failed' && this.#state !== 'failed') {
				this.#fail({ kind: 'no-path', detail: 'The connection could not be established.' });
			} else if ((s === 'closed' || s === 'disconnected') && this.#state === 'connected') {
				this.#fail({ kind: 'dropped', detail: 'The other side went away.' });
			}
		});
	}

	get state(): PeerState {
		return this.#state;
	}

	get direction(): Direction {
		return this.#direction;
	}

	// -----------------------------------------------------------------------
	// host
	// -----------------------------------------------------------------------

	/**
	 * Produce the code to hand to the guest.
	 *
	 * Non-trickle: this waits for ICE gathering to finish so every candidate
	 * is inside the one code. A few seconds, in exchange for a single paste
	 * instead of a conversation.
	 */
	async createOffer(): Promise<string> {
		this.#expect('idle');
		this.#token = randomToken();
		this.#sealer = new Sealer(await deriveKey(this.#token), 'host');

		this.#dc = this.#pc.createDataChannel('reach', { ordered: true });
		this.#wireChannel(this.#dc);

		this.#setState('gathering');
		const offer = await this.#pc.createOffer();
		await this.#pc.setLocalDescription(offer);
		await this.#gatherComplete();

		this.#setState('awaiting-answer');
		return encodeOffer(this.#token, this.#localSdp());
	}

	/** Apply the guest's reply. Throws `bad-code` if it was not for this share. */
	async acceptAnswer(code: string): Promise<void> {
		this.#expect('awaiting-answer');
		let sdp: string;
		try {
			sdp = await decodeAnswer(this.#sealer!, code);
		} catch (e) {
			throw this.#reject({ kind: 'bad-code', detail: (e as Error).message });
		}
		await this.#pc.setRemoteDescription({ type: 'answer', sdp });
		this.#setState('connecting');
		this.#armConnectTimeout();
	}

	// -----------------------------------------------------------------------
	// guest
	// -----------------------------------------------------------------------

	/** Take the host's code, produce the reply. */
	async acceptOffer(code: string): Promise<string> {
		this.#expect('idle');
		let token: Bytes;
		let sdp: string;
		try {
			({ token, sdp } = await decodeOffer(code));
		} catch (e) {
			throw this.#reject({ kind: 'bad-code', detail: (e as Error).message });
		}

		// This same instance seals the answer and then every channel frame:
		// the answer is frame zero of the guest's stream.
		this.#sealer = new Sealer(await deriveKey(token), 'guest');
		this.#pc.addEventListener('datachannel', (e) => {
			this.#dc = e.channel;
			this.#wireChannel(e.channel);
		});

		await this.#pc.setRemoteDescription({ type: 'offer', sdp });
		this.#setState('gathering');
		const answer = await this.#pc.createAnswer();
		await this.#pc.setLocalDescription(answer);
		await this.#gatherComplete();

		this.#setState('connecting');
		this.#armConnectTimeout();
		return encodeAnswer(this.#sealer, this.#localSdp());
	}

	// -----------------------------------------------------------------------
	// traffic
	// -----------------------------------------------------------------------

	/**
	 * Queue a message. Resolves once it has been handed to the channel.
	 *
	 * Terminal bursts bigger than one frame are split; each piece is its own
	 * sealed frame with its own counter, and they are reassembled by the
	 * simple fact that the receiver writes them to the terminal in order.
	 */
	send(msg: Message): Promise<void> {
		const pieces: Message[] =
			msg.type === 'out' || msg.type === 'in'
				? chunk(msg.data).map((data) => ({ type: msg.type, data }) as Message)
				: [msg];

		this.#sendChain = this.#sendChain
			.then(async () => {
				for (const piece of pieces) {
					const dc = this.#dc;
					if (!dc || dc.readyState !== 'open' || !this.#sealer) return;
					await this.#drain(dc);
					const sealed = await this.#sealer.seal(encode(piece));
					dc.send(sealed);
				}
			})
			.catch(() => {
				/* a send after close is not an error worth surfacing */
			});
		return this.#sendChain;
	}

	/** Say goodbye if possible, then tear down. Safe to call twice. */
	close(): void {
		if (this.#closed) return;
		this.#closed = true;
		this.#clearConnectTimeout();

		const finish = () => {
			try {
				this.#dc?.close();
			} catch {
				/* already closed */
			}
			this.#pc.close();
			this.#setState('closed');
		};

		if (this.#dc?.readyState === 'open') {
			// Give the bye a moment to leave. Not longer: a peer that has
			// vanished would otherwise hold the close open indefinitely.
			void Promise.race([this.send({ type: 'bye' }), new Promise((r) => setTimeout(r, 300))]).then(finish);
		} else {
			finish();
		}
	}

	// -----------------------------------------------------------------------
	// internals
	// -----------------------------------------------------------------------

	#localSdp(): string {
		const sdp = this.#pc.localDescription?.sdp;
		if (!sdp) throw new Error('No local description');
		for (const line of sdp.split('\n')) {
			const m = /typ (\w+)/.exec(line);
			if (line.startsWith('a=candidate') && m) this.gathered.add(m[1]);
		}
		return sdp;
	}

	#gatherComplete(): Promise<void> {
		if (this.#pc.iceGatheringState === 'complete') return Promise.resolve();
		return new Promise((resolve) => {
			const done = () => {
				this.#pc.removeEventListener('icegatheringstatechange', check);
				clearTimeout(timer);
				resolve();
			};
			const check = () => {
				if (this.#pc.iceGatheringState === 'complete') done();
			};
			const timer = setTimeout(done, GATHER_TIMEOUT_MS);
			this.#pc.addEventListener('icegatheringstatechange', check);
		});
	}

	#wireChannel(dc: RTCDataChannel): void {
		dc.binaryType = 'arraybuffer';

		dc.addEventListener('open', () => {
			this.#clearConnectTimeout();
			this.#setState('connected');
		});

		dc.addEventListener('close', () => {
			if (this.#state === 'connected') {
				this.#fail({ kind: 'dropped', detail: 'The channel closed.' });
			}
		});

		dc.addEventListener('message', (e: MessageEvent) => {
			this.#recvChain = this.#recvChain
				.then(() => this.#receive(e.data))
				.catch(() => {
					/* handled inside #receive */
				});
		});
	}

	async #receive(raw: unknown): Promise<void> {
		if (this.#closed || !this.#sealer) return;
		let msg: Message;
		try {
			if (!(raw instanceof ArrayBuffer)) throw new Error('Non-binary frame');
			const plain = await this.#sealer.open(new Uint8Array(raw));
			msg = decode(plain);
		} catch (e) {
			// A frame that fails to verify is a forgery, a replay, or a bug on
			// the other side. None of those is a link worth keeping.
			this.#fail({ kind: 'tampered', detail: (e as Error).message });
			return;
		}
		if (msg.type === 'bye') {
			this.#setState('closed');
			this.close();
			return;
		}
		this.#cb.onmessage?.(msg);
	}

	/** Wait for the channel's buffer to fall below the low mark. */
	#drain(dc: RTCDataChannel): Promise<void> {
		if (dc.bufferedAmount < BUFFER_HIGH) return Promise.resolve();
		dc.bufferedAmountLowThreshold = BUFFER_LOW;
		return new Promise((resolve) => {
			const onLow = () => {
				dc.removeEventListener('bufferedamountlow', onLow);
				resolve();
			};
			dc.addEventListener('bufferedamountlow', onLow);
		});
	}

	#armConnectTimeout(): void {
		this.#clearConnectTimeout();
		this.#connectTimer = setTimeout(() => {
			if (this.#state === 'connecting') {
				this.#fail({
					kind: 'timeout',
					detail: 'The other side did not connect in time. Check they pasted the code, and that neither of you is behind a NAT that needs TURN.'
				});
			}
		}, CONNECT_TIMEOUT_MS);
	}

	#clearConnectTimeout(): void {
		if (this.#connectTimer) {
			clearTimeout(this.#connectTimer);
			this.#connectTimer = null;
		}
	}

	#expect(state: PeerState): void {
		if (this.#state !== state) {
			throw new Error(`Cannot do that while ${this.#state}`);
		}
	}

	#setState(state: PeerState): void {
		if (this.#state === state) return;
		this.#state = state;
		this.#cb.onstate?.(state);
	}

	#fail(failure: ShareFailure): void {
		if (this.#state === 'failed' || this.#state === 'closed') return;
		this.#clearConnectTimeout();
		this.#setState('failed');
		this.#cb.onfailure?.(failure);
		this.#closed = true;
		try {
			this.#dc?.close();
		} catch {
			/* fine */
		}
		this.#pc.close();
	}

	/** A failure raised from a caller's own action: report it and also throw it. */
	#reject(failure: ShareFailure): Error {
		this.#cb.onfailure?.(failure);
		return new Error(failure.detail);
	}
}
