// Tests for sessions → inventory hosts. Run: npm run ansible:test
import { strict as assert } from 'node:assert';
import { hostFromSession, inventoryName, uniqueName, alreadyInInventory, missingGroups } from '../src/lib/ansible/from-sessions.ts';

let passed = 0;
function test(name, fn) {
	try {
		fn();
		passed++;
	} catch (e) {
		console.error(`✗ ${name}\n  ${e.message}`);
		process.exitCode = 1;
	}
}

const key = (over = {}) => ({
	id: 's1', name: 'prod web 01', host: '10.0.0.11', port: 22, username: 'root',
	auth_method: { type: 'Key', path: 'C:\\Users\\me\\.ssh\\id_ed25519' }, folder_id: null, tags: ['prod', 'web servers'], vault_id: null, ...over
});

test('names become inventory words', () => {
	assert.equal(inventoryName('prod web 01'), 'prod-web-01');
	assert.equal(inventoryName('  Clients — Acme / db  '), 'Clients-Acme-db');
	assert.equal(inventoryName('!!!'), 'host');
});

test('unique names suffix, never clobber', () => {
	const taken = new Set(['web', 'web-2']);
	assert.equal(uniqueName('web', taken), 'web-3');
	assert.equal(uniqueName('db', taken), 'db');
});

test('a key session carries its key and its tags become groups', () => {
	const { host, note } = hostFromSession(key(), []);
	assert.equal(host.name, 'prod-web-01');
	assert.equal(host.ansibleHost, '10.0.0.11');
	assert.equal(host.ansiblePort, null, 'default port is left implicit');
	assert.equal(host.ansibleUser, 'root');
	assert.equal(host.variables.ansible_ssh_private_key_file, 'C:\\Users\\me\\.ssh\\id_ed25519');
	assert.deepEqual(host.groups, ['prod', 'web-servers']);
	assert.equal(note, null);
});

test('a password session gets no credential and says so', () => {
	const { host, note } = hostFromSession(key({ auth_method: { type: 'Password', password: 'hunter2' } }), []);
	assert.equal(note, 'password');
	assert.equal(JSON.stringify(host).includes('hunter2'), false, 'the password never reaches the inventory');
	assert.equal(host.variables.ansible_password, undefined);
});

test('a non-default port is kept', () => {
	const { host } = hostFromSession(key({ port: 2222 }), []);
	assert.equal(host.ansiblePort, 2222);
});

test('the first jump hop becomes ProxyJump', () => {
	const { host } = hostFromSession(key({ jump_chain: [{ host: 'bastion.example', port: 2200, username: 'jump', auth_method: { type: 'Key' } }] }), []);
	assert.equal(host.variables.ansible_ssh_common_args, '-o ProxyJump=jump@bastion.example:2200');
	const { host: h2 } = hostFromSession(key({ jump_chain: [{ host: 'b', port: 22, username: 'j', auth_method: { type: 'Key' } }] }), []);
	assert.equal(h2.variables.ansible_ssh_common_args, '-o ProxyJump=j@b');
});

test('already-present hosts are recognised by address, user and port', () => {
	const existing = [{ name: 'x', ansibleHost: '10.0.0.11', ansiblePort: null, ansibleUser: 'root', groups: [], variables: {} }];
	assert.equal(alreadyInInventory(key(), existing), true);
	assert.equal(alreadyInInventory(key({ username: 'alex' }), existing), false);
	assert.equal(alreadyInInventory(key({ port: 2222 }), existing), false);
});

test('missing groups are created once', () => {
	const hosts = [hostFromSession(key(), []).host, hostFromSession(key({ name: 'db', tags: ['prod', 'db'] }), []).host];
	const groups = missingGroups(hosts, [{ name: 'prod', variables: {}, children: [] }]);
	assert.deepEqual(groups.map((g) => g.name), ['web-servers', 'db']);
});

console.log(`${passed} passed${process.exitCode ? ', with failures' : ''}`);
