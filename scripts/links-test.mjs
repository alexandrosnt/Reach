/**
 * Tests which text in the terminal is a link, in Node, with no terminal.
 * Node strips TypeScript types on import, so the exact module the webview
 * runs is what is under test here.
 *
 * Run: node scripts/links-test.mjs
 */

import { findLinks } from '../src/lib/terminal/links.ts';

let failures = 0;
const eq = (name, got, want) => {
	const g = JSON.stringify(got);
	const w = JSON.stringify(want);
	if (g === w) console.log(`  PASS  ${name}`);
	else {
		failures++;
		console.log(`  FAIL  ${name}\n        got  ${g}\n        want ${w}`);
	}
};
const whole = (name, url) => eq(name, findLinks(url), [url]);
const section = (s) => console.log(`\n${s}`);

section('long links open whole (the addon cut these short)');
whole('maps link with !', 'https://www.google.com/maps/place/Athens/@37.99,23.72,12z/data=!3m1!4b1!4m6!3m5!1s0x14a1bd');
whole('parentheses in the path', 'https://en.wikipedia.org/wiki/Mercury_(planet)');
whole('parentheses mid-path', 'https://example.com/a_(b)/c');
whole('a star', 'https://example.com/path*glob/more');
whole("an apostrophe inside", "https://example.com/it's/here");
whole('a sign-in link', 'https://login.microsoftonline.com/common/oauth2/v2.0/authorize?client_id=abc&scope=openid%20profile&state=xyz&redirect_uri=https%3A%2F%2Fx');
whole('brackets in the query', 'https://example.com/a?q=[1,2]&x=y');
whole('a fragment', 'https://example.com/docs#section-2.1');
whole('a port and user', 'http://user@host.example:8080/x');
whole('a tilde in the path', 'https://example.com/~user/page');
whole('uppercase scheme', 'HTTPS://EXAMPLE.COM/A');

section('the text around a link stays out');
eq('a sentence-ending dot', findLinks('see https://example.com/a.'), ['https://example.com/a']);
eq('a comma', findLinks('https://example.com/a, then'), ['https://example.com/a']);
eq('a colon', findLinks('at https://example.com/a: done'), ['https://example.com/a']);
eq('an exclamation mark', findLinks('go to https://example.com/a!'), ['https://example.com/a']);
eq('a question mark', findLinks('is it https://example.com/a?'), ['https://example.com/a']);
eq('in parentheses', findLinks('(see https://example.com/a)'), ['https://example.com/a']);
eq('in parentheses, link with its own', findLinks('(see https://en.wikipedia.org/wiki/Mercury_(planet))'), ['https://en.wikipedia.org/wiki/Mercury_(planet)']);
eq('a Markdown link', findLinks('[docs](https://example.com/a)'), ['https://example.com/a']);
eq('in single quotes', findLinks("open 'https://example.com/a'"), ['https://example.com/a']);
eq('in double quotes', findLinks('href="https://example.com/a"'), ['https://example.com/a']);
eq('in angle brackets', findLinks('<https://example.com/a>'), ['https://example.com/a']);
eq('in square brackets', findLinks('[https://example.com/a]'), ['https://example.com/a']);
eq('two links on a line', findLinks('https://a.example/x and https://b.example/y'), ['https://a.example/x', 'https://b.example/y']);

section('not links');
eq('no scheme', findLinks('example.com/a'), []);
eq('another scheme', findLinks('ftp://example.com/a'), []);
eq('scheme alone', findLinks('https://'), []);

console.log(failures ? `\n${failures} failed` : '\nall passed');
process.exit(failures ? 1 : 0);
