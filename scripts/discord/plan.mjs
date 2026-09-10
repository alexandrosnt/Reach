/**
 * The shape this server should have: roles first, then channels.
 *
 * Edit this file, not the runner. `setup.mjs` reads it, compares it against
 * what the guild already has, and creates only what is missing.
 *
 * Two rules keep this honest:
 *
 *   1. Roles are named here and referenced by name in channels. The runner
 *      resolves names to ids and throws on one it cannot find, so a renamed
 *      role fails the run rather than silently leaving a channel open.
 *   2. Permissions are named, never numeric. `bits()` rejects an unknown
 *      name, so a typo cannot quietly grant nothing.
 */

import { TALK } from './permissions.mjs';

// ---------------------------------------------------------------------------
// roles
// ---------------------------------------------------------------------------

/**
 * Highest first. The runner assigns positions in this order, directly beneath
 * the bot's own role — Discord forbids creating a role above yourself, so the
 * bot's managed role will sit on top until you remove it.
 *
 * `hoist` displays the role as its own group in the member list. Only the
 * three tiers that carry responsibility are hoisted; a member list with nine
 * headings is not a structure, it is a wall.
 */
export const ROLES = [
	{
		name: 'Maintainer',
		color: 0xe55a5a,
		hoist: true,
		mentionable: true,
		permissions: ['ADMINISTRATOR'],
		purpose: 'Owns the project. Administrator, so it bypasses every channel override below.'
	},
	{
		name: 'Moderator',
		color: 0xe8913a,
		hoist: true,
		mentionable: true,
		permissions: [
			...TALK,
			'MANAGE_MESSAGES',
			'MANAGE_THREADS',
			'MODERATE_MEMBERS',
			'KICK_MEMBERS',
			'BAN_MEMBERS',
			'MANAGE_NICKNAMES',
			'VIEW_AUDIT_LOG',
			'MUTE_MEMBERS',
			'DEAFEN_MEMBERS',
			'MOVE_MEMBERS',
			'CREATE_INSTANT_INVITE'
		],
		purpose: 'Keeps order. Deliberately has no MANAGE_CHANNELS, MANAGE_ROLES or MANAGE_GUILD: moderation should not be able to rewrite the server.'
	},
	{
		name: 'Core Contributor',
		color: 0x3b82f6,
		hoist: true,
		mentionable: true,
		permissions: [...TALK, 'MANAGE_THREADS', 'CREATE_PRIVATE_THREADS', 'MANAGE_EVENTS'],
		purpose: 'Trusted with the codebase. Can tidy threads and run events, nothing destructive.'
	},

	// Recognition roles. No powers, on purpose — they colour a name and make a
	// group pingable, and that is the whole job.
	{
		name: 'Contributor',
		color: 0x5ac8a8,
		hoist: false,
		mentionable: true,
		permissions: [],
		purpose: 'Has had a pull request merged.'
	},
	{
		name: 'Translator',
		color: 0x9b7be8,
		hoist: false,
		mentionable: true,
		permissions: [],
		purpose: 'Maintains one of the locales.'
	},
	{
		name: 'Theme Author',
		color: 0xe86fb0,
		hoist: false,
		mentionable: true,
		permissions: [],
		purpose: 'Has a theme in the registry.'
	},
	{
		name: 'Plugin Author',
		color: 0x4fb8d8,
		hoist: false,
		mentionable: true,
		permissions: [],
		purpose: 'Has a plugin in the marketplace.'
	},
	{
		name: 'Bug Hunter',
		color: 0xc9a227,
		hoist: false,
		mentionable: true,
		permissions: [],
		purpose: 'Reported something real, reproducibly.'
	},

	// Opt-in ping. Kept last so it sits at the bottom and never colours anyone.
	{
		name: 'Release Notifications',
		color: 0,
		hoist: false,
		mentionable: true,
		permissions: [],
		purpose: 'Self-assigned. Ping this instead of @everyone when a version ships.'
	}
];

const STAFF = ['Maintainer', 'Moderator'];

/**
 * What a brand new member can do anywhere, before any channel override.
 *
 * Everything above this line is opt-in. Notably absent: MENTION_EVERYONE, so
 * nobody can ping the server, and every MANAGE_* bit, so nobody can reshape it.
 * CREATE_INSTANT_INVITE stays on because an open-source community that cannot
 * invite anyone does not grow; drop it here if you would rather hand out one
 * link yourself.
 */
export const EVERYONE_PERMISSIONS = [...TALK, 'CHANGE_NICKNAME', 'CONNECT', 'SPEAK', 'CREATE_INSTANT_INVITE'];

// ---------------------------------------------------------------------------
// channels
// ---------------------------------------------------------------------------

/**
 * `emoji`      prefixes the channel's display name. Matching ignores it and
 *              keys off `name`, so changing an emoji renames the channel in
 *              place rather than creating a second one.
 * `readOnly`   denies SEND_MESSAGES to @everyone. Reactions and history stay
 *              open, so people can still respond without derailing the channel.
 * `writers`    grants SEND_MESSAGES back to named roles inside a readOnly
 *              channel.
 * `private`    hides the channel from @everyone and reveals it to named roles.
 * `webhook`    creates a webhook whose URL is written to .discord/webhooks.json.
 */
export const PLAN = [
	{
		category: 'INFORMATION',
		emoji: '📌',
		channels: [
			{
				name: 'welcome',
				emoji: '👋',
				topic: 'What Reach is, how to get it, and how this server works. Start here.',
				readOnly: true,
				writers: STAFF,
				noThreads: true
			},
			{
				name: 'announcements',
				emoji: '📣',
				topic: 'Release notes and project news, posted by the maintainers.',
				readOnly: true,
				writers: STAFF,
				announcement: true
			},
			{
				name: 'roles',
				emoji: '🎭',
				topic: 'Pick up Release Notifications here, and see what the other roles mean.',
				readOnly: true,
				writers: STAFF
			},
			{
				name: 'releases',
				emoji: '🚀',
				topic: 'New Reach versions land here automatically from GitHub. Read-only.',
				readOnly: true,
				webhook: 'GitHub Releases'
			}
		]
	},
	{
		category: 'DEVELOPMENT',
		emoji: '💻',
		channels: [
			{
				name: 'dev-chat',
				emoji: '🔧',
				topic: 'Building Reach: the Rust/Tauri backend, the Svelte frontend, and everything between.'
			},
			{
				name: 'github',
				emoji: '🐙',
				topic: 'Automatic feed of commits, pull requests and issues from the Reach repository. Read-only.',
				readOnly: true,
				webhook: 'GitHub Activity'
			},
			{
				name: 'translations',
				emoji: '🌍',
				topic: 'Reach ships in 9 languages. Coordinate new locales and corrections here — every key must exist in every locale.'
			},
			{
				name: 'themes',
				emoji: '🎨',
				topic: 'Theme authoring and the theme registry. A theme is data: 16 UI tokens and 22 terminal colours.'
			},
			{
				name: 'plugins',
				emoji: '🧩',
				topic: 'Plugin development and the marketplace.'
			},
			{
				name: 'mcp',
				emoji: '🤖',
				topic: 'The MCP server that exposes terminal sessions to AI clients: agents, modes, guards and skills.'
			}
		]
	},
	{
		category: 'SUPPORT',
		emoji: '🆘',
		channels: [
			{
				name: 'help',
				emoji: '❓',
				topic: 'Stuck connecting, syncing or building? Ask here.'
			},
			{
				name: 'bug-reports',
				emoji: '🐛',
				topic: 'Something broken? Say which OS, which Reach version, what you did and what you expected.'
			},
			{
				name: 'feature-requests',
				emoji: '💡',
				topic: 'Ideas for Reach. Search before posting so each idea keeps one thread.'
			}
		]
	},
	{
		category: 'COMMUNITY',
		emoji: '🌐',
		channels: [
			{
				name: 'general',
				emoji: '💬',
				topic: 'General chat for Reach users.'
			},
			{
				name: 'showcase',
				emoji: '✨',
				topic: 'Show your setup: themes, layouts, workflows.'
			},
			{
				name: 'off-topic',
				emoji: '🎲',
				topic: 'Everything else.'
			}
		]
	},
	{
		category: 'STAFF',
		emoji: '🔒',
		channels: [
			{
				name: 'staff-chat',
				emoji: '🔐',
				topic: 'Private to Maintainer and Moderator.',
				private: STAFF
			},
			{
				name: 'mod-log',
				emoji: '📋',
				topic: 'Moderation actions and notes. Private to Maintainer and Moderator.',
				private: STAFF
			}
		]
	}
];
