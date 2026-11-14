/**
 * Reading hosts out of a Devolutions Remote Desktop Manager export.
 *
 * RDM writes the same entry model in two shapes: a .rdm (or .xml) file, which
 * is XML, and a .json file. Both are turned into plain objects first, then
 * every object that says its ConnectionType is read as an entry. RDP, SSH and
 * VNC entries become hosts; groups and credential entries are not hosts but
 * a credential entry linked from a host lends it its username and domain.
 * An SSH gateway entry (a VPN entry of the SSHGateway kind) is not a host
 * either, but becomes the jump host of the SSH entries linked to it.
 * Everything else is listed as skipped, with the reason.
 *
 * An export saved without credentials has no usernames at all: RDM keeps
 * them with the credentials. The dialog asks for one in that case.
 *
 * Passwords, passphrases and every other secret in the file are never read.
 * An export encrypted with a master key cannot be read at all: the person is
 * told to export again without one. The schema is not documented, so fields
 * are looked up by every name RDM is known to use, ignoring case.
 *
 * Kept apart from the dialog so it can be tested in Node.
 */

import type { Folder, SessionConfig, SessionKind } from '#lib/ipc/sessions.js';

export type RdmKind = Extract<SessionKind, 'ssh' | 'rdp' | 'vnc'>;

export interface RdmImportHost {
	/** Position in the export, unique within one file. */
	id: string;
	kind: RdmKind;
	name: string;
	host: string;
	port: number;
	username: string;
	/** Windows logon domain, RDP only. */
	domain: string | null;
	/** SSH only: a private key file named by the entry, on the machine RDM ran on. */
	keyPath: string | null;
	/** The RDM group path, outermost first; empty at the top level. */
	folder: string[];
	/** SSH only: the SSH gateway(s) RDM reaches the host through, outermost first. */
	jump: RdmGateway[];
}

/** One hop of an RDM SSH gateway entry; its credentials are not in the export. */
export interface RdmGateway {
	host: string;
	port: number;
}

/** Why an entry in the export could not become a session. */
export type RdmSkipReason = 'no_host' | 'bad_port' | 'unsupported';

export interface RdmSkipped {
	label: string;
	reason: RdmSkipReason;
	/** For `unsupported`: the RDM connection type, as the file names it. */
	type?: string;
}

export type RdmParseResult =
	| { ok: true; hosts: RdmImportHost[]; skipped: RdmSkipped[] }
	| { ok: false; error: 'bad_file'; detail: string }
	| { ok: false; error: 'encrypted' }
	| { ok: false; error: 'no_hosts' };

const DEFAULT_PORT: Record<RdmKind, number> = { ssh: 22, rdp: 3389, vnc: 5900 };

type Obj = Record<string, unknown>;

function isObj(value: unknown): value is Obj {
	return typeof value === 'object' && value !== null && !Array.isArray(value);
}

/** A trimmed string, or '' for anything that is not a string or a number. */
function text(value: unknown): string {
	if (typeof value === 'string') return value.trim();
	if (typeof value === 'number' && Number.isFinite(value)) return String(value);
	return '';
}

// ---------------------------------------------------------------------------
// XML
// ---------------------------------------------------------------------------

const ENTITIES: Record<string, string> = { amp: '&', lt: '<', gt: '>', quot: '"', apos: "'" };

function decodeEntities(s: string): string {
	return s.replace(/&(#x[0-9a-fA-F]+|#[0-9]+|[a-zA-Z]+);/g, (whole, ent: string) => {
		if (ent[0] === '#') {
			const code = ent[1] === 'x' || ent[1] === 'X' ? parseInt(ent.slice(2), 16) : parseInt(ent.slice(1), 10);
			return Number.isFinite(code) && code >= 0 && code <= 0x10ffff ? String.fromCodePoint(code) : whole;
		}
		return ENTITIES[ent] ?? whole;
	});
}

interface XmlElement {
	name: string;
	children: XmlElement[];
	text: string;
}

/**
 * A small XML reader for RDM's element-and-text files: attributes are
 * ignored, and so are comments, processing instructions and the doctype.
 * Throws on markup that does not nest.
 */
export function parseXml(raw: string): XmlElement {
	const root: XmlElement = { name: '#root', children: [], text: '' };
	const stack: XmlElement[] = [root];
	let i = 0;
	const n = raw.length;
	while (i < n) {
		const lt = raw.indexOf('<', i);
		const top = stack[stack.length - 1];
		if (lt === -1) {
			top.text += decodeEntities(raw.slice(i));
			break;
		}
		if (lt > i) top.text += decodeEntities(raw.slice(i, lt));
		if (raw.startsWith('<!--', lt)) {
			const end = raw.indexOf('-->', lt + 4);
			if (end === -1) throw new Error('unclosed comment');
			i = end + 3;
		} else if (raw.startsWith('<![CDATA[', lt)) {
			const end = raw.indexOf(']]>', lt + 9);
			if (end === -1) throw new Error('unclosed CDATA section');
			top.text += raw.slice(lt + 9, end);
			i = end + 3;
		} else if (raw.startsWith('<?', lt)) {
			const end = raw.indexOf('?>', lt + 2);
			if (end === -1) throw new Error('unclosed processing instruction');
			i = end + 2;
		} else if (raw.startsWith('<!', lt)) {
			const end = raw.indexOf('>', lt + 2);
			if (end === -1) throw new Error('unclosed declaration');
			i = end + 1;
		} else if (raw[lt + 1] === '/') {
			const end = raw.indexOf('>', lt + 2);
			if (end === -1) throw new Error('unclosed end tag');
			const name = raw.slice(lt + 2, end).trim();
			if (stack.length < 2 || top.name !== name) throw new Error(`unexpected </${name}>`);
			stack.pop();
			i = end + 1;
		} else {
			const end = tagEnd(raw, lt + 1);
			if (end === -1) throw new Error('unclosed start tag');
			let body = raw.slice(lt + 1, end);
			const selfClosing = body.endsWith('/');
			if (selfClosing) body = body.slice(0, -1);
			const name = body.trim().split(/\s/, 1)[0];
			if (!name) throw new Error('empty tag name');
			const el: XmlElement = { name, children: [], text: '' };
			top.children.push(el);
			if (!selfClosing) stack.push(el);
			i = end + 1;
		}
	}
	if (stack.length !== 1) throw new Error(`<${stack[stack.length - 1].name}> is not closed`);
	if (root.children.length === 0) throw new Error('no elements');
	return root;
}

/** The `>` that ends a start tag, skipping any inside quoted attribute values. */
function tagEnd(raw: string, from: number): number {
	let quote = '';
	for (let i = from; i < raw.length; i++) {
		const c = raw[i];
		if (quote) {
			if (c === quote) quote = '';
		} else if (c === '"' || c === "'") quote = c;
		else if (c === '>') return i;
	}
	return -1;
}

/**
 * An element as a plain object: one with child elements becomes an object
 * keyed by child name (an array when a name repeats), one without becomes its
 * text. Same shape as the JSON export, so one reader serves both.
 */
function xmlToValue(el: XmlElement): unknown {
	if (el.children.length === 0) return el.text.trim();
	const out: Obj = {};
	const repeated = new Set<string>();
	for (const child of el.children) {
		const value = xmlToValue(child);
		if (!(child.name in out)) out[child.name] = value;
		else if (repeated.has(child.name)) (out[child.name] as unknown[]).push(value);
		else {
			out[child.name] = [out[child.name], value];
			repeated.add(child.name);
		}
	}
	return out;
}

// ---------------------------------------------------------------------------
// Entries
// ---------------------------------------------------------------------------

/** A key of `obj` matching `name` regardless of case. */
function field(obj: Obj, name: string): unknown {
	if (name in obj) return obj[name];
	const lower = name.toLowerCase();
	for (const key of Object.keys(obj)) if (key.toLowerCase() === lower) return obj[key];
	return undefined;
}

/** The first non-empty text among dotted paths, like "Terminal.Host". */
function pick(obj: Obj, paths: string[]): string {
	for (const path of paths) {
		let cur: unknown = obj;
		for (const part of path.split('.')) {
			cur = isObj(cur) ? field(cur, part) : undefined;
		}
		const value = text(cur);
		if (value) return value;
	}
	return '';
}

/** Every object in the tree that names a ConnectionType, in file order. */
function collectEntries(value: unknown, out: Obj[]): void {
	if (Array.isArray(value)) {
		for (const v of value) collectEntries(v, out);
		return;
	}
	if (!isObj(value)) return;
	if (field(value, 'ConnectionType') !== undefined) {
		out.push(value);
		return;
	}
	for (const v of Object.values(value)) collectEntries(v, out);
}

type EntryClass = { kind: RdmKind } | { ignore: true } | { unsupported: string };

/** What an entry is, from its ConnectionType, or from its settings block when the type is a number. */
function classify(entry: Obj): EntryClass {
	const raw = text(field(entry, 'ConnectionType'));
	const type = raw.toLowerCase();
	switch (type) {
		case 'rdpconfigured':
		case 'rdp':
		case 'rdpfilename':
			return { kind: 'rdp' };
		case 'sshshell':
		case 'ssh':
			return { kind: 'ssh' };
		case 'vnc':
			return { kind: 'vnc' };
		case 'vpn':
			return isGatewayEntry(entry) ? { ignore: true } : { unsupported: raw };
		case 'putty': {
			const protocol = pick(entry, ['Putty.Protocol', 'Putty.ProtocolType', 'Protocol']).toLowerCase();
			return protocol === '' || protocol === 'ssh' ? { kind: 'ssh' } : { unsupported: `Putty (${protocol})` };
		}
		case 'group':
		case 'credential':
		case 'credentials':
		case 'dataentry':
		case 'document':
		case 'attachment':
		case 'shortcut':
			return { ignore: true };
	}
	if (/^\d+$/.test(type) || type === '') {
		// A numeric enum: fall back on which settings block the entry carries.
		if (isObj(field(entry, 'RDP'))) return { kind: 'rdp' };
		if (isObj(field(entry, 'VNC'))) return { kind: 'vnc' };
		const terminal = field(entry, 'Terminal');
		if (isObj(terminal) && pick(terminal, ['Host'])) return { kind: 'ssh' };
		if (isObj(field(entry, 'Credentials'))) return { ignore: true };
	}
	return { unsupported: raw || '?' };
}

function isGatewayEntry(entry: Obj): boolean {
	return pick(entry, ['VPN.Application']).toLowerCase() === 'sshgateway';
}

/** Every object (or the one object) under a key that may hold a list. */
function list(value: unknown): Obj[] {
	if (Array.isArray(value)) return value.filter(isObj);
	return isObj(value) ? [value] : [];
}

/** The hops of an SSH gateway entry, in order; hops without a usable address are left out. */
function gatewayHops(entry: Obj): RdmGateway[] {
	const vpn = field(entry, 'VPN');
	if (!isObj(vpn)) return [];
	const holder = field(vpn, 'SSHGateways');
	// XML gives { SSHGateway: {...} | [...] }; JSON gives the array itself.
	const items = isObj(holder) && field(holder, 'SSHGateway') !== undefined ? list(field(holder, 'SSHGateway')) : list(holder);
	const hops: RdmGateway[] = [];
	for (const g of items) {
		const address = splitAddress(pick(g, ['Host', 'HostName']));
		if (!address.host) continue;
		const portText = pick(g, ['Port', 'HostPort']) || address.port;
		const port = !portText || portText === '0' ? 22 : parsePort(portText);
		if (port !== null) hops.push({ host: address.host, port });
	}
	return hops;
}

/** A TCP port, or null when the value is not one. */
export function parsePort(value: string): number | null {
	const n = Number(value);
	return Number.isInteger(n) && n >= 1 && n <= 65535 ? n : null;
}

/**
 * The host and any port written in an RDM address: "server", "server:3390",
 * "[fe80::1]:22", "rdp://server", or a bare IPv6 address.
 */
export function splitAddress(address: string): { host: string; port: string } {
	let a = address.trim().replace(/^[a-z][a-z0-9+.-]*:\/\//i, '');
	a = a.replace(/\/.*$/, '');
	const bracket = /^\[([^\]]+)\](?::(\d*))?$/.exec(a);
	if (bracket) return { host: bracket[1], port: bracket[2] ?? '' };
	const colons = a.split(':').length - 1;
	if (colons === 1) {
		const [host, port] = a.split(':');
		return { host, port };
	}
	return { host: a, port: '' };
}

/** The group path: RDM separates levels with a backslash. */
export function splitGroup(group: string): string[] {
	return group
		.split(/[\\/]/)
		.map((s) => s.trim())
		.filter(Boolean);
}

const HOST_PATHS: Record<RdmKind, string[]> = {
	ssh: ['Terminal.Host', 'Putty.Host', 'Host', 'Url', 'HostName'],
	rdp: ['Url', 'RDP.Host', 'Host', 'HostName'],
	vnc: ['VNC.Host', 'Url', 'Host', 'HostName']
};
const PORT_PATHS: Record<RdmKind, string[]> = {
	ssh: ['Terminal.HostPort', 'Terminal.Port', 'Putty.Port', 'HostPort', 'Port'],
	rdp: ['RDP.Port', 'RDP.HostPort', 'HostPort', 'Port'],
	vnc: ['VNC.Port', 'VNC.HostPort', 'HostPort', 'Port']
};
const USER_PATHS: Record<RdmKind, string[]> = {
	ssh: ['Terminal.Username', 'Terminal.UserName', 'Putty.Username', 'UserName', 'Username'],
	rdp: ['RDP.UserName', 'RDP.Username', 'UserName', 'Username'],
	vnc: ['VNC.Username', 'VNC.UserName', 'UserName', 'Username']
};
const CREDENTIAL_USER_PATHS = ['Credentials.UserName', 'Credentials.Username', 'UserName', 'Username'];
const CREDENTIAL_DOMAIN_PATHS = ['Credentials.Domain', 'Domain'];

export function parseRdmExport(raw: string): RdmParseResult {
	const trimmed = raw.replace(/^﻿/, '').trim();
	if (!trimmed) return { ok: false, error: 'no_hosts' };

	let data: unknown;
	if (trimmed.startsWith('<')) {
		try {
			data = xmlToValue(parseXml(trimmed));
		} catch (e) {
			return { ok: false, error: 'bad_file', detail: e instanceof Error ? e.message : String(e) };
		}
	} else if (trimmed.startsWith('{') || trimmed.startsWith('[')) {
		try {
			data = JSON.parse(trimmed);
		} catch (e) {
			return { ok: false, error: 'bad_file', detail: e instanceof Error ? e.message : String(e) };
		}
	} else {
		// Neither XML nor JSON: an export sealed with a master key is one opaque blob.
		return { ok: false, error: /^[A-Za-z0-9+/=\s]+$/.test(trimmed) ? 'encrypted' : 'no_hosts' };
	}

	const entries: Obj[] = [];
	collectEntries(data, entries);
	if (entries.length === 0) {
		return { ok: false, error: looksEncrypted(data) ? 'encrypted' : 'no_hosts' };
	}

	// Credential entries, by ID, to lend a username and domain to the hosts that link them.
	const credentials = new Map<string, Obj>();
	for (const e of entries) {
		const id = text(field(e, 'ID')).toLowerCase();
		const type = text(field(e, 'ConnectionType')).toLowerCase();
		if (id && (type === 'credential' || type === 'credentials' || (!type && isObj(field(e, 'Credentials'))))) credentials.set(id, e);
	}

	// Groups that name a credential, by their path: an entry linked to a
	// credential the export doesn't hold (RDM's "inherited" link, or one in
	// another vault) takes the nearest enclosing group's.
	const groupCredentials = new Map<string, string>();
	for (const e of entries) {
		if (text(field(e, 'ConnectionType')).toLowerCase() !== 'group') continue;
		const link = text(field(e, 'CredentialConnectionID')).toLowerCase();
		if (link) groupCredentials.set(splitGroup(text(field(e, 'Group'))).join('\\').toLowerCase(), link);
	}
	const credentialFor = (entry: Obj): Obj | undefined => {
		const link = text(field(entry, 'CredentialConnectionID')).toLowerCase();
		if (!link) return undefined;
		const own = credentials.get(link);
		if (own) return own;
		const path = splitGroup(text(field(entry, 'Group')));
		for (let n = path.length; n > 0; n--) {
			const up = groupCredentials.get(path.slice(0, n).join('\\').toLowerCase());
			const found = up ? credentials.get(up) : undefined;
			if (found) return found;
		}
		return undefined;
	};

	// SSH gateway entries, by ID, for the SSH hosts that link one.
	const gateways = new Map<string, RdmGateway[]>();
	for (const e of entries) {
		const id = text(field(e, 'ID')).toLowerCase();
		if (id && isGatewayEntry(e)) gateways.set(id, gatewayHops(e));
	}

	const hosts: RdmImportHost[] = [];
	const skipped: RdmSkipped[] = [];
	entries.forEach((entry, i) => {
		const cls = classify(entry);
		if ('ignore' in cls) return;
		const name = text(field(entry, 'Name'));
		const label = name || `#${i + 1}`;
		if ('unsupported' in cls) {
			skipped.push({ label, reason: 'unsupported', type: cls.unsupported });
			return;
		}
		const kind = cls.kind;
		const address = splitAddress(pick(entry, HOST_PATHS[kind]));
		if (!address.host) {
			skipped.push({ label, reason: 'no_host' });
			return;
		}
		const portText = pick(entry, PORT_PATHS[kind]) || address.port;
		// Absent, or 0 as some RDM versions write it, means the protocol's default.
		const finalPort = !portText || portText === '0' ? DEFAULT_PORT[kind] : parsePort(portText);
		if (finalPort === null) {
			skipped.push({ label, reason: 'bad_port' });
			return;
		}

		const credential = credentialFor(entry);
		const username = pick(entry, USER_PATHS[kind]) || (credential ? pick(credential, CREDENTIAL_USER_PATHS) : '');
		let domain: string | null = null;
		if (kind === 'rdp') {
			domain = pick(entry, ['RDP.Domain', 'Domain']) || (credential ? pick(credential, CREDENTIAL_DOMAIN_PATHS) : '') || null;
		}
		let jump: RdmGateway[] = [];
		if (kind === 'ssh' && pick(entry, ['Terminal.UseSSHGateway']).toLowerCase() === 'true') {
			jump = gateways.get(pick(entry, ['Terminal.LinkedSSHGateway']).toLowerCase()) ?? [];
		}
		const keyPath = kind === 'ssh' ? pick(entry, ['Terminal.PrivateKeyPath', 'Putty.PrivateKeyPath', 'PrivateKeyPath']) || null : null;

		hosts.push({
			id: String(i),
			kind,
			name: name || address.host,
			host: address.host,
			port: finalPort,
			username,
			domain,
			keyPath,
			folder: splitGroup(text(field(entry, 'Group'))),
			jump
		});
	});

	if (hosts.length === 0 && skipped.length === 0) return { ok: false, error: 'no_hosts' };
	return { ok: true, hosts, skipped };
}

/** An export with no entries but an encrypted payload where they should be. */
function looksEncrypted(data: unknown): boolean {
	let found = false;
	const walk = (v: unknown, key: string): void => {
		if (found) return;
		if (Array.isArray(v)) v.forEach((x) => walk(x, key));
		else if (isObj(v)) for (const [k, x] of Object.entries(v)) walk(x, k);
		else if (typeof v === 'string' && (/encrypt|cipher|protected/i.test(key) || (v.length > 64 && /^[A-Za-z0-9+/=\s]+$/.test(v)))) found = true;
	};
	walk(data, '');
	return found;
}

// ---------------------------------------------------------------------------
// Matching what is already saved
// ---------------------------------------------------------------------------

/** A session of the same kind already saved for the same machine and user, as the RDP import checks. */
export function findExisting(host: RdmImportHost, sessions: SessionConfig[]): SessionConfig | undefined {
	const address = host.host.toLowerCase();
	return sessions.find(
		(s) =>
			(s.kind ?? 'ssh') === host.kind &&
			s.host.toLowerCase() === address &&
			s.port === host.port &&
			s.username === host.username
	);
}

/** A folder of that name under `parentId` in the vault, so a second import reuses it. */
export function findChildFolder(name: string, parentId: string | null, vaultId: string | null, folders: Folder[]): Folder | undefined {
	return folders.find((f) => (f.parent_id ?? null) === parentId && (f.vault_id ?? null) === vaultId && f.name === name);
}
