/**
 * Saved sessions as inventory hosts.
 *
 * Reach already knows, for every saved session, what an inventory entry
 * needs: address, port, user, the key, the bastion in front of it. No other
 * Ansible GUI can offer this, because none of them are also the SSH
 * client. What it will not do is copy a password into an inventory file —
 * a password session becomes a host with no credential and a note saying
 * so; Ansible asks for it, or a key replaces it.
 */
import type { SessionConfig } from '$lib/ipc/sessions';
import type { AnsibleInventoryHost, AnsibleInventoryGroup } from '$lib/ipc/ansible';

/** Inventory names are words: letters, digits, `_`, `-`, `.`. */
export function inventoryName(raw: string): string {
	const cleaned = raw
		.trim()
		.replace(/[^A-Za-z0-9_.-]+/g, '-')
		.replace(/^-+|-+$/g, '')
		.replace(/-{2,}/g, '-');
	return cleaned || 'host';
}

/** A name not already taken, by suffixing `-2`, `-3`, … */
export function uniqueName(base: string, taken: Set<string>): string {
	if (!taken.has(base)) return base;
	for (let i = 2; ; i++) {
		const candidate = `${base}-${i}`;
		if (!taken.has(candidate)) return candidate;
	}
}

export interface ConvertedHost {
	host: AnsibleInventoryHost;
	/** Why the entry is incomplete, when it is. */
	note: string | null;
}

/**
 * One session → one host. `existing` supplies the taken names.
 * Tags become groups; a key becomes `ansible_ssh_private_key_file`; the
 * first jump hop becomes a `ProxyJump` in `ansible_ssh_common_args`.
 */
export function hostFromSession(session: SessionConfig, existing: AnsibleInventoryHost[]): ConvertedHost {
	const taken = new Set(existing.map((h) => h.name));
	const name = uniqueName(inventoryName(session.name || session.host), taken);
	const variables: Record<string, string> = {};
	let note: string | null = null;

	switch (session.auth_method?.type) {
		case 'Key':
			if (session.auth_method.path) variables.ansible_ssh_private_key_file = session.auth_method.path;
			break;
		case 'Password':
			note = 'password';
			break;
		default:
			break;
	}

	const hop = session.jump_chain?.[0];
	if (hop) {
		const port = hop.port && hop.port !== 22 ? `:${hop.port}` : '';
		variables.ansible_ssh_common_args = `-o ProxyJump=${hop.username}@${hop.host}${port}`;
	}

	return {
		host: {
			name,
			ansibleHost: session.host,
			ansiblePort: session.port && session.port !== 22 ? session.port : null,
			ansibleUser: session.username || null,
			groups: (session.tags ?? []).map(inventoryName).filter((g, i, a) => a.indexOf(g) === i),
			variables
		},
		note
	};
}

/** True when the inventory already has this session's address and user. */
export function alreadyInInventory(session: SessionConfig, existing: AnsibleInventoryHost[]): boolean {
	return existing.some(
		(h) =>
			(h.ansibleHost ?? h.name) === session.host &&
			(h.ansibleUser ?? '') === (session.username ?? '') &&
			(h.ansiblePort ?? 22) === (session.port ?? 22)
	);
}

/** Groups the converted hosts reference that the inventory lacks yet. */
export function missingGroups(hosts: AnsibleInventoryHost[], groups: AnsibleInventoryGroup[]): AnsibleInventoryGroup[] {
	const have = new Set(groups.map((g) => g.name));
	const out: AnsibleInventoryGroup[] = [];
	for (const h of hosts) {
		for (const g of h.groups) {
			if (!have.has(g)) {
				have.add(g);
				out.push({ name: g, variables: {}, children: [] });
			}
		}
	}
	return out;
}
