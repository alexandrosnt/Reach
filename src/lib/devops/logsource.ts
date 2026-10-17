/**
 * Which saved log files apply to a container or pod, and how they read in the
 * source picker (issue #101). The backend checks and stores them (see
 * src-tauri/src/logfiles.rs); this decides what a page offers.
 *
 * A saved source names files for a scope: one Compose service (whatever its
 * containers are called after the next `up`), one container by name, one
 * Kubernetes container name in a namespace (across the pods it runs in), or
 * every container on the host, usually with `{service}` in the path.
 */

export type Scope =
	| { kind: 'service'; project: string; service: string }
	| { kind: 'container'; name: string }
	| { kind: 'podContainer'; namespace: string; container: string }
	| { kind: 'host' };

export interface LogRule {
	scope: Scope;
	paths: string[];
	label?: string | null;
}

/** What a page knows about the container whose log it shows. */
export interface Target {
	/** Container or pod name. */
	name: string;
	project?: string | null;
	service?: string | null;
	namespace?: string | null;
	/** The container's name inside the pod, for Kubernetes. */
	container?: string | null;
}

export interface Source {
	rule: LogRule;
	/** The rule's paths with `{service}` and friends filled in. */
	paths: string[];
	label: string;
}

export function applies(rule: LogRule, t: Target): boolean {
	const s = rule.scope;
	switch (s.kind) {
		case 'service':
			return !!t.service && s.project === (t.project ?? '') && s.service === t.service;
		case 'container':
			return s.name === t.name;
		case 'podContainer':
			return !!t.container && s.namespace === (t.namespace ?? '') && s.container === t.container;
		case 'host':
			return true;
	}
}

/** `{service}`, `{project}`, `{container}` and `{namespace}` filled in. */
export function expand(path: string, t: Target): string {
	return path
		.replaceAll('{service}', t.service || t.container || t.name)
		.replaceAll('{project}', t.project ?? '')
		.replaceAll('{container}', t.container || t.name)
		.replaceAll('{namespace}', t.namespace ?? '');
}

/** A path made general for every container: the service's name becomes `{service}`. */
export function templated(path: string, t: Target): string {
	const word = t.service || t.container;
	if (!word || word.length < 2) return path;
	return path.split(word).join('{service}');
}

/** "api.INFO", "api.WARNING + api.ERROR", "a.log + 3 more". */
export function sourceLabel(paths: string[]): string {
	const names = paths.map((p) => p.slice(p.lastIndexOf('/') + 1) || p);
	return names.length <= 2 ? names.join(' + ') : `${names[0]} + ${names.length - 1} more`;
}

const SPECIFIC: Record<Scope['kind'], number> = { container: 0, service: 1, podContainer: 1, host: 2 };

/** The saved sources for `t`, most specific first, each once. */
export function sourcesFor(rules: LogRule[], t: Target): Source[] {
	const out: Source[] = [];
	const seen = new Set<string>();
	for (const rule of [...rules].sort((a, b) => SPECIFIC[a.scope.kind] - SPECIFIC[b.scope.kind])) {
		if (!applies(rule, t)) continue;
		const paths = rule.paths.map((p) => expand(p, t));
		const key = paths.join('\n');
		if (seen.has(key)) continue;
		seen.add(key);
		out.push({ rule, paths, label: rule.label?.trim() || sourceLabel(paths) });
	}
	return out;
}

export function sameScope(a: Scope, b: Scope): boolean {
	return JSON.stringify(a) === JSON.stringify(b);
}

function samePaths(a: string[], b: string[]): boolean {
	return a.length === b.length && a.every((p, i) => p === b[i]);
}

/** Add a rule, replacing one with the same scope and files. */
export function upsert(rules: LogRule[], rule: LogRule): LogRule[] {
	const rest = rules.filter((r) => !(sameScope(r.scope, rule.scope) && samePaths(r.paths, rule.paths)));
	return [...rest, rule];
}

export function remove(rules: LogRule[], rule: LogRule): LogRule[] {
	return rules.filter((r) => !(sameScope(r.scope, rule.scope) && samePaths(r.paths, rule.paths)));
}

/** A rotated copy's suffix: `.1`, `.12.gz`, `-20261017`, `.2026-10-17`, compressed or not. */
const ROTATION = /([.-](\d{1,6}|\d{8}(?:[-_T]?\d{4,6})?|\d{4}-\d{2}-\d{2}(?:[-_T]\d{2}[-:]?\d{2}(?:[-:]?\d{2})?)?))?(\.(?:gz|zst|bz2|xz))?$/;

/** The file a rotated copy was rotated from: `api.log.93` → `api.log`. */
export function rotationStem(path: string): string | null {
	const m = ROTATION.exec(path);
	if (!m || (!m[1] && !m[3])) return null;
	const stem = path.slice(0, m.index);
	return stem.length > 1 && !stem.endsWith('/') ? stem : null;
}

export interface FileGroup<F extends { path: string; modified: number }> {
	file: F;
	/** Rotated copies of it, newest first. */
	older: F[];
}

/**
 * Rotated copies folded under the file they came from, so a listing shows
 * what is being written now instead of a hundred `api.log.N`. Where the
 * current file is not there itself, the newest copy stands for the group.
 */
export function groupRotated<F extends { path: string; modified: number }>(files: F[]): FileGroup<F>[] {
	const byStem = new Map<string, F[]>();
	for (const f of files) {
		const stem = rotationStem(f.path);
		if (stem) byStem.set(stem, [...(byStem.get(stem) ?? []), f]);
	}
	const paths = new Set(files.map((f) => f.path));
	const folded = new Set<string>();
	const heads = new Map<string, F[]>();
	for (const [stem, members] of byStem) {
		if (members.length < 2 && !paths.has(stem)) continue;
		const sorted = [...members].sort((a, b) => b.modified - a.modified);
		const head = paths.has(stem) ? stem : sorted[0].path;
		const older = sorted.filter((f) => f.path !== head);
		for (const f of older) folded.add(f.path);
		heads.set(head, older);
	}
	return files.filter((f) => !folded.has(f.path)).map((f) => ({ file: f, older: heads.get(f.path) ?? [] }));
}

/** "2.4 MB", "980 B". */
export function size(bytes: number): string {
	if (bytes < 1024) return `${bytes} B`;
	const units = ['KB', 'MB', 'GB', 'TB'];
	let v = bytes / 1024;
	let i = 0;
	while (v >= 1024 && i < units.length - 1) {
		v /= 1024;
		i++;
	}
	return `${v < 10 ? v.toFixed(1) : Math.round(v)} ${units[i]}`;
}
