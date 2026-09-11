/**
 * An Ansible run, read off the default callback's output.
 *
 * Ansible's stdout callback writes a line per play, per task, per host
 * result, and a recap table at the end. Those lines are the contract every
 * CI system and every wrapper has read for a decade, and with colour off
 * (`ANSIBLE_NOCOLOR=1`, which Reach sets) they parse cleanly. This module
 * turns them into plays → tasks → host results, plus the recap.
 *
 * It is built so an event source can feed the same view later:
 * `ansible-runner run -j` emits one JSON event per host result, and the
 * fold below takes exactly that shape. When runner is present, swap the
 * parser; the view does not change.
 *
 * Unknown lines are ignored. The raw log is always kept beside this.
 */

export type HostStatus = 'ok' | 'changed' | 'failed' | 'unreachable' | 'skipped' | 'ignored';

export interface HostResult {
	host: string;
	status: HostStatus;
	/** The `msg` from a JSON result, when there was one. */
	msg: string | null;
	/** A loop item, for `with_items` / `loop` tasks. */
	item: string | null;
}

export interface TaskView {
	name: string;
	/** `role : task` names arrive joined; the role is split off for display. */
	role: string | null;
	handler: boolean;
	hosts: HostResult[];
}

export interface PlayView {
	name: string;
	tasks: TaskView[];
}

export interface RecapRow {
	host: string;
	ok: number;
	changed: number;
	unreachable: number;
	failed: number;
	skipped: number;
	rescued: number;
	ignored: number;
}

export interface AnsibleRunView {
	/** Any play, task or recap seen — the switch between structured and raw. */
	structured: boolean;
	plays: PlayView[];
	recap: RecapRow[];
	/** `[WARNING]: …` lines, verbatim. */
	warnings: string[];
	/** `ERROR! …` lines: a run that never got to a play. */
	errors: string[];
}

export function emptyAnsibleRun(): AnsibleRunView {
	return { structured: false, plays: [], recap: [], warnings: [], errors: [] };
}

export type AnsibleEvent =
	| { type: 'play'; name: string }
	| { type: 'task'; name: string; role: string | null; handler: boolean }
	| { type: 'result'; host: string; status: HostStatus; msg: string | null; item: string | null }
	| { type: 'ignoring' }
	| { type: 'recap'; row: RecapRow }
	| { type: 'warning'; text: string }
	| { type: 'error'; text: string };

const PLAY = /^PLAY \[(.*)\] \*+\s*$/;
const TASK = /^(TASK|RUNNING HANDLER) \[(.*)\] \*+\s*$/;
const RESULT =
	/^(ok|changed|failed|fatal|skipping|unreachable): \[([^\]]*?)(?: -> [^\]]*)?\](?:: (FAILED|UNREACHABLE)!)?(?: \(item=(.*?)\))?(?: => (?:\(item=(.*?)\))?\s*(.*))?\s*$/;
const RECAP_ROW =
	/^(\S+)\s+: ok=(\d+)\s+changed=(\d+)\s+unreachable=(\d+)\s+failed=(\d+)(?:\s+skipped=(\d+))?(?:\s+rescued=(\d+))?(?:\s+ignored=(\d+))?/;

function msgOf(rest: string | undefined): string | null {
	if (!rest) return null;
	const text = rest.trim();
	if (!text.startsWith('{')) return text || null;
	try {
		const v = JSON.parse(text);
		if (v && typeof v === 'object') {
			if (typeof v.msg === 'string') return v.msg;
			if (typeof v.stderr === 'string' && v.stderr) return v.stderr;
			if (typeof v.reason === 'string') return v.reason;
		}
	} catch {
		// A result too long for one line, or not JSON at all: keep the text.
	}
	return null;
}

/** One stdout line → one event, or null when the line is not one. */
export function parseAnsibleLine(line: string): AnsibleEvent | null {
	const l = line.replace(/\r$/, '');
	if (!l.trim()) return null;

	let m = PLAY.exec(l);
	if (m) return { type: 'play', name: m[1] };

	m = TASK.exec(l);
	if (m) {
		const full = m[2];
		const sep = full.indexOf(' : ');
		const role = sep > 0 ? full.slice(0, sep) : null;
		const name = sep > 0 ? full.slice(sep + 3) : full;
		return { type: 'task', name, role, handler: m[1] === 'RUNNING HANDLER' };
	}

	if (/^PLAY RECAP \*+/.test(l)) return null;

	m = RECAP_ROW.exec(l);
	if (m) {
		return {
			type: 'recap',
			row: {
				host: m[1],
				ok: +m[2],
				changed: +m[3],
				unreachable: +m[4],
				failed: +m[5],
				skipped: +(m[6] ?? 0),
				rescued: +(m[7] ?? 0),
				ignored: +(m[8] ?? 0)
			}
		};
	}

	if (l.startsWith('...ignoring')) return { type: 'ignoring' };
	if (l.startsWith('[WARNING]:')) return { type: 'warning', text: l.slice('[WARNING]:'.length).trim() };
	if (l.startsWith('ERROR!')) return { type: 'error', text: l.slice('ERROR!'.length).trim() };

	m = RESULT.exec(l);
	if (m) {
		const [, verb, host, kind, item1, item2, rest] = m;
		let status: HostStatus;
		if (verb === 'fatal') status = kind === 'UNREACHABLE' ? 'unreachable' : 'failed';
		else if (verb === 'skipping') status = 'skipped';
		else if (verb === 'unreachable') status = 'unreachable';
		else status = verb as HostStatus;
		return { type: 'result', host, status, msg: msgOf(rest), item: item1 ?? item2 ?? null };
	}

	return null;
}

/** Fold one event into the view. Mutates and returns `view`. */
export function applyAnsibleEvent(view: AnsibleRunView, e: AnsibleEvent): AnsibleRunView {
	switch (e.type) {
		case 'play':
			view.structured = true;
			view.plays.push({ name: e.name, tasks: [] });
			break;
		case 'task': {
			view.structured = true;
			if (view.plays.length === 0) view.plays.push({ name: '', tasks: [] });
			view.plays[view.plays.length - 1].tasks.push({ name: e.name, role: e.role, handler: e.handler, hosts: [] });
			break;
		}
		case 'result': {
			const task = currentTask(view);
			if (!task) break;
			task.hosts.push({ host: e.host, status: e.status, msg: e.msg, item: e.item });
			break;
		}
		case 'ignoring': {
			// "...ignoring" follows the failure it forgives.
			const task = currentTask(view);
			const last = task?.hosts[task.hosts.length - 1];
			if (last && last.status === 'failed') last.status = 'ignored';
			break;
		}
		case 'recap':
			view.structured = true;
			view.recap.push(e.row);
			break;
		case 'warning':
			view.warnings.push(e.text);
			break;
		case 'error':
			view.errors.push(e.text);
			break;
	}
	return view;
}

function currentTask(view: AnsibleRunView): TaskView | null {
	const play = view.plays[view.plays.length - 1];
	if (!play) return null;
	return play.tasks[play.tasks.length - 1] ?? null;
}

/** The worst thing that happened on a task, for its colour. */
export function taskStatus(task: TaskView): HostStatus | 'pending' {
	if (task.hosts.length === 0) return 'pending';
	const order: HostStatus[] = ['unreachable', 'failed', 'changed', 'ignored', 'ok', 'skipped'];
	for (const s of order) if (task.hosts.some((h) => h.status === s)) return s;
	return 'ok';
}
