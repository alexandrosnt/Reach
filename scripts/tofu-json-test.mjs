// Tests for the OpenTofu machine-readable UI parser. Run: npm run tofu:test
import { strict as assert } from 'node:assert';
import { parseTofuUiLine, applyTofuUiMessage, emptyRunView, splitLines } from '../src/lib/tofu/ui-json.ts';

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

const res = {
	addr: 'aws_instance.web[0]',
	module: '',
	resource: 'aws_instance.web[0]',
	implied_provider: 'aws',
	resource_type: 'aws_instance',
	resource_name: 'web',
	resource_key: 0
};
const base = { '@level': 'info', '@module': 'tofu.ui', '@timestamp': '2026-09-11T10:00:00Z' };
const line = (o) => JSON.stringify({ ...base, ...o });

test('prose is not a message', () => {
	assert.equal(parseTofuUiLine('Initializing the backend...'), null);
	assert.equal(parseTofuUiLine(''), null);
	assert.equal(parseTofuUiLine('{not json'), null);
	assert.equal(parseTofuUiLine('{"no":"type"}'), null);
});

test('version', () => {
	const m = parseTofuUiLine(line({ type: 'version', '@message': 'OpenTofu 1.11.4', tofu: '1.11.4', ui: '1.2' }));
	assert.deepEqual(m, { type: 'version', tofu: '1.11.4', ui: '1.2' });
});

test('version from terraform names the field differently', () => {
	const m = parseTofuUiLine(line({ type: 'version', terraform: '1.9.0', ui: '1.2' }));
	assert.equal(m.tofu, '1.9.0');
});

test('planned_change carries the resource and action', () => {
	const m = parseTofuUiLine(line({ type: 'planned_change', '@message': 'aws_instance.web[0]: Plan to create', change: { resource: res, action: 'create' } }));
	assert.equal(m.type, 'planned_change');
	assert.equal(m.action, 'create');
	assert.equal(m.resource.addr, 'aws_instance.web[0]');
	assert.equal(m.resource.resourceType, 'aws_instance');
	assert.equal(m.resource.resourceKey, 0);
	assert.equal(m.reason, null);
});

test('planned_change with a replace reason', () => {
	const m = parseTofuUiLine(line({ type: 'planned_change', change: { resource: res, action: 'replace', reason: 'cannot_update' } }));
	assert.equal(m.action, 'replace');
	assert.equal(m.reason, 'cannot_update');
});

test('unknown action is noop, not a throw', () => {
	const m = parseTofuUiLine(line({ type: 'planned_change', change: { resource: res, action: 'forget' } }));
	assert.equal(m.action, 'noop');
});

test('change_summary', () => {
	const m = parseTofuUiLine(line({ type: 'change_summary', '@message': 'Plan: 3 to add, 1 to change, 1 to destroy.', changes: { add: 3, change: 1, remove: 1, import: 0, operation: 'plan' } }));
	assert.deepEqual(m.changes, { add: 3, change: 1, remove: 1, import: 0, operation: 'plan' });
});

test('diagnostic with a source range becomes file:line', () => {
	const m = parseTofuUiLine(line({
		type: 'diagnostic', '@level': 'error', '@message': 'Error: Reference to undeclared input variable',
		diagnostic: { severity: 'error', summary: 'Reference to undeclared input variable', detail: 'An input variable with the name "db_password" has not been declared.', range: { filename: 'main.tf', start: { line: 42, column: 12, byte: 900 }, end: { line: 42, column: 28, byte: 916 } } }
	}));
	assert.equal(m.type, 'diagnostic');
	assert.equal(m.diagnostic.severity, 'error');
	assert.equal(m.diagnostic.location, 'main.tf:42');
	assert.match(m.diagnostic.detail, /db_password/);
});

test('diagnostic without a range has no location', () => {
	const m = parseTofuUiLine(line({ type: 'diagnostic', '@level': 'warning', diagnostic: { severity: 'warning', summary: 'Deprecated', detail: '' } }));
	assert.equal(m.diagnostic.location, null);
});

test('apply lifecycle folds into one row', () => {
	const view = emptyRunView();
	const msgs = [
		line({ type: 'apply_start', hook: { resource: res, action: 'create' } }),
		line({ type: 'apply_progress', hook: { resource: res, action: 'create', elapsed_seconds: 10 } }),
		line({ type: 'apply_complete', hook: { resource: res, action: 'create', id_key: 'id', id_value: 'i-0abc', elapsed_seconds: 12 } })
	];
	for (const l of msgs) applyTofuUiMessage(view, parseTofuUiLine(l));
	assert.equal(view.apply.length, 1, 'three messages, one row');
	assert.equal(view.apply[0].state, 'done');
	assert.equal(view.apply[0].elapsed, 12);
	assert.equal(view.apply[0].id, 'i-0abc');
	assert.equal(view.structured, true);
});

test('apply_errored marks the row', () => {
	const view = emptyRunView();
	applyTofuUiMessage(view, parseTofuUiLine(line({ type: 'apply_start', hook: { resource: res, action: 'delete' } })));
	applyTofuUiMessage(view, parseTofuUiLine(line({ type: 'apply_errored', hook: { resource: res, action: 'delete', elapsed_seconds: 3 } })));
	assert.equal(view.apply[0].state, 'errored');
	assert.equal(view.apply[0].action, 'delete');
});

test('outputs', () => {
	const m = parseTofuUiLine(line({ type: 'outputs', outputs: { ip: { sensitive: false, type: 'string', value: '1.2.3.4' }, pw: { sensitive: true, type: 'string', value: 'x' } } }));
	assert.equal(m.outputs.ip.value, '1.2.3.4');
	assert.equal(m.outputs.pw.sensitive, true);
});

test('refreshes are counted, not listed', () => {
	const view = emptyRunView();
	for (let i = 0; i < 5; i++) {
		applyTofuUiMessage(view, parseTofuUiLine(line({ type: 'refresh_start', hook: { resource: res } })));
		applyTofuUiMessage(view, parseTofuUiLine(line({ type: 'refresh_complete', hook: { resource: res, id_key: 'id', id_value: 'i-1' } })));
	}
	assert.equal(view.refreshed, 5);
	assert.equal(view.apply.length, 0);
});

test('resource_drift', () => {
	const m = parseTofuUiLine(line({ type: 'resource_drift', change: { resource: res, action: 'update' } }));
	assert.equal(m.type, 'resource_drift');
	assert.equal(m.action, 'update');
});

test('a known type with a broken payload degrades to log', () => {
	const m = parseTofuUiLine(line({ type: 'planned_change', '@message': 'odd', change: 'not an object' }));
	assert.equal(m.type, 'log');
	assert.equal(m.message, 'odd');
});

test('an unknown type is still a log line, so nothing is lost', () => {
	const m = parseTofuUiLine(line({ type: 'test_summary', '@message': 'Success! 3 passed.' }));
	assert.equal(m.type, 'log');
	assert.equal(m.message, 'Success! 3 passed.');
});

test('splitLines carries a partial tail across chunks', () => {
	let { lines, carry } = splitLines('', '{"a":1}\n{"b":');
	assert.deepEqual(lines, ['{"a":1}']);
	assert.equal(carry, '{"b":');
	({ lines, carry } = splitLines(carry, '2}\r\n'));
	assert.deepEqual(lines, ['{"b":2}']);
	assert.equal(carry, '');
});

console.log(`${passed} passed${process.exitCode ? ', with failures' : ''}`);
