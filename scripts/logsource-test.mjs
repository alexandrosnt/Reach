/**
 * Tests which saved log files a container is offered (issue #101).
 *
 * Node 24 strips TypeScript types on import, so the exact module the webview
 * runs is what is under test here.
 *
 * Run: node scripts/logsource-test.mjs
 */

import { applies, expand, templated, sourceLabel, sourcesFor, upsert, remove, size, rotationStem, groupRotated } from '../src/lib/devops/logsource.ts';

let failures = 0;
const check = (name, cond, extra = '') => {
	if (cond) console.log(`  PASS  ${name}`);
	else {
		failures++;
		console.log(`  FAIL  ${name}${extra ? '\n        ' + extra : ''}`);
	}
};
const eq = (a, b) => JSON.stringify(a) === JSON.stringify(b);

const api = { name: 'shop-api-1', project: 'shop', service: 'api' };
const worker = { name: 'shop-worker-1', project: 'shop', service: 'worker' };
const lone = { name: 'redis' };
const pod = { name: 'api-7d9f-x2', namespace: 'prod', container: 'api' };

const svc = { scope: { kind: 'service', project: 'shop', service: 'api' }, paths: ['/logs/api.INFO'] };
const byName = { scope: { kind: 'container', name: 'redis' }, paths: ['/data/redis.log'] };
const host = { scope: { kind: 'host' }, paths: ['/logs/{service}.WARNING', '/logs/{service}.ERROR'] };
const podRule = { scope: { kind: 'podContainer', namespace: 'prod', container: 'api' }, paths: ['/var/log/app.log'] };

console.log('\nscopes');
check('a service rule fits that service, not another', applies(svc, api) && !applies(svc, worker) && !applies(svc, lone));
check('a service rule survives a recreated container (new name, same service)', applies(svc, { ...api, name: 'shop-api-2' }));
check('a container rule fits by name only', applies(byName, lone) && !applies(byName, api));
check('a pod rule fits its namespace and container name', applies(podRule, pod) && !applies(podRule, { ...pod, namespace: 'dev' }));
check('a host rule fits everything', [api, worker, lone, pod].every((t) => applies(host, t)));

console.log('\ntemplates');
check('{service} becomes the service', expand('/logs/{service}.INFO', api) === '/logs/api.INFO');
check('without a service, the container name', expand('/logs/{service}.INFO', lone) === '/logs/redis.INFO');
check('in a pod, the container name', expand('/logs/{service}.INFO', pod) === '/logs/api.INFO');
check('a path made general', templated('/logs/api.INFO', api) === '/logs/{service}.INFO');
check('nothing to generalise without a service', templated('/data/redis.log', lone) === '/data/redis.log');

console.log('\nwhat the picker offers');
const offered = sourcesFor([host, svc], api);
check('specific first, then the host-wide one', eq(offered.map((s) => s.label), ['api.INFO', 'api.WARNING + api.ERROR']), JSON.stringify(offered.map((s) => s.label)));
check('host-wide paths filled in', eq(offered[1].paths, ['/logs/api.WARNING', '/logs/api.ERROR']));
check('the same files twice are offered once', sourcesFor([svc, { scope: { kind: 'host' }, paths: ['/logs/{service}.INFO'] }], api).length === 1);
check('a label names many files briefly', sourceLabel(['/a/x.log', '/a/y.log', '/a/z.log']) === 'x.log + 2 more');

console.log('\nsaving');
const twice = upsert(upsert([], svc), { ...svc, label: 'Full log' });
check('the same scope and files replace, keeping the newest', twice.length === 1 && twice[0].label === 'Full log');
check('other files for the same scope are added', upsert([svc], { ...svc, paths: ['/logs/api.ERROR'] }).length === 2);
check('forgetting removes only that one', eq(remove([svc, host], svc), [host]));

console.log('\nrotations');
check('numbered, dated and compressed copies have a stem', rotationStem('/logs/api.log.93') === '/logs/api.log' && rotationStem('/var/log/syslog.2.gz') === '/var/log/syslog' && rotationStem('/var/log/app.log-20261017') === '/var/log/app.log' && rotationStem('/logs/x.2026-10-17') === '/logs/x');
check('current files have none', rotationStem('/logs/api.INFO') === null && rotationStem('/logs/app-2.log') === null && rotationStem('/var/log/worker/jobs.log') === null);
const listing = [
	{ path: '/logs/api.INFO', modified: 100 },
	{ path: '/logs/api.log.93', modified: 100 },
	{ path: '/logs/api.WARNING', modified: 99 },
	{ path: '/logs/api.log.92', modified: 80 },
	{ path: '/logs/api.log.1', modified: 10 },
	{ path: '/var/log/syslog', modified: 50 },
	{ path: '/var/log/syslog.1', modified: 40 },
	{ path: '/var/log/syslog.2.gz', modified: 30 }
];
const groups = groupRotated(listing);
check('rotations fold under the newest copy when the stem is not there', eq(groups.map((g) => [g.file.path, g.older.length]), [['/logs/api.INFO', 0], ['/logs/api.log.93', 2], ['/logs/api.WARNING', 0], ['/var/log/syslog', 2]]), JSON.stringify(groups.map((g) => [g.file.path, g.older.length])));
check('and under the current file when it is', groups[3].older.map((f) => f.path).join() === '/var/log/syslog.1,/var/log/syslog.2.gz');
check('a single numbered file stays as it is', groupRotated([{ path: '/data/run.1', modified: 1 }]).length === 1);

console.log('\nsizes');
check('bytes, KB, MB', size(980) === '980 B' && size(2048) === '2.0 KB' && size(5 * 1024 * 1024) === '5.0 MB' && size(150 * 1024 * 1024) === '150 MB');

console.log(failures ? `\n${failures} failed` : '\nall passed');
process.exit(failures ? 1 : 0);
