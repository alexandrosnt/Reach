/**
 * Tests reading RDP hosts out of pasted JSON for the bulk importer, in Node.
 *
 * Node 24 strips TypeScript types on import, so the exact module the webview
 * runs is what is under test here.
 *
 * Run: node scripts/rdp-import-test.mjs
 */

import { parseRdpHosts, parsePort, findExisting, findFolder } from '../src/lib/sessions/rdp-import.ts';

let failures = 0;
const check = (name, cond, extra = '') => {
	if (cond) console.log(`  PASS  ${name}`);
	else {
		failures++;
		console.log(`  FAIL  ${name}${extra ? '\n        ' + extra : ''}`);
	}
};
const section = (s) => console.log(`\n${s}`);
const parse = (value) => parseRdpHosts(JSON.stringify(value));

section('input shapes');
{
	const r = parseRdpHosts('{ not json');
	check('broken JSON is reported, with the reason', !r.ok && r.error === 'bad_json' && r.detail.length > 0);
	check('a JSON object without hosts finds nothing', parse({ foo: 1 }).error === 'no_hosts');
	check('an empty array finds nothing', parse([]).error === 'no_hosts');
	check('a string finds nothing', parse('hello').error === 'no_hosts');
}

section('plain array');
{
	const r = parse([{ name: 'DC01', ip: '10.0.0.10', port: 3389, username: 'admin', domain: 'CORP', folder: 'HQ' }]);
	const h = r.ok && r.hosts[0];
	check('a full entry is read', r.ok && r.hosts.length === 1);
	check('every field comes across', h && h.name === 'DC01' && h.host === '10.0.0.10' && h.port === 3389 && h.username === 'admin' && h.domain === 'CORP' && h.folder === 'HQ');
	const m = parse([{ host: ' pc.example.com ' }]);
	const mh = m.ok && m.hosts[0];
	check('host is accepted for ip, trimmed', mh && mh.host === 'pc.example.com');
	check('a missing name falls back to the host', mh && mh.name === 'pc.example.com');
	check('a missing port is 3389', mh && mh.port === 3389);
	check('missing optional fields are empty, not "undefined"', mh && mh.username === '' && mh.domain === null && mh.folder === null);
	const s = parse([{ name: 'S', ip: '10.0.0.1', port: '3390' }]);
	check('a port given as a string is read', s.ok && s.hosts[0].port === 3390);
}

section('Termix export');
{
	const r = parse({
		hosts: [
			{ connectionType: 'ssh', name: 'web', ip: '10.0.0.2', port: 22, username: 'root', password: 'secret' },
			{ connectionType: 'rdp', name: 'desk', ip: '10.0.0.3', port: 3389, username: 'me', password: 'secret', folder: 'Office' },
			{ connectionType: 'vnc', name: 'pi', ip: '10.0.0.4', port: 5900 },
			{ connectionType: 'rdp', name: 'alt', ip: '10.0.0.5', pluginSettings: { 'remote-desktop': { rdpPort: 3391 } } }
		]
	});
	check('only RDP hosts are taken', r.ok && r.hosts.map((h) => h.name).join() === 'desk,alt');
	check('no password reaches the result', r.ok && !JSON.stringify(r.hosts).includes('secret'));
	check("a port from Termix's RDP plugin settings is used", r.ok && r.hosts[1].port === 3391);
	check('an export with no RDP hosts finds nothing', parse({ hosts: [{ connectionType: 'ssh', ip: 'x' }] }).error === 'no_hosts');
}

section('bad entries are skipped, not fatal');
{
	const r = parse([
		{ name: 'ok', ip: '10.0.0.1' },
		{ name: 'noaddr' },
		{ name: 'big', ip: '10.0.0.2', port: 70000 },
		{ name: 'zero', ip: '10.0.0.3', port: 0 },
		{ name: 'text', ip: '10.0.0.4', port: 'abc' },
		{ name: 'frac', ip: '10.0.0.5', port: 3389.5 },
		null,
		42
	]);
	check('the good host is kept', r.ok && r.hosts.length === 1 && r.hosts[0].name === 'ok');
	check('a host without an address is skipped as no_host', r.ok && r.skipped.some((s) => s.label === 'noaddr' && s.reason === 'no_host'));
	check('ports out of range or not whole are skipped as bad_port', r.ok && ['big', 'zero', 'text', 'frac'].every((n) => r.skipped.some((s) => s.label === n && s.reason === 'bad_port')));
	check('entries that are not objects are ignored', r.ok && r.skipped.length === 5);
	const only = parse([{ name: 'big', ip: '10.0.0.2', port: 70000 }]);
	check('all entries bad still lists why', only.ok && only.hosts.length === 0 && only.skipped.length === 1);
	check('an object as a name does not become "[object Object]"', parse([{ name: { a: 1 }, ip: '10.0.0.9' }]).hosts[0].name === '10.0.0.9');
}

section('ports');
check('1 and 65535 are valid', parsePort(1) === 1 && parsePort(65535) === 65535);
check('65536 and -1 are not', parsePort(65536) === null && parsePort(-1) === null);
check('absent means 3389', parsePort(undefined) === 3389 && parsePort(null) === 3389 && parsePort('') === 3389);

section('already saved');
{
	const host = { id: '0', name: 'desk', host: 'PC.example.com', port: 3389, username: 'me', domain: null, folder: null };
	const rdp = { kind: 'rdp', host: 'pc.example.com', port: 3389, username: 'me' };
	check('the same RDP host and user is found, address case ignored', findExisting(host, [rdp]) === rdp);
	check('an SSH session to the same address is not a match', findExisting(host, [{ ...rdp, kind: 'ssh' }]) === undefined);
	check('another port is not a match', findExisting(host, [{ ...rdp, port: 3390 }]) === undefined);
	check('another user is not a match', findExisting(host, [{ ...rdp, username: 'admin' }]) === undefined);
}

section('folders');
{
	const folders = [
		{ id: 'a', name: 'HQ', parent_id: null, vault_id: null },
		{ id: 'b', name: 'HQ', parent_id: null, vault_id: 'team' },
		{ id: 'c', name: 'Lab', parent_id: 'a', vault_id: null }
	];
	check('a folder of that name in the same vault is reused', findFolder('HQ', null, folders)?.id === 'a');
	check('the vault decides which one', findFolder('HQ', 'team', folders)?.id === 'b');
	check('a nested folder of that name is not reused', findFolder('Lab', null, folders) === undefined);
	check('no folder of that name means a new one', findFolder('Branch', null, folders) === undefined);
}

console.log(failures ? `\n${failures} failed` : '\nall passed');
process.exit(failures ? 1 : 0);
