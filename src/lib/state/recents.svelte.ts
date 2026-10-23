/**
 * The sessions connected to most recently on this device, newest first, for
 * the start screen. Ids only, in local storage: nothing about a session
 * (host, user, secret) is kept here, and nothing of it syncs.
 */

const KEY = 'reach-recent-sessions';
const MAX = 8;

function load(): string[] {
	try {
		const v = JSON.parse(localStorage.getItem(KEY) ?? '[]');
		return Array.isArray(v) ? v.filter((x): x is string => typeof x === 'string').slice(0, MAX) : [];
	} catch {
		return [];
	}
}

let recent = $state<string[]>(typeof localStorage === 'undefined' ? [] : load());

export function recentSessionIds(): string[] {
	return recent;
}

export function noteSessionUsed(id: string): void {
	recent = [id, ...recent.filter((x) => x !== id)].slice(0, MAX);
	try {
		localStorage.setItem(KEY, JSON.stringify(recent));
	} catch {
		// Storage full or unavailable: the list just is not remembered.
	}
}

/** A deleted session leaves the list. */
export function forgetSession(id: string): void {
	if (!recent.includes(id)) return;
	recent = recent.filter((x) => x !== id);
	try {
		localStorage.setItem(KEY, JSON.stringify(recent));
	} catch {
		// As above.
	}
}
