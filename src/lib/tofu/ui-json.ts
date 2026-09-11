/**
 * OpenTofu's machine-readable UI, as typed messages.
 *
 * With `-json`, `plan`, `apply` and `destroy` write one JSON object per line
 * to stdout instead of prose. Every object carries `type`, `@level`,
 * `@message` and `@timestamp`; the interesting ones carry a payload beside
 * those. This module turns a line into a `TofuUiMessage`, or `null` when
 * the line is not one — the raw text is kept either way, so a parser miss
 * costs a badge, never a line of output.
 *
 * Shapes follow https://opentofu.org/docs/internals/machine-readable-ui/
 * (UI schema 1.x, shared with Terraform's).
 */

export type TofuChangeAction = 'noop' | 'create' | 'read' | 'update' | 'replace' | 'delete' | 'move' | 'import';

export interface TofuUiResource {
	addr: string;
	module: string;
	resource: string;
	impliedProvider: string;
	resourceType: string;
	resourceName: string;
	resourceKey: string | number | null;
}

export interface TofuUiChangeSummary {
	add: number;
	change: number;
	remove: number;
	import: number;
	operation: 'plan' | 'apply' | 'destroy' | string;
}

export interface TofuUiDiagnostic {
	severity: 'error' | 'warning' | string;
	summary: string;
	detail: string;
	/** `file:line` when the diagnostic points at source, else null. */
	location: string | null;
}

export interface TofuUiOutput {
	sensitive: boolean;
	type: unknown;
	value: unknown;
	action?: string;
}

export type TofuUiMessage =
	| { type: 'version'; tofu: string; ui: string }
	| { type: 'log'; message: string; level: string }
	| { type: 'diagnostic'; diagnostic: TofuUiDiagnostic }
	| { type: 'planned_change'; resource: TofuUiResource; action: TofuChangeAction; reason: string | null }
	| { type: 'resource_drift'; resource: TofuUiResource; action: TofuChangeAction }
	| { type: 'change_summary'; changes: TofuUiChangeSummary }
	| { type: 'outputs'; outputs: Record<string, TofuUiOutput> }
	| { type: 'apply_start'; resource: TofuUiResource; action: TofuChangeAction }
	| { type: 'apply_progress'; resource: TofuUiResource; action: TofuChangeAction; elapsed: number }
	| { type: 'apply_complete'; resource: TofuUiResource; action: TofuChangeAction; elapsed: number; id: string | null }
	| { type: 'apply_errored'; resource: TofuUiResource; action: TofuChangeAction; elapsed: number }
	| { type: 'refresh_start'; resource: TofuUiResource }
	| { type: 'refresh_complete'; resource: TofuUiResource; id: string | null }
	| { type: 'provision_start' | 'provision_progress' | 'provision_complete' | 'provision_errored'; resource: TofuUiResource; message: string };

type Json = Record<string, unknown>;

function obj(v: unknown): Json | null {
	return v !== null && typeof v === 'object' && !Array.isArray(v) ? (v as Json) : null;
}
function str(v: unknown, fallback = ''): string {
	return typeof v === 'string' ? v : fallback;
}
function num(v: unknown, fallback = 0): number {
	return typeof v === 'number' && Number.isFinite(v) ? v : fallback;
}

function resource(v: unknown): TofuUiResource | null {
	const r = obj(v);
	if (!r || typeof r.addr !== 'string') return null;
	const key = r.resource_key;
	return {
		addr: r.addr,
		module: str(r.module),
		resource: str(r.resource, r.addr),
		impliedProvider: str(r.implied_provider),
		resourceType: str(r.resource_type),
		resourceName: str(r.resource_name),
		resourceKey: typeof key === 'string' || typeof key === 'number' ? key : null
	};
}

function action(v: unknown): TofuChangeAction {
	const a = str(v, 'noop');
	switch (a) {
		case 'create':
		case 'read':
		case 'update':
		case 'replace':
		case 'delete':
		case 'move':
		case 'import':
			return a;
		default:
			return 'noop';
	}
}

/** The id a hook reports for a resource once it exists, as `key=value`'s value. */
function hookId(h: Json): string | null {
	const v = h.id_value;
	if (v === undefined || v === null) return null;
	return typeof v === 'string' ? v : JSON.stringify(v);
}

/**
 * Parse one stdout line. Returns `null` for anything that is not a UI
 * message — prose from a command run without `-json`, a blank line, or a
 * shape this module does not know. Never throws.
 */
export function parseTofuUiLine(line: string): TofuUiMessage | null {
	const trimmed = line.trim();
	if (!trimmed.startsWith('{') || !trimmed.endsWith('}')) return null;
	let root: Json | null;
	try {
		root = obj(JSON.parse(trimmed));
	} catch {
		return null;
	}
	if (!root || typeof root.type !== 'string') return null;
	const message = str(root['@message']);
	const level = str(root['@level'], 'info');

	switch (root.type) {
		case 'version':
			return { type: 'version', tofu: str(root.tofu ?? root.terraform), ui: str(root.ui) };
		case 'log':
			return { type: 'log', message, level };
		case 'diagnostic': {
			const d = obj(root.diagnostic);
			if (!d) return { type: 'log', message, level };
			const range = obj(d.range);
			const start = range ? obj(range.start) : null;
			const file = range ? str(range.filename) : '';
			const location = file ? (start && typeof start.line === 'number' ? `${file}:${start.line}` : file) : null;
			return {
				type: 'diagnostic',
				diagnostic: {
					severity: str(d.severity, level),
					summary: str(d.summary, message),
					detail: str(d.detail),
					location
				}
			};
		}
		case 'planned_change':
		case 'resource_drift': {
			const c = obj(root.change);
			const r = c ? resource(c.resource) : null;
			if (!c || !r) return { type: 'log', message, level };
			if (root.type === 'resource_drift') return { type: 'resource_drift', resource: r, action: action(c.action) };
			return { type: 'planned_change', resource: r, action: action(c.action), reason: typeof c.reason === 'string' ? c.reason : null };
		}
		case 'change_summary': {
			const c = obj(root.changes);
			if (!c) return { type: 'log', message, level };
			return {
				type: 'change_summary',
				changes: {
					add: num(c.add),
					change: num(c.change),
					remove: num(c.remove),
					import: num(c.import),
					operation: str(c.operation, 'plan')
				}
			};
		}
		case 'outputs': {
			const o = obj(root.outputs) ?? {};
			const outputs: Record<string, TofuUiOutput> = {};
			for (const [name, raw] of Object.entries(o)) {
				const v = obj(raw);
				if (!v) continue;
				outputs[name] = {
					sensitive: v.sensitive === true,
					type: v.type,
					value: v.value,
					action: typeof v.action === 'string' ? v.action : undefined
				};
			}
			return { type: 'outputs', outputs };
		}
		case 'apply_start':
		case 'apply_progress':
		case 'apply_complete':
		case 'apply_errored':
		case 'refresh_start':
		case 'refresh_complete':
		case 'provision_start':
		case 'provision_progress':
		case 'provision_complete':
		case 'provision_errored': {
			const h = obj(root.hook);
			const r = h ? resource(h.resource) : null;
			if (!h || !r) return { type: 'log', message, level };
			const elapsed = num(h.elapsed_seconds);
			switch (root.type) {
				case 'apply_start':
					return { type: 'apply_start', resource: r, action: action(h.action) };
				case 'apply_progress':
					return { type: 'apply_progress', resource: r, action: action(h.action), elapsed };
				case 'apply_complete':
					return { type: 'apply_complete', resource: r, action: action(h.action), elapsed, id: hookId(h) };
				case 'apply_errored':
					return { type: 'apply_errored', resource: r, action: action(h.action), elapsed };
				case 'refresh_start':
					return { type: 'refresh_start', resource: r };
				case 'refresh_complete':
					return { type: 'refresh_complete', resource: r, id: hookId(h) };
				default:
					return { type: root.type, resource: r, message };
			}
		}
		default:
			return { type: 'log', message, level };
	}
}

// ---------------------------------------------------------------------------
// A whole run, folded from its messages.

export type TofuApplyState = 'running' | 'done' | 'errored';

export interface TofuApplyRow {
	resource: TofuUiResource;
	action: TofuChangeAction;
	state: TofuApplyState;
	elapsed: number;
	id: string | null;
}

export interface TofuRunView {
	/** Any UI message seen at all — the switch between structured and raw. */
	structured: boolean;
	tofuVersion: string | null;
	summary: TofuUiChangeSummary | null;
	planned: { resource: TofuUiResource; action: TofuChangeAction; reason: string | null }[];
	drift: { resource: TofuUiResource; action: TofuChangeAction }[];
	/** Insertion-ordered by address, so a row keeps its place as it progresses. */
	apply: TofuApplyRow[];
	diagnostics: TofuUiDiagnostic[];
	outputs: Record<string, TofuUiOutput>;
	/** Refreshed resources, counted rather than listed: dozens, all "no news". */
	refreshed: number;
}

export function emptyRunView(): TofuRunView {
	return {
		structured: false,
		tofuVersion: null,
		summary: null,
		planned: [],
		drift: [],
		apply: [],
		diagnostics: [],
		outputs: {},
		refreshed: 0
	};
}

/** Fold one message into the view. Mutates and returns `view`. */
export function applyTofuUiMessage(view: TofuRunView, m: TofuUiMessage): TofuRunView {
	view.structured = true;
	switch (m.type) {
		case 'version':
			view.tofuVersion = m.tofu || null;
			break;
		case 'change_summary':
			view.summary = m.changes;
			break;
		case 'planned_change':
			view.planned.push({ resource: m.resource, action: m.action, reason: m.reason });
			break;
		case 'resource_drift':
			view.drift.push({ resource: m.resource, action: m.action });
			break;
		case 'diagnostic':
			view.diagnostics.push(m.diagnostic);
			break;
		case 'outputs':
			view.outputs = m.outputs;
			break;
		case 'refresh_complete':
			view.refreshed += 1;
			break;
		case 'apply_start':
		case 'apply_progress':
		case 'apply_complete':
		case 'apply_errored': {
			let row = view.apply.find((r) => r.resource.addr === m.resource.addr);
			if (!row) {
				row = { resource: m.resource, action: m.action, state: 'running', elapsed: 0, id: null };
				view.apply.push(row);
			}
			row.action = m.action;
			if (m.type === 'apply_progress') row.elapsed = m.elapsed;
			if (m.type === 'apply_complete') {
				row.state = 'done';
				row.elapsed = m.elapsed;
				row.id = m.id;
			}
			if (m.type === 'apply_errored') {
				row.state = 'errored';
				row.elapsed = m.elapsed;
			}
			break;
		}
		default:
			break;
	}
	return view;
}

/**
 * Split a stream of chunks into lines. A chunk may end mid-line (remote
 * execution hands back whatever the channel had), so the tail is carried
 * to the next call. Returns the complete lines and the new carry.
 */
export function splitLines(carry: string, chunk: string): { lines: string[]; carry: string } {
	const text = carry + chunk;
	const parts = text.split(/\r?\n/);
	const rest = parts.pop() ?? '';
	return { lines: parts, carry: rest };
}
