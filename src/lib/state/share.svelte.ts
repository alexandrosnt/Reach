/**
 * Session sharing: the state the UI reads and the wiring the peer needs.
 *
 * Two rules, both inherited from the MCP server:
 *
 * 1. Off means no code runs. `$lib/share/peer` and everything under it is
 *    reached only by the dynamic import in `makePeer`, which runs only when
 *    the user starts or joins a share. Vite splits that import into its own
 *    chunk, so a user who never touches sharing never downloads it either.
 *    Every import at the top of this file is type-only or configuration.
 *
 * 2. Enabled is not active. Loading the config contacts nothing. The first
 *    packet leaves in response to a click.
 *
 * The protocol is symmetric, so each side keeps the same two things: which
 * of its own terminals it is sharing (and on what terms), and what it knows
 * about the other side's. "Read-only" is enforced here, on the receiving
 * end of `in` frames — the other machine's claim about what it may do is
 * never consulted.
 */

import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import * as ipc from '$lib/ipc/share';
import type { IceServer, ShareConfig } from '$lib/ipc/share';
import type { Message, Mode } from '$lib/share/protocol';
import type { PeerState, ShareFailure, SharePeer } from '$lib/share/peer';
import { sshSend } from '$lib/ipc/ssh';
import { ptyWrite } from '$lib/ipc/pty';
import { getTabs, type Tab } from '$lib/state/tabs.svelte';
import { readTerminalSize } from '$lib/state/terminal-buffer.svelte';

export type Role = 'host' | 'guest';

export type Phase =
	/** Nothing going on. */
	| 'idle'
	/** Gathering candidates; a few seconds. */
	| 'preparing'
	/** Host: code ready, waiting for the reply. Guest: reply ready, waiting for the host. */
	| 'exchanging'
	/** Both descriptions applied; ICE and DTLS in progress. */
	| 'connecting'
	| 'connected'
	| 'ended'
	| 'failed';

export interface RemoteInfo {
	title: string;
	cols: number;
	rows: number;
	/** What the other side allows me to do to their terminal. */
	mode: Mode;
	/** Whether they are sharing a terminal at all, or only watching mine. */
	sharing: boolean;
}

export interface Session {
	role: Role;
	phase: Phase;
	/** The local tab being shared, or null to watch only. */
	localTabId: string | null;
	/** What the other side may do to my terminal. Enforced here. */
	localMode: Mode;
	/** The code to hand to the other side, once produced. */
	myCode: string | null;
	failure: ShareFailure | null;
	remote: RemoteInfo | null;
}

let config = $state<ShareConfig>({ enabled: false, iceServers: [] });
let session = $state<Session | null>(null);
let configLoaded = $state(false);

// Not reactive, and not in `session`: the peer is a live object with its
// own state machine, and the UI only ever needs the phase it reports.
let peer: SharePeer | null = null;
let unlistenOutput: UnlistenFn | null = null;
let sizeTimer: ReturnType<typeof setInterval> | null = null;
let lastSize: { cols: number; rows: number } | null = null;

// Remote output goes to whichever RemoteTerminal is mounted. Frames that
// arrive before it mounts are kept, capped, so the pane does not open blank.
let outputSubscriber: ((data: Uint8Array) => void) | null = null;
const backlog: Uint8Array[] = [];
let backlogBytes = 0;
const BACKLOG_MAX = 256 * 1024;

// ---------------------------------------------------------------------------
// reads
// ---------------------------------------------------------------------------

export function getConfig(): ShareConfig {
	return config;
}

export function isEnabled(): boolean {
	return config.enabled;
}

export function getSession(): Session | null {
	return session;
}

/** True while the remote pane should be on screen. */
export function isShowingRemote(): boolean {
	return session?.phase === 'connected' && session.remote?.sharing === true;
}

// ---------------------------------------------------------------------------
// configuration
// ---------------------------------------------------------------------------

/** Apply what the vault holds. Call once it is unlocked. */
export async function load(): Promise<void> {
	try {
		config = await ipc.shareLoad();
	} catch {
		// A locked vault or a missing key leaves the default in place, which
		// is off. Never fail loudly at startup over an optional feature.
	}
	configLoaded = true;
}

export function isLoaded(): boolean {
	return configLoaded;
}

export async function setEnabled(enabled: boolean): Promise<void> {
	config = await ipc.shareSetEnabled(enabled);
	// Turning it off is a decision about the feature, and an active share is
	// the feature running.
	if (!enabled && session) end();
}

export async function setIceServers(servers: IceServer[]): Promise<void> {
	config = await ipc.shareSetIceServers(servers);
}

export async function resetIceServers(): Promise<void> {
	config = await ipc.shareResetIceServers();
}

// ---------------------------------------------------------------------------
// the peer
// ---------------------------------------------------------------------------

async function makePeer(role: Role): Promise<SharePeer> {
	// The only place the sharing code is loaded from. See the module doc.
	const { SharePeer } = await import('$lib/share/peer');
	return new SharePeer(config.iceServers, role, {
		onstate: onPeerState,
		onmessage: onPeerMessage,
		onfailure: onPeerFailure
	});
}

function onPeerState(state: PeerState): void {
	if (!session) return;
	switch (state) {
		case 'gathering':
			session.phase = 'preparing';
			break;
		case 'awaiting-answer':
			session.phase = 'exchanging';
			break;
		case 'connecting':
			// The guest reaches this before it has shown its reply, so the
			// phase that matters to the UI is still "hand this over".
			if (session.phase !== 'exchanging') session.phase = 'connecting';
			break;
		case 'connected':
			session.phase = 'connected';
			void onConnected();
			break;
		case 'closed':
			if (session.phase !== 'failed') session.phase = 'ended';
			teardown();
			break;
		case 'failed':
			session.phase = 'failed';
			teardown();
			break;
	}
}

function onPeerFailure(failure: ShareFailure): void {
	if (!session) return;
	session.failure = failure;
	if (failure.kind === 'bad-code') return; // recoverable: the user pastes again
	session.phase = 'failed';
	teardown();
}

function onPeerMessage(msg: Message): void {
	if (!session) return;
	switch (msg.type) {
		case 'hello':
			session.remote = {
				title: msg.title,
				cols: msg.cols,
				rows: msg.rows,
				mode: msg.mode,
				sharing: msg.sharing
			};
			break;
		case 'out':
			deliverRemoteOutput(msg.data);
			break;
		case 'in':
			// The one enforcement point. Their hello said what they would like
			// to do; this is what they are allowed to do.
			if (session.localMode === 'read-write') void writeToLocal(msg.data);
			break;
		case 'resize':
			if (session.remote) {
				session.remote.cols = msg.cols;
				session.remote.rows = msg.rows;
			}
			break;
		case 'mode':
			if (session.remote) session.remote.mode = msg.mode;
			break;
		case 'bye':
			break; // the peer closes itself; onPeerState handles it
	}
}

// ---------------------------------------------------------------------------
// starting and joining
// ---------------------------------------------------------------------------

function fresh(role: Role, localTabId: string | null, localMode: Mode): void {
	dismiss();
	session = {
		role,
		phase: 'preparing',
		localTabId,
		localMode,
		myCode: null,
		failure: null,
		remote: null
	};
}

/** Host: produce the code to send to the other person. */
export async function startHosting(localTabId: string | null, localMode: Mode): Promise<void> {
	if (!config.enabled) throw new Error('Sharing is turned off');
	fresh('host', localTabId, localMode);
	try {
		peer = await makePeer('host');
		const code = await peer.createOffer();
		if (session) {
			session.myCode = code;
			session.phase = 'exchanging';
		}
	} catch (e) {
		fail(e);
	}
}

/** Host: apply the reply that came back. */
export async function acceptAnswer(code: string): Promise<void> {
	if (!peer || !session || session.role !== 'host') return;
	session.failure = null;
	try {
		await peer.acceptAnswer(code);
		session.phase = 'connecting';
	} catch {
		// onPeerFailure already recorded a bad-code failure for the UI.
	}
}

/** Guest: take the host's code, produce the reply. */
export async function joinWithCode(
	code: string,
	localTabId: string | null,
	localMode: Mode
): Promise<void> {
	if (!config.enabled) throw new Error('Sharing is turned off');
	fresh('guest', localTabId, localMode);
	try {
		peer = await makePeer('guest');
		const reply = await peer.acceptOffer(code);
		if (session) {
			session.myCode = reply;
			session.phase = 'exchanging';
		}
	} catch (e) {
		fail(e);
	}
}

function fail(e: unknown): void {
	if (!session) return;
	if (!session.failure) {
		session.failure = { kind: 'bad-code', detail: e instanceof Error ? e.message : String(e) };
	}
	if (session.failure.kind !== 'bad-code') session.phase = 'failed';
	else session.phase = 'idle';
	teardown();
}

// ---------------------------------------------------------------------------
// while connected
// ---------------------------------------------------------------------------

function tabById(id: string | null): Tab | undefined {
	return id ? getTabs().find((t) => t.id === id) : undefined;
}

/** The id the terminal registered under, and the one its events carry. */
function streamIdOf(tab: Tab): string {
	return tab.type === 'ssh' && tab.connectionId ? tab.connectionId : tab.id;
}

async function onConnected(): Promise<void> {
	if (!peer || !session) return;
	const tab = tabById(session.localTabId);
	const size = tab ? readTerminalSize(streamIdOf(tab)) : null;
	lastSize = size;

	await peer.send({
		type: 'hello',
		mode: session.localMode,
		cols: size?.cols ?? 80,
		rows: size?.rows ?? 24,
		title: tab?.title ?? '',
		sharing: !!tab
	});

	if (tab) {
		await attachOutput(tab);
		// The terminal owns its resize path; polling its reported size is
		// the least invasive way to follow it. Once a second is plenty.
		sizeTimer = setInterval(() => {
			if (!peer || !session) return;
			const now = readTerminalSize(streamIdOf(tab));
			if (now && (now.cols !== lastSize?.cols || now.rows !== lastSize?.rows)) {
				lastSize = now;
				void peer.send({ type: 'resize', cols: now.cols, rows: now.rows });
			}
		}, 1000);
	}
}

async function attachOutput(tab: Tab): Promise<void> {
	const id = streamIdOf(tab);
	const event = tab.type === 'ssh' ? `ssh-data-${id}` : `pty-data-${id}`;
	unlistenOutput = await listen<number[] | string | Uint8Array>(event, (e) => {
		if (!peer) return;
		const p = e.payload;
		const data =
			typeof p === 'string'
				? new TextEncoder().encode(p)
				: p instanceof Uint8Array
					? p
					: Uint8Array.from(p);
		void peer.send({ type: 'out', data: data as Uint8Array<ArrayBuffer> });
	});
}

async function writeToLocal(data: Uint8Array): Promise<void> {
	const tab = tabById(session?.localTabId ?? null);
	if (!tab) return;
	const bytes = Array.from(data);
	if (tab.type === 'ssh' && tab.connectionId) await sshSend(tab.connectionId, bytes);
	else await ptyWrite(tab.id, bytes);
}

/** Change what the other side may do to my terminal, and tell them. */
export function setLocalMode(mode: Mode): void {
	if (!session) return;
	session.localMode = mode;
	if (peer && session.phase === 'connected') void peer.send({ type: 'mode', mode });
}

/** Keystrokes from the remote pane, headed for their terminal. */
export function sendInput(data: Uint8Array): void {
	if (!peer || session?.phase !== 'connected') return;
	// Courtesy only — they enforce their own mode. Not sending what will be
	// dropped just saves the bandwidth.
	if (session.remote?.mode !== 'read-write') return;
	void peer.send({ type: 'in', data: data as Uint8Array<ArrayBuffer> });
}

function deliverRemoteOutput(data: Uint8Array): void {
	if (outputSubscriber) {
		outputSubscriber(data);
		return;
	}
	backlog.push(data);
	backlogBytes += data.length;
	while (backlogBytes > BACKLOG_MAX && backlog.length > 0) {
		backlogBytes -= backlog.shift()!.length;
	}
}

/** The remote pane registers here. Backlog is replayed, then cleared. */
export function subscribeRemoteOutput(fn: (data: Uint8Array) => void): () => void {
	outputSubscriber = fn;
	for (const chunk of backlog) fn(chunk);
	backlog.length = 0;
	backlogBytes = 0;
	return () => {
		if (outputSubscriber === fn) outputSubscriber = null;
	};
}

// ---------------------------------------------------------------------------
// ending
// ---------------------------------------------------------------------------

function teardown(): void {
	unlistenOutput?.();
	unlistenOutput = null;
	if (sizeTimer) clearInterval(sizeTimer);
	sizeTimer = null;
	lastSize = null;
	peer = null;
	backlog.length = 0;
	backlogBytes = 0;
}

/** Say goodbye and tear down. The session stays visible as "ended". */
export function end(): void {
	const p = peer;
	teardown();
	p?.close();
	if (session && session.phase !== 'failed') session.phase = 'ended';
}

/** Clear an ended or failed session from view. */
export function dismiss(): void {
	if (peer) end();
	session = null;
}
