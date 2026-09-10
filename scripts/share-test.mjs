/**
 * Tests the sharing core — crypto, codec, protocol — in Node, with no
 * browser and no network.
 *
 * Node 24 strips TypeScript types on import and ships WebCrypto and
 * CompressionStream, so the exact modules the webview will run are what is
 * under test here. Not a port of them.
 *
 * Run: node scripts/share-test.mjs
 */

import {
	randomToken, deriveKey, Sealer, TOKEN_BYTES
} from '../src/lib/share/crypto.ts';
import {
	encodeOffer, decodeOffer, encodeAnswer, decodeAnswer,
	deflate, inflate, toBase64Url, fromBase64Url
} from '../src/lib/share/codec.ts';
import { encode, decode, chunk, MAX_PAYLOAD } from '../src/lib/share/protocol.ts';

let failures = 0;
const check = (name, cond, extra = '') => {
	if (cond) console.log(`  PASS  ${name}`);
	else {
		failures++;
		console.log(`  FAIL  ${name}${extra ? '\n        ' + extra : ''}`);
	}
};
const section = (s) => console.log(`\n${s}`);
const rejects = async (fn) => {
	try {
		await fn();
		return false;
	} catch {
		return true;
	}
};
const same = (a, b) => a.length === b.length && a.every((v, i) => v === b[i]);
const bytes = (s) => new TextEncoder().encode(s);
const str = (b) => new TextDecoder().decode(b);

/** A realistic offer, the shape RTCPeerConnection actually produces. */
const SDP = [
	'v=0', 'o=- 4611731400430051336 2 IN IP4 127.0.0.1', 's=-', 't=0 0',
	'a=group:BUNDLE 0', 'a=extmap-allow-mixed', 'a=msid-semantic: WMS',
	'm=application 9 UDP/DTLS/SCTP webrtc-datachannel', 'c=IN IP4 0.0.0.0',
	'a=ice-ufrag:x9Kd', 'a=ice-pwd:Q7mV2sLp8Hc4Nw1eRt6yUi0o',
	'a=ice-options:trickle',
	'a=fingerprint:sha-256 7B:1C:6A:9E:44:D2:0F:38:A1:5C:E7:92:B3:4D:88:F0:12:6E:C9:3A:55:AB:71:0D:E4:9F:26:B8:47:C3:1A:5E',
	'a=setup:actpass', 'a=mid:0', 'a=sctp-port:5000', 'a=max-message-size:262144',
	'a=candidate:1 1 udp 2113937151 192.168.1.42 51234 typ host generation 0',
	'a=candidate:2 1 udp 1677729535 203.0.113.9 51234 typ srflx raddr 0.0.0.0 rport 0 generation 0',
	'a=end-of-candidates', ''
].join('\r\n');

// ===========================================================================
section('crypto: key derivation');
{
	check('token is 16 bytes', randomToken().length === TOKEN_BYTES);
	check('tokens differ', !same(randomToken(), randomToken()));
	check('rejects a short token', await rejects(() => deriveKey(new Uint8Array(8))));

	const t = randomToken();
	const a = new Sealer(await deriveKey(t), 'host');
	const b = new Sealer(await deriveKey(t), 'guest');
	const frame = await a.seal(bytes('same token, independent derivations'));
	check('two derivations of one token agree', str(await b.open(frame)) === 'same token, independent derivations');
}

// ===========================================================================
section('crypto: sealing');
{
	const t = randomToken();
	const host = new Sealer(await deriveKey(t), 'host');
	const guest = new Sealer(await deriveKey(t), 'guest');

	const f1 = await host.seal(bytes('one'));
	check('host -> guest', str(await guest.open(f1)) === 'one');
	const f2 = await guest.seal(bytes('two'));
	check('guest -> host', str(await host.open(f2)) === 'two');

	// The whole reason for the direction tag.
	const own = await host.seal(bytes('mine'));
	check('a peer cannot open its own frame (reflection)', await rejects(() => host.open(own)));

	// Replay: deliver f1 again after it was accepted.
	check('a replayed frame is rejected', await rejects(() => guest.open(f1)));

	const f3 = await host.seal(bytes('three'));
	const f4 = await host.seal(bytes('four'));
	await guest.open(f4);
	check('an older frame after a newer one is rejected', await rejects(() => guest.open(f3)));

	// Tamper with one ciphertext byte.
	const f5 = await host.seal(bytes('five'));
	const bad = new Uint8Array(f5);
	bad[bad.length - 1] ^= 0x01;
	check('a tampered frame is rejected', await rejects(() => guest.open(bad)));
	check('and does not advance the window past the genuine one', str(await guest.open(f5)) === 'five');

	// Wrong key entirely.
	const other = new Sealer(await deriveKey(randomToken()), 'guest');
	const f6 = await host.seal(bytes('six'));
	check('a different token cannot open the frame', await rejects(() => other.open(f6)));

	check('too-short frame rejected', await rejects(() => guest.open(new Uint8Array(20))));

	// Nonce uniqueness across many frames, both directions.
	const seen = new Set();
	for (let i = 0; i < 500; i++) {
		seen.add(toBase64Url((await host.seal(bytes('h'))).subarray(0, 12)));
		seen.add(toBase64Url((await guest.seal(bytes('g'))).subarray(0, 12)));
	}
	check('1000 nonces, 1000 distinct (no direction collision)', seen.size === 1000, `${seen.size}`);
}

// ===========================================================================
section('codec: base64url and deflate');
{
	const all = new Uint8Array(256).map((_, i) => i);
	check('base64url round-trips every byte', same(fromBase64Url(toBase64Url(all)), all));
	const enc = toBase64Url(all);
	check('no +, / or = in output', !/[+/=]/.test(enc));
	check('deflate round-trips', str(await inflate(await deflate(bytes(SDP)))) === SDP);
	const packed = await deflate(bytes(SDP));
	check(`deflate shrinks a real SDP (${SDP.length} -> ${packed.length})`, packed.length < SDP.length * 0.7);
}

// ===========================================================================
section('codec: offer');
{
	const t = randomToken();
	const code = await encodeOffer(t, SDP);
	check('starts with REACH-', code.startsWith('REACH-'));
	check(`a real offer is a pasteable code (${code.length} chars)`, code.length < 700, `${code.length}`);
	check('no whitespace in a code', !/\s/.test(code));

	const back = await decodeOffer(code);
	check('token survives', same(back.token, t));
	check('sdp survives byte for byte', back.sdp === SDP);

	// What chat apps do to long strings.
	const mangled = code.slice(0, 40) + '\n' + code.slice(40, 120) + ' \n  ' + code.slice(120) + '  \n';
	check('tolerates inserted line breaks and spaces', (await decodeOffer(mangled)).sdp === SDP);
	check('tolerates a lowercased prefix', (await decodeOffer('reach-' + code.slice(6))).sdp === SDP);

	check('rejects a missing prefix', await rejects(() => decodeOffer(code.slice(6))));
	check('rejects garbage', await rejects(() => decodeOffer('REACH-!!!not base64!!!')));
	check('rejects truncation', await rejects(() => decodeOffer(code.slice(0, 40))));
	const wrongVersion = 'REACH-' + toBase64Url(new Uint8Array([2, ...fromBase64Url(code.slice(6)).subarray(1)]));
	check('rejects an unknown version', await rejects(() => decodeOffer(wrongVersion)));
	check('rejects an offer with no fingerprint', await rejects(() => encodeOffer(t, 'v=0\r\n').then(decodeOffer)));
	check('rejects a code that is absurdly long', await rejects(() => decodeOffer('REACH-' + 'A'.repeat(10000))));
}

// ===========================================================================
section('codec: answer');
{
	const t = randomToken();
	const hostSealer = new Sealer(await deriveKey(t), 'host');
	const guestSealer = new Sealer(await deriveKey(t), 'guest');

	const code = await encodeAnswer(guestSealer, SDP);
	check('answer starts with REACH-', code.startsWith('REACH-'));
	check('host decodes the answer', (await decodeAnswer(hostSealer, code)) === SDP);

	// The proof-of-token property.
	const impostor = new Sealer(await deriveKey(randomToken()), 'guest');
	const forged = await encodeAnswer(impostor, SDP);
	check('an answer made without the token is refused', await rejects(() => decodeAnswer(hostSealer, forged)));

	// Wrong direction: a "host" cannot answer a host.
	const hostAsGuest = new Sealer(await deriveKey(t), 'host');
	const wrongWay = await encodeAnswer(hostAsGuest, SDP);
	const freshHost = new Sealer(await deriveKey(t), 'host');
	check('an answer sealed as host is refused', await rejects(() => decodeAnswer(freshHost, wrongWay)));

	// Replay of an answer already consumed.
	const t2 = randomToken();
	const h2 = new Sealer(await deriveKey(t2), 'host');
	const g2 = new Sealer(await deriveKey(t2), 'guest');
	const c2 = await encodeAnswer(g2, SDP);
	await decodeAnswer(h2, c2);
	check('a replayed answer is refused', await rejects(() => decodeAnswer(h2, c2)));

	// The answer is frame zero: the channel continues from counter 1.
	const t3 = randomToken();
	const h3 = new Sealer(await deriveKey(t3), 'host');
	const g3 = new Sealer(await deriveKey(t3), 'guest');
	await decodeAnswer(h3, await encodeAnswer(g3, SDP));
	const next = await g3.seal(bytes('first channel frame'));
	check('the same sealer carries on into the channel', str(await h3.open(next)) === 'first channel frame');

	// One error message for every failure mode.
	let m1 = '', m2 = '';
	try { await decodeAnswer(new Sealer(await deriveKey(t), 'host'), forged); } catch (e) { m1 = e.message; }
	try { await decodeAnswer(new Sealer(await deriveKey(t), 'host'), 'REACH-' + toBase64Url(new Uint8Array([1, 9, 9, 9]))); } catch (e) { m2 = e.message; }
	check('wrong token and damaged code give the same message', m1 === m2, `${m1} | ${m2}`);
}

// ===========================================================================
section('protocol');
{
	const rt = (m) => decode(encode(m));

	const hello = rt({ type: 'hello', mode: 'read-only', cols: 120, rows: 40, title: 'root@box', sharing: true });
	check('hello round-trips', hello.type === 'hello' && hello.mode === 'read-only' && hello.cols === 120 && hello.title === 'root@box' && hello.sharing === true);

	const out = rt({ type: 'out', data: bytes('\x1b[32mok\x1b[0m\r\n') });
	check('out carries raw bytes', out.type === 'out' && str(out.data) === '\x1b[32mok\x1b[0m\r\n');
	const inn = rt({ type: 'in', data: new Uint8Array([3]) });
	check('in carries raw bytes (Ctrl-C)', inn.type === 'in' && inn.data[0] === 3);
	const rs = rt({ type: 'resize', cols: 80, rows: 24 });
	check('resize round-trips', rs.type === 'resize' && rs.cols === 80 && rs.rows === 24);
	const md = rt({ type: 'mode', mode: 'read-write' });
	check('mode round-trips', md.type === 'mode' && md.mode === 'read-write');
	check('bye round-trips', rt({ type: 'bye' }).type === 'bye');

	// Hostile input from the other machine.
	const bad = (obj, code) => new Uint8Array([code, ...bytes(JSON.stringify(obj))]);
	check('rejects a hello with an invented mode', await rejects(() => decode(bad({ mode: 'admin', cols: 80, rows: 24, title: '' }, 1))));
	check('rejects absurd dimensions', await rejects(() => decode(bad({ cols: 1e9, rows: 24 }, 4))));
	check('rejects zero dimensions', await rejects(() => decode(bad({ cols: 0, rows: 24 }, 4))));
	check('rejects a mode that is not a mode', await rejects(() => decode(bad({ mode: 'yes' }, 5))));
	check('rejects an unknown type', await rejects(() => decode(new Uint8Array([99, 1, 2]))));
	check('rejects an empty frame', await rejects(() => decode(new Uint8Array(0))));
	check('title is capped', rt({ type: 'hello', mode: 'read-only', cols: 1, rows: 1, title: 'x'.repeat(5000), sharing: false }).title.length === 200);
	check('encode refuses an oversized payload', await rejects(() => Promise.resolve(encode({ type: 'out', data: new Uint8Array(MAX_PAYLOAD + 1) }))));

	const big = new Uint8Array(MAX_PAYLOAD * 2 + 7);
	const parts = chunk(big);
	check('chunk splits a large burst', parts.length === 3 && parts.reduce((n, p) => n + p.length, 0) === big.length);
	check('chunk leaves a small burst alone', chunk(new Uint8Array(10)).length === 1);
}

console.log(`\n${failures === 0 ? 'ALL PASS' : failures + ' FAILURE(S)'}`);
process.exitCode = failures ? 1 : 0;
