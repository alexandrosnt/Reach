/**
 * Discord permission bits.
 *
 * These are a 64-bit field, so they are BigInt here and strings on the wire —
 * a plain JS number silently loses precision above 2^53, which is where
 * MANAGE_THREADS and everything after it lives.
 *
 * Only the bits this server actually uses are listed. Adding one means adding
 * it here first, which is the point: `bits()` throws on a name it does not
 * know, so a typo fails loudly instead of quietly granting a role nothing.
 */

export const P = {
	CREATE_INSTANT_INVITE: 1n << 0n,
	KICK_MEMBERS: 1n << 1n,
	BAN_MEMBERS: 1n << 2n,
	ADMINISTRATOR: 1n << 3n,
	MANAGE_CHANNELS: 1n << 4n,
	MANAGE_GUILD: 1n << 5n,
	ADD_REACTIONS: 1n << 6n,
	VIEW_AUDIT_LOG: 1n << 7n,
	VIEW_CHANNEL: 1n << 10n,
	SEND_MESSAGES: 1n << 11n,
	MANAGE_MESSAGES: 1n << 13n,
	EMBED_LINKS: 1n << 14n,
	ATTACH_FILES: 1n << 15n,
	READ_MESSAGE_HISTORY: 1n << 16n,
	MENTION_EVERYONE: 1n << 17n,
	USE_EXTERNAL_EMOJIS: 1n << 18n,
	CONNECT: 1n << 20n,
	SPEAK: 1n << 21n,
	MUTE_MEMBERS: 1n << 22n,
	DEAFEN_MEMBERS: 1n << 23n,
	MOVE_MEMBERS: 1n << 24n,
	CHANGE_NICKNAME: 1n << 26n,
	MANAGE_NICKNAMES: 1n << 27n,
	MANAGE_ROLES: 1n << 28n,
	MANAGE_WEBHOOKS: 1n << 29n,
	USE_APPLICATION_COMMANDS: 1n << 31n,
	MANAGE_EVENTS: 1n << 33n,
	MANAGE_THREADS: 1n << 34n,
	CREATE_PUBLIC_THREADS: 1n << 35n,
	CREATE_PRIVATE_THREADS: 1n << 36n,
	SEND_MESSAGES_IN_THREADS: 1n << 38n,
	MODERATE_MEMBERS: 1n << 40n
};

/** Fold permission names into the string the API expects. */
export function bits(names) {
	let acc = 0n;
	for (const name of names) {
		if (!(name in P)) throw new Error(`Unknown permission: ${name}`);
		acc |= P[name];
	}
	return acc.toString();
}

/** Everything a person needs to take part in a normal conversation. */
export const TALK = [
	'VIEW_CHANNEL',
	'SEND_MESSAGES',
	'SEND_MESSAGES_IN_THREADS',
	'READ_MESSAGE_HISTORY',
	'ADD_REACTIONS',
	'EMBED_LINKS',
	'ATTACH_FILES',
	'USE_EXTERNAL_EMOJIS',
	'CREATE_PUBLIC_THREADS',
	'USE_APPLICATION_COMMANDS'
];
