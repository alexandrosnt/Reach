/**
 * The channel structure this server should have.
 *
 * Edit this file, not the runner. `setup.mjs` reads it, compares it against
 * what the guild already has, and creates only what is missing — so changing
 * a topic here and re-running will NOT rewrite an existing channel's topic.
 * Rename or retopic those by hand; this script only ever adds.
 *
 * `readOnly` denies SEND_MESSAGES to @everyone. Reactions and threads stay
 * open, so people can still respond to an announcement without derailing it.
 *
 * `webhook` asks for a webhook on that channel, whose URL is written to
 * `.discord/webhooks.json` (gitignored) for you to paste into GitHub.
 */

export const PLAN = [
	{
		category: 'INFORMATION',
		channels: [
			{
				name: 'welcome',
				topic: 'What Reach is, how to get it, and how this server works. Start here.',
				readOnly: true
			},
			{
				name: 'announcements',
				topic: 'Release notes and project news, posted by the maintainers.',
				readOnly: true,
				announcement: true
			},
			{
				name: 'releases',
				topic: 'New Reach versions land here automatically from GitHub. Read-only.',
				readOnly: true,
				webhook: 'GitHub Releases'
			}
		]
	},
	{
		category: 'DEVELOPMENT',
		channels: [
			{
				name: 'dev-chat',
				topic: 'Building Reach: the Rust/Tauri backend, the Svelte frontend, and everything between.'
			},
			{
				name: 'github',
				topic: 'Automatic feed of commits, pull requests and issues from the Reach repository. Read-only.',
				readOnly: true,
				webhook: 'GitHub Activity'
			},
			{
				name: 'translations',
				topic: 'Reach ships in 9 languages. Coordinate new locales and corrections here — every key must exist in every locale.'
			},
			{
				name: 'themes',
				topic: 'Theme authoring and the theme registry. A theme is data: 16 UI tokens and 22 terminal colours.'
			},
			{
				name: 'plugins',
				topic: 'Plugin development and the marketplace.'
			},
			{
				name: 'mcp',
				topic: 'The MCP server that exposes terminal sessions to AI clients: agents, modes, guards and skills.'
			}
		]
	},
	{
		category: 'SUPPORT',
		channels: [
			{
				name: 'help',
				topic: 'Stuck connecting, syncing or building? Ask here.'
			},
			{
				name: 'bug-reports',
				topic: 'Something broken? Say which OS, which Reach version, what you did and what you expected.'
			},
			{
				name: 'feature-requests',
				topic: 'Ideas for Reach. Search before posting so each idea keeps one thread.'
			}
		]
	},
	{
		category: 'COMMUNITY',
		channels: [
			{
				name: 'general',
				topic: 'General chat for Reach users.'
			},
			{
				name: 'showcase',
				topic: 'Show your setup: themes, layouts, workflows.'
			},
			{
				name: 'off-topic',
				topic: 'Everything else.'
			}
		]
	}
];
