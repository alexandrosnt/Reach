/**
 * What the Kubernetes workspace's parts share: the views in the left nav,
 * what is selected, and the rules for "is this a problem?" so the Overview,
 * the lists and the "Problems only" switch all agree.
 */

import type { Kind, PodRow, WorkloadRow, ObjectRow, EventRow, Revision } from '#lib/ipc/k8s.js';
import type { Environment } from '#lib/ipc/containers.js';
import { podStatus, type Status } from '#lib/devops/status.js';
import { t } from '#lib/state/i18n.svelte.js';

/** The open cluster, as every view needs it. */
export interface Ctx {
	key: string;
	clusterId: string;
	clusterName: string;
	environment: Environment;
	readOnly: boolean;
	/** Null means all namespaces. */
	namespace: string | null;
}

export type View =
	| 'overview'
	| 'pods'
	| 'workloads'
	| 'jobs'
	| 'cronjobs'
	| 'services'
	| 'ingresses'
	| 'configmaps'
	| 'secrets'
	| 'pvcs'
	| 'helm'
	| 'events'
	| 'nodes'
	| 'namespaces';

export const NAV: { group: string; items: View[] }[] = [
	{ group: '', items: ['overview'] },
	{ group: 'workloads', items: ['pods', 'workloads', 'jobs', 'cronjobs'] },
	{ group: 'network', items: ['services', 'ingresses'] },
	{ group: 'config', items: ['configmaps', 'secrets'] },
	{ group: 'storage', items: ['pvcs'] },
	{ group: 'helm', items: ['helm'] },
	{ group: 'cluster', items: ['events', 'nodes', 'namespaces'] }
];

/** Views listed through k8sObjects, and the kind each lists. */
export const OBJECT_KIND: Partial<Record<View, Kind>> = {
	jobs: 'Job',
	cronjobs: 'CronJob',
	services: 'Service',
	ingresses: 'Ingress',
	configmaps: 'ConfigMap',
	secrets: 'Secret',
	pvcs: 'PersistentVolumeClaim',
	nodes: 'Node',
	namespaces: 'Namespace'
};

/** Kinds that live outside namespaces. */
export const CLUSTER_SCOPED: Kind[] = ['Node', 'Namespace'];

export type Selection =
	| { type: 'pod'; row: PodRow }
	| { type: 'workload'; row: WorkloadRow }
	| { type: 'object'; kind: Kind; row: ObjectRow }
	| { type: 'helm'; row: Revision };

export function selId(s: Selection): string {
	const kind = s.type === 'pod' ? 'Pod' : s.type === 'helm' ? 'Helm' : s.type === 'workload' ? s.row.kind : s.kind;
	return `${kind}/${s.row.namespace}/${s.row.name}`;
}

/** Running but not every container ready is a problem too (a failing probe). */
export function podProblem(p: PodRow): boolean {
	const tone = podStatus(p.status, p.restarts).tone;
	if (tone === 'bad' || tone === 'warn') return true;
	if (tone === 'ok') {
		const [ready, total] = p.ready.split('/').map(Number);
		return ready < total;
	}
	return false;
}

export function workloadProblem(w: WorkloadRow): boolean {
	return w.ready < w.desired;
}

export function nodeStatus(summary: string): Status {
	const word = summary.split(' ')[0] ?? '';
	if (word === 'Ready') return { tone: 'ok', label: t('k8s.node_ready'), raw: summary };
	if (word === 'NotReady') return { tone: 'bad', label: t('k8s.node_not_ready'), raw: summary };
	return { tone: 'warn', label: t('k8s.node_unknown'), raw: summary };
}

/** Helm's release states, in words. */
export function helmStatus(status: string): Status {
	const raw = status;
	switch (status) {
		case 'deployed':
			return { tone: 'ok', label: t('k8s.helm_deployed'), raw };
		case 'superseded':
			return { tone: 'idle', label: t('k8s.helm_superseded'), raw };
		case 'failed':
			return { tone: 'bad', label: t('k8s.helm_failed'), raw };
		case 'uninstalled':
			return { tone: 'idle', label: t('k8s.helm_uninstalled'), raw };
		case 'uninstalling':
		case 'pending-install':
		case 'pending-upgrade':
		case 'pending-rollback':
			return { tone: 'busy', label: t('k8s.helm_pending'), raw };
		default:
			return { tone: 'warn', label: status || '?', raw };
	}
}

export function helmProblem(r: Revision): boolean {
	return r.status !== 'deployed' && r.status !== 'superseded';
}

export function eventStatus(e: EventRow): Status {
	return e.kind === 'Warning'
		? { tone: 'warn', label: t('k8s.event_warning'), raw: e.kind }
		: { tone: 'idle', label: t('k8s.event_normal'), raw: e.kind || 'Normal' };
}

/** The API server's answer to a replace carrying an outdated resourceVersion. */
export function isConflict(error: string): boolean {
	return error.includes('(409)') || error.includes('has been modified');
}

const HIDDEN = '••••••••';

/**
 * A Secret's YAML with every value hidden: the `data` and `stringData`
 * blocks, and the last-applied annotation, which carries the same values
 * in plain JSON.
 */
export function maskSecretYaml(yaml: string): string {
	const out: string[] = [];
	// Lines indented deeper than `below` continue the hidden value; `keys`
	// keeps the key names visible (inside `data`), else the line goes whole.
	let hide: { below: number; keys: boolean } | null = null;
	for (const line of yaml.split('\n')) {
		const indent = line.length - line.trimStart().length;
		if (hide) {
			if (line.trim() === '') {
				out.push(line);
				continue;
			}
			if (indent > hide.below) {
				const kv = hide.keys ? /^([^:]+):\s*(.*)$/.exec(line.slice(indent)) : null;
				out.push(line.slice(0, indent) + (kv ? (kv[2] ? `${kv[1]}: ${HIDDEN}` : `${kv[1]}:`) : HIDDEN));
				continue;
			}
			hide = null;
		}
		const m = /^(data|stringData):\s*(.*)$/.exec(line);
		if (m) {
			// `data: {}` is empty; anything else inline is hidden whole.
			out.push(m[2] && m[2] !== '{}' ? `${m[1]}: ${HIDDEN}` : line);
			hide = { below: 0, keys: !m[2] };
			continue;
		}
		const a = /^(\s*)(["']?kubectl\.kubernetes\.io\/last-applied-configuration["']?):/.exec(line);
		if (a) {
			out.push(`${a[1]}${a[2]}: ${HIDDEN}`);
			hide = { below: a[1].length, keys: false };
			continue;
		}
		out.push(line);
	}
	return out.join('\n');
}

/** A Secret's `data` entries, decoded from base64 as UTF-8 where they are text. */
export function decodeSecretData(yaml: string): { key: string; value: string }[] {
	const rows: { key: string; value: string }[] = [];
	let inData = false;
	for (const line of yaml.split('\n')) {
		if (/^data:\s*$/.test(line)) {
			inData = true;
			continue;
		}
		if (!inData) continue;
		if (line.trim() && !/^\s/.test(line)) break;
		const m = /^\s+["']?([^"':]+)["']?:\s*["']?([A-Za-z0-9+/=]*)["']?\s*$/.exec(line);
		if (!m) continue;
		rows.push({ key: m[1], value: decode(m[2]) });
	}
	return rows;
}

function decode(b64: string): string {
	try {
		const bin = atob(b64);
		const bytes = Uint8Array.from(bin, (c) => c.charCodeAt(0));
		return new TextDecoder('utf-8', { fatal: true }).decode(bytes);
	} catch {
		return t('k8s.secret_binary');
	}
}

/** Remember the last cluster and namespace; storage may be unavailable. */
const LAST = 'reach.k8s.last';

export function loadLast(): { cluster: string; namespace: string | null } | null {
	try {
		const raw = localStorage.getItem(LAST);
		return raw ? JSON.parse(raw) : null;
	} catch {
		return null;
	}
}

export function saveLast(cluster: string, namespace: string | null): void {
	try {
		localStorage.setItem(LAST, JSON.stringify({ cluster, namespace }));
	} catch {
		// Private mode or blocked storage: nothing to remember then.
	}
}
