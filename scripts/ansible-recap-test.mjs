// Tests for the Ansible output parser. Run: npm run ansible:test
import { strict as assert } from 'node:assert';
import { parseAnsibleLine, applyAnsibleEvent, emptyAnsibleRun, taskStatus } from '../src/lib/ansible/recap.ts';

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

const SAMPLE = `
PLAY [Configure webservers] *******************************************************

TASK [Gathering Facts] ************************************************************
ok: [web-01]
ok: [web-02]
ok: [db-01]

TASK [common : install baseline packages] *****************************************
changed: [web-01]
ok: [web-02]
changed: [db-01]

TASK [nginx : template vhost] *****************************************************
fatal: [web-02]: FAILED! => {"changed": false, "msg": "Destination directory /etc/nginx/sites-available does not exist"}
changed: [web-01]
skipping: [db-01]

TASK [nginx : ensure log dir] *****************************************************
fatal: [web-01]: FAILED! => {"changed": false, "msg": "permission denied"}
...ignoring
ok: [web-01] => (item=/var/log/nginx)
changed: [web-01 -> localhost]

TASK [postgres : ensure cluster] **************************************************
fatal: [db-02]: UNREACHABLE! => {"changed": false, "msg": "Failed to connect to the host via ssh: Connection timed out", "unreachable": true}

RUNNING HANDLER [nginx : reload nginx] ********************************************
changed: [web-01]

PLAY RECAP ************************************************************************
db-01                      : ok=2    changed=1    unreachable=0    failed=0    skipped=1    rescued=0    ignored=0
db-02                      : ok=0    changed=0    unreachable=1    failed=0    skipped=0    rescued=0    ignored=0
web-01                     : ok=6    changed=4    unreachable=0    failed=0    skipped=0    rescued=0    ignored=1
web-02                     : ok=2    changed=0    unreachable=0    failed=1    skipped=0    rescued=0    ignored=0
`;

function fold(text) {
	const view = emptyAnsibleRun();
	for (const line of text.split('\n')) {
		const e = parseAnsibleLine(line);
		if (e) applyAnsibleEvent(view, e);
	}
	return view;
}

test('a whole run folds into plays, tasks and hosts', () => {
	const v = fold(SAMPLE);
	assert.equal(v.structured, true);
	assert.equal(v.plays.length, 1);
	assert.equal(v.plays[0].name, 'Configure webservers');
	assert.equal(v.plays[0].tasks.length, 6);
	assert.equal(v.plays[0].tasks[0].name, 'Gathering Facts');
	assert.equal(v.plays[0].tasks[0].hosts.length, 3);
});

test('role and task name are split', () => {
	const v = fold(SAMPLE);
	const t = v.plays[0].tasks[1];
	assert.equal(t.role, 'common');
	assert.equal(t.name, 'install baseline packages');
	assert.equal(v.plays[0].tasks[0].role, null);
});

test('a fatal FAILED result carries its msg', () => {
	const v = fold(SAMPLE);
	const h = v.plays[0].tasks[2].hosts.find((x) => x.host === 'web-02');
	assert.equal(h.status, 'failed');
	assert.match(h.msg, /sites-available/);
});

test('...ignoring forgives the failure before it', () => {
	const v = fold(SAMPLE);
	const t = v.plays[0].tasks[3];
	assert.equal(t.hosts[0].status, 'ignored');
	assert.equal(t.hosts[0].host, 'web-01');
});

test('loop items and delegation parse', () => {
	const v = fold(SAMPLE);
	const t = v.plays[0].tasks[3];
	assert.equal(t.hosts[1].item, '/var/log/nginx');
	assert.equal(t.hosts[1].status, 'ok');
	assert.equal(t.hosts[2].host, 'web-01', 'delegate is dropped, host kept');
	assert.equal(t.hosts[2].status, 'changed');
});

test('UNREACHABLE is its own status', () => {
	const v = fold(SAMPLE);
	const h = v.plays[0].tasks[4].hosts[0];
	assert.equal(h.status, 'unreachable');
	assert.match(h.msg, /timed out/);
});

test('handlers are tasks flagged as such', () => {
	const v = fold(SAMPLE);
	const t = v.plays[0].tasks[5];
	assert.equal(t.handler, true);
	assert.equal(t.role, 'nginx');
	assert.equal(t.name, 'reload nginx');
});

test('the recap table', () => {
	const v = fold(SAMPLE);
	assert.equal(v.recap.length, 4);
	const w1 = v.recap.find((r) => r.host === 'web-01');
	assert.deepEqual(w1, { host: 'web-01', ok: 6, changed: 4, unreachable: 0, failed: 0, skipped: 0, rescued: 0, ignored: 1 });
});

test('task status is the worst host status', () => {
	const v = fold(SAMPLE);
	const t = v.plays[0].tasks;
	assert.equal(taskStatus(t[0]), 'ok');
	assert.equal(taskStatus(t[1]), 'changed');
	assert.equal(taskStatus(t[2]), 'failed');
	assert.equal(taskStatus(t[3]), 'changed', 'ignored is not failed');
	assert.equal(taskStatus(t[4]), 'unreachable');
	assert.equal(taskStatus({ name: 'x', role: null, handler: false, hosts: [] }), 'pending');
});

test('warnings and errors are kept, prose is not', () => {
	const v = fold('[WARNING]: No inventory was parsed, only implicit localhost is available\nERROR! the playbook: site.yml could not be found\nsome other line\n');
	assert.equal(v.structured, false);
	assert.deepEqual(v.warnings, ['No inventory was parsed, only implicit localhost is available']);
	assert.deepEqual(v.errors, ['the playbook: site.yml could not be found']);
});

test('an older recap without rescued/ignored still parses', () => {
	const e = parseAnsibleLine('host1                      : ok=3    changed=1    unreachable=0    failed=0');
	assert.equal(e.type, 'recap');
	assert.equal(e.row.skipped, 0);
	assert.equal(e.row.ignored, 0);
});

test('a result before any task is dropped, not crashed on', () => {
	const v = emptyAnsibleRun();
	applyAnsibleEvent(v, parseAnsibleLine('ok: [h]'));
	assert.equal(v.plays.length, 0);
});

test('a non-JSON result keeps its text as the message', () => {
	const e = parseAnsibleLine('fatal: [h]: FAILED! => something went wrong');
	assert.equal(e.msg, 'something went wrong');
});

console.log(`${passed} passed${process.exitCode ? ', with failures' : ''}`);
