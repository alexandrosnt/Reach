/**
 * Reading RDP hosts out of pasted JSON for the bulk importer.
 *
 * Two shapes are accepted: a Termix export ({ hosts: [...] }, where each host
 * says its connectionType) and a plain array of { name, ip|host, port,
 * username, domain?, folder? }. Passwords and every other secret in the input
 * are never read. Kept apart from the dialog so it can be tested in Node.
 */

import type { Folder, SessionConfig } from '#lib/ipc/sessions.js';

export interface RdpImportHost {
	/** Position in the input, unique within one paste. */
	id: string;
	name: string;
	host: string;
	port: number;
	username: string;
	domain: string | null;
	folder: string | null;
}

/** Why an entry in the input could not become a session. */
export type RdpSkipReason = 'no_host' | 'bad_port';

export interface RdpSkipped {
	label: string;
	reason: RdpSkipReason;
}

export type RdpParseResult =
	| { ok: true; hosts: RdpImportHost[]; skipped: RdpSkipped[] }
	| { ok: false; error: 'bad_json'; detail: string }
	| { ok: false; error: 'no_hosts' };

const RDP_PORT = 3389;

type Entry = Record<string, unknown>;

function isEntry(value: unknown): value is Entry {
	return typeof value === 'object' && value !== null && !Array.isArray(value);
}

/** A trimmed string, or '' for anything that is not a string or a number. */
function text(value: unknown): string {
	if (typeof value === 'string') return value.trim();
	if (typeof value === 'number' && Number.isFinite(value)) return String(value);
	return '';
}

/** A TCP port, or null when the value is not one. Absent means the RDP default. */
export function parsePort(value: unknown): number | null {
	if (value === undefined || value === null || value === '') return RDP_PORT;
	const n = typeof value === 'number' ? value : Number(text(value));
	return Number.isInteger(n) && n >= 1 && n <= 65535 ? n : null;
}

/** The entries to read: the array itself, or a Termix export's hosts. */
function entriesOf(data: unknown): unknown[] | null {
	if (Array.isArray(data)) return data;
	if (isEntry(data) && Array.isArray(data.hosts)) return data.hosts;
	return null;
}

/** Termix writes connectionType on every host; a plain list has none and is all RDP. */
function isRdp(entry: Entry): boolean {
	const type = text(entry.connectionType).toLowerCase();
	return type === '' || type === 'rdp';
}

/** Termix keeps a non-default RDP port in its remote-desktop plugin settings. */
function termixRdpPort(entry: Entry): unknown {
	const plugins = entry.pluginSettings;
	if (!isEntry(plugins)) return undefined;
	const rdp = plugins['remote-desktop'];
	return isEntry(rdp) ? rdp.rdpPort : undefined;
}

export function parseRdpHosts(raw: string): RdpParseResult {
	let data: unknown;
	try {
		data = JSON.parse(raw);
	} catch (e) {
		return { ok: false, error: 'bad_json', detail: e instanceof Error ? e.message : String(e) };
	}
	const entries = entriesOf(data);
	if (!entries) return { ok: false, error: 'no_hosts' };

	const hosts: RdpImportHost[] = [];
	const skipped: RdpSkipped[] = [];
	entries.forEach((entry, i) => {
		if (!isEntry(entry) || !isRdp(entry)) return;
		const host = text(entry.ip) || text(entry.host) || text(entry.address);
		const name = text(entry.name) || host;
		if (!host) {
			skipped.push({ label: name || `#${i + 1}`, reason: 'no_host' });
			return;
		}
		const port = parsePort(entry.port ?? termixRdpPort(entry));
		if (port === null) {
			skipped.push({ label: name, reason: 'bad_port' });
			return;
		}
		hosts.push({
			id: String(i),
			name,
			host,
			port,
			username: text(entry.username),
			domain: text(entry.domain) || null,
			folder: text(entry.folder) || null
		});
	});

	if (hosts.length === 0 && skipped.length === 0) return { ok: false, error: 'no_hosts' };
	return { ok: true, hosts, skipped };
}

/** An RDP session already saved for the same machine and user. */
export function findExisting(host: RdpImportHost, sessions: SessionConfig[]): SessionConfig | undefined {
	const address = host.host.toLowerCase();
	return sessions.find(
		(s) =>
			s.kind === 'rdp' &&
			s.host.toLowerCase() === address &&
			s.port === host.port &&
			s.username === host.username
	);
}

/** A top-level folder of that name already in the vault, so a second import reuses it. */
export function findFolder(name: string, vaultId: string | null, folders: Folder[]): Folder | undefined {
	return folders.find((f) => f.parent_id === null && (f.vault_id ?? null) === vaultId && f.name === name);
}
