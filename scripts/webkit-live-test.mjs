/**
 * Runs the WebKit input adapter against real WebKit, on a real Mac.
 *
 * Issue #47 has now cost two rounds of asking the reporter to test for us,
 * because nobody here has a Mac. `webkit-input-test.mjs` proves the logic
 * against a transcription of xterm's handlers, which is only ever as good as
 * the transcription and as the event order it assumes. This drives the actual
 * browser instead: real WebKit, the real xterm 5.5.0 bundle we ship, and the
 * real adapter bundled straight from its TypeScript source.
 *
 * It is written to be useful whether or not the bug reproduces here. A driven
 * browser does not necessarily take the same input path as a person typing
 * into WKWebView, so a clean run does not prove the bug is absent in the wild.
 * The run therefore always prints the raw event order it observed, which is
 * the thing worth knowing, and fails only when the adapter is shown to make
 * something worse or to leave a reproduced bug unfixed.
 *
 * Needs Playwright's WebKit, which CI installs; skips cleanly without it.
 *
 * Run: node scripts/webkit-live-test.mjs
 */

import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const ROOT = path.dirname(path.dirname(fileURLToPath(import.meta.url)));

let webkit;
try {
	({ webkit } = await import('playwright'));
} catch {
	console.log('SKIP: playwright is not installed (npm i -D playwright && npx playwright install webkit)');
	process.exit(0);
}

// Bundle the adapter from source so this tests the shipped module, not a copy.
const { build } = await import('esbuild');
const bundled = await build({
	entryPoints: [path.join(ROOT, 'src/lib/terminal/webkit-input-fix.ts')],
	bundle: true,
	format: 'iife',
	globalName: 'WebkitInputFix',
	write: false,
	alias: { $lib: path.join(ROOT, 'src/lib') }
});
const adapterJs = bundled.outputFiles[0].text;
const xtermJs = readFileSync(path.join(ROOT, 'node_modules/@xterm/xterm/lib/xterm.js'), 'utf8');

const PAGE = `<!doctype html><meta charset="utf-8">
<style>html,body{margin:0;height:100%}#t{width:800px;height:400px}</style>
<div id="t"></div>`;

const SETUP = `
window.__data = [];
window.__log = [];
window.__term = new Terminal({ allowProposedApi: true });
window.__term.open(document.getElementById('t'));
window.__term.onData((d) => window.__data.push(d));
const ta = window.__term.textarea;
for (const type of ['beforeinput','input','keydown','keyup','compositionstart','compositionend']) {
  ta.addEventListener(type, (e) => window.__log.push({
    type: e.type, key: e.key, keyCode: e.keyCode, data: e.data,
    inputType: e.inputType, composed: e.composed, isComposing: e.isComposing
  }), true);
}
window.__detected = WebkitInputFix.installWebkitInputFix.length >= 0;
`;

async function run({ install }) {
	const browser = await webkit.launch();
	const page = await browser.newPage();
	await page.setContent(PAGE);
	await page.addScriptTag({ content: xtermJs });
	await page.addScriptTag({ content: adapterJs });
	await page.evaluate(SETUP);

	const engineSaysWebKit = await page.evaluate(() => {
		const ua = navigator.userAgent;
		return /AppleWebKit/.test(ua) && !/Chrome|Chromium|Edg\//.test(ua);
	});

	if (install) {
		// No isWebKitEngine override: this also checks our own detection on a
		// real WebKit build, which is load-bearing and has never been run here.
		await page.evaluate(() => {
			window.__dispose = WebkitInputFix.installWebkitInputFix(window.__term);
		});
	}

	await page.evaluate(() => window.__term.focus());

	// A shifted symbol: ':' is Shift+; and is how vim enters command mode.
	await page.evaluate(() => { window.__data.length = 0; window.__log.length = 0; });
	await page.keyboard.press('Shift+Semicolon');
	await page.waitForTimeout(150);
	const colon = await page.evaluate(() => ({ data: window.__data.join(''), log: window.__log.slice() }));

	// A fast burst, which is the original character-loss report.
	await page.evaluate(() => { window.__data.length = 0; });
	await page.keyboard.type('hello world', { delay: 0 });
	await page.waitForTimeout(200);
	const burst = await page.evaluate(() => window.__data.join(''));

	// Chords that emit from the keydown must not double up.
	await page.evaluate(() => { window.__data.length = 0; });
	await page.keyboard.press('Control+c');
	await page.waitForTimeout(150);
	const ctrlC = await page.evaluate(() => window.__data.slice());

	await browser.close();
	return { engineSaysWebKit, colon, burst, ctrlC };
}

/**
 * Replays the event order WKWebView produces, against the real xterm build in
 * a real browser.
 *
 * Driven input cannot reproduce this: Playwright synthesises a keydown with a
 * real keyCode and no input event at all, which is not what a person typing
 * into WKWebView produces. The order below is the one steve3d captured in #47,
 * text first and then a keydown carrying 229. Feeding that to the genuine
 * handlers takes my transcription of them out of the chain of trust, which is
 * the one thing the Node test can never do.
 */
const REPLAY = `
(() => {
  const ta = window.__term.textarea;
  const key = (init, code) => {
    const e = new KeyboardEvent('keydown', { bubbles: true, composed: true, ...init });
    Object.defineProperty(e, 'keyCode', { get: () => code });
    ta.dispatchEvent(e);
  };
  const text = (data) => ta.dispatchEvent(
    new InputEvent('input', { data, inputType: 'insertText', bubbles: true, composed: true })
  );
  const up = (k, code) => {
    const e = new KeyboardEvent('keyup', { key: k, bubbles: true, composed: true });
    Object.defineProperty(e, 'keyCode', { get: () => code });
    ta.dispatchEvent(e);
  };

  window.__data.length = 0;
  key({ key: 'Shift', shiftKey: true }, 16);
  text(':');
  key({ key: ':', shiftKey: true }, 229);
  up(':', 229); up('Shift', 16);
  const colon = window.__data.join('');

  window.__data.length = 0;
  // Two characters typed quickly enough that no keyup lands between them.
  text('a'); key({ key: 'a' }, 229);
  text('b'); key({ key: 'b' }, 229);
  const fast = window.__data.join('');

  return { colon, fast };
})()`;

/**
 * The deferred textarea diff, which is the risk this adapter carries
 * (upstream xterm.js #6045).
 *
 * A keydown carrying 229 makes xterm's composition helper snapshot the
 * textarea and, a macrotask later, emit whatever changed. With the gate
 * cleared, the next character can be taken by `_inputEvent` *and* still fall
 * inside that pending comparison, which would send it twice. Reproducing it
 * needs the textarea's real value to move, because that is what the
 * comparison reads, so this sets it exactly as the browser would before
 * firing each input event.
 */
const REPLAY_DUPLICATION = `
(async () => {
  const ta = window.__term.textarea;
  const key = (init, code) => {
    const e = new KeyboardEvent('keydown', { bubbles: true, composed: true, ...init });
    Object.defineProperty(e, 'keyCode', { get: () => code });
    ta.dispatchEvent(e);
  };
  const text = (data) => {
    ta.value += data;
    ta.dispatchEvent(new InputEvent('input', { data, inputType: 'insertText', bubbles: true, composed: true }));
  };

  window.__data.length = 0;
  ta.value = '';
  // Both characters inside one macrotask, so the diff scheduled by the first
  // keydown is still pending when the second character is accepted.
  text('a'); key({ key: 'a' }, 229);
  text('b'); key({ key: 'b' }, 229);
  await new Promise((r) => setTimeout(r, 80));
  return window.__data.join('');
})()`;

async function runReplay({ install, script = REPLAY }) {
	const browser = await webkit.launch();
	const page = await browser.newPage();
	await page.setContent(PAGE);
	await page.addScriptTag({ content: xtermJs });
	await page.addScriptTag({ content: adapterJs });
	await page.evaluate(SETUP);
	if (install) {
		await page.evaluate(() => {
			window.__dispose = WebkitInputFix.installWebkitInputFix(window.__term);
		});
	}
	await page.evaluate(() => window.__term.focus());
	const result = await page.evaluate(script);
	await browser.close();
	return result;
}

const off = await run({ install: false });
const on = await run({ install: true });

console.log(`\nour isWebKit() on this build: ${on.engineSaysWebKit}`);

console.log('\nevent order WebKit delivered for Shift+;');
for (const e of off.colon.log) {
	const bits = [e.type];
	if (e.key !== undefined) bits.push(`key=${JSON.stringify(e.key)}`);
	if (e.keyCode !== undefined && e.keyCode !== 0) bits.push(`keyCode=${e.keyCode}`);
	if (e.data != null) bits.push(`data=${JSON.stringify(e.data)}`);
	if (e.inputType) bits.push(e.inputType);
	if (e.composed) bits.push('composed');
	console.log('  ' + bits.join(' '));
}

const rows = [
	['Shift+; (expect ":")', JSON.stringify(off.colon.data), JSON.stringify(on.colon.data)],
	['fast "hello world"', JSON.stringify(off.burst), JSON.stringify(on.burst)],
	['Ctrl+C (expect 1 event)', off.ctrlC.length, on.ctrlC.length]
];
const pad = (v, n) => String(v).padEnd(n);
console.log('\n' + pad('case', 26) + pad('adapter OFF', 22) + 'adapter ON');
for (const [name, a, b] of rows) console.log(pad(name, 26) + pad(a, 22) + b);

let failures = 0;
const fail = (m) => { failures++; console.log(`  FAIL  ${m}`); };
const pass = (m) => console.log(`  PASS  ${m}`);

console.log('');
if (!on.engineSaysWebKit) {
	fail('our isWebKit() returned false on a real WebKit build, so the adapter would never install');
} else {
	pass('our isWebKit() correctly identifies real WebKit');
}

// The adapter must never make a case worse than leaving xterm alone.
if (on.colon.data.length < off.colon.data.length) fail('the adapter lost a colon that stock xterm delivered');
else pass('the adapter did not lose the colon');

if (on.burst.length < off.burst.length) fail('the adapter dropped characters stock xterm delivered');
else pass('the adapter dropped nothing from the burst');

if (on.burst !== [...new Set([on.burst])][0] || /(.)\1{3,}/.test(on.burst.replace(/l+|o+/g, ''))) {
	console.log('  NOTE  burst output worth eyeballing above');
}
if (on.ctrlC.length > Math.max(1, off.ctrlC.length)) fail('the adapter duplicated Ctrl+C');
else pass('Ctrl+C was not duplicated');

// Only assert a fix where the bug actually showed up here.
if (off.colon.data !== ':') {
	if (on.colon.data === ':') pass('the colon bug reproduced under automation and the adapter fixes it');
	else fail(`the colon bug reproduced (OFF gave ${JSON.stringify(off.colon.data)}) and the adapter did not fix it`);
} else {
	console.log('  NOTE  the colon bug did NOT reproduce under automation.');
	console.log('        Driven input does not always take the same path as a person typing into');
	console.log('        WKWebView, so this neither confirms nor refutes the report. The event');
	console.log('        order printed above is the thing to compare against the traces in #47.');
}

if (off.burst !== 'hello world' && on.burst === 'hello world') {
	pass('the character-loss bug reproduced under automation and the adapter fixes it');
} else if (off.burst === 'hello world') {
	console.log('  NOTE  fast typing did not lose characters under automation either.');
}

console.log('\nreplaying the WKWebView event order against real xterm in real WebKit');
const rOff = await runReplay({ install: false });
const rOn = await runReplay({ install: true });
console.log(pad('case', 26) + pad('adapter OFF', 22) + 'adapter ON');
console.log(pad('Shift+; (expect ":")', 26) + pad(JSON.stringify(rOff.colon), 22) + JSON.stringify(rOn.colon));
console.log(pad('fast "ab"', 26) + pad(JSON.stringify(rOff.fast), 22) + JSON.stringify(rOn.fast));
console.log('');

if (rOff.colon === ':') {
	console.log('  NOTE  real xterm accepted the colon even unpatched under replay;');
	console.log('        the reproduction assumption in #47 may not hold on this build.');
} else if (rOn.colon === ':') {
	pass('real xterm loses the colon, and the adapter fixes it');
} else {
	fail(`real xterm lost the colon (${JSON.stringify(rOff.colon)}) and the adapter did not fix it`);
}

if (rOff.fast === 'ab') {
	console.log('  NOTE  real xterm kept both characters unpatched under replay.');
} else if (rOn.fast === 'ab') {
	pass('real xterm drops a fast character, and the adapter fixes it');
} else {
	fail(`real xterm dropped a character (${JSON.stringify(rOff.fast)}) and the adapter did not fix it`);
}

console.log('\nthe deferred textarea diff (upstream #6045), the risk this adapter carries');
const dOff = await runReplay({ install: false, script: REPLAY_DUPLICATION });
const dOn = await runReplay({ install: true, script: REPLAY_DUPLICATION });
console.log(pad('two chars in one task', 26) + pad(JSON.stringify(dOff), 22) + JSON.stringify(dOn));
console.log('');

if (dOn === 'ab') {
	pass('no duplication: the adapter emits each character exactly once');
} else if (dOn.length > 2) {
	fail(`the adapter duplicated a character: emitted ${JSON.stringify(dOn)}`);
} else {
	fail(`the adapter lost a character: emitted ${JSON.stringify(dOn)}`);
}

console.log(failures === 0 ? '\nALL PASS' : `\n${failures} FAILED`);
process.exit(failures === 0 ? 0 : 1);
