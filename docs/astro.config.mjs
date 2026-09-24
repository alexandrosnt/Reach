// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

/*
 * The origin and path the site is served from.
 *
 * Both are environment variables so moving to a bought domain is a deploy-time
 * change rather than an edit here and in every internal link. On GitHub Pages
 * these keep their defaults; on a custom domain the workflow sets
 * SITE_URL=https://example.com and BASE_PATH=/ and nothing else moves.
 */
const site = (process.env.SITE_URL ?? 'https://reachssh.com').replace(/\/$/, '');
const base = process.env.BASE_PATH ?? '/';

// https://astro.build/config
export default defineConfig({
	site,
	base,
	integrations: [
		starlight({
			title: 'Reach',
			description:
				'Reach is a free, open source SSH client and remote server manager for Windows, macOS, Linux and Android. Terminals, SFTP, tunnels, secrets and automation in one app.',
			// Browsers request /favicon.ico whether or not the markup asks them
			// to, so it exists regardless; this points the link tag at it too.
			// Built by scripts/make-favicons.mjs from the application's own icon,
			// so the browser tab and the taskbar show the same artwork.
			favicon: '/favicon.ico',
			head: [
				// Search engines want a square that is a multiple of 48px and
				// downscale it themselves. The .ico tops out at 48, which is the
				// smallest that qualifies and gives them nothing to work with, so
				// the larger PNGs are declared alongside it. Browsers keep using
				// the .ico; Google picks the size it wants.
				{ tag: 'link', attrs: { rel: 'icon', type: 'image/png', sizes: '48x48', href: '/icon-48.png' } },
				{ tag: 'link', attrs: { rel: 'icon', type: 'image/png', sizes: '96x96', href: '/icon-96.png' } },
				{ tag: 'link', attrs: { rel: 'icon', type: 'image/png', sizes: '144x144', href: '/icon-144.png' } },
				{
					tag: 'link',
					attrs: { rel: 'apple-touch-icon', sizes: '180x180', href: '/apple-touch-icon.png' },
				},
				{ tag: 'link', attrs: { rel: 'manifest', href: '/site.webmanifest' } },
				// What sits under the icon when someone adds the site to an iOS
				// home screen. Without it iOS uses the page title, which here is
				// fifty characters and gets truncated to "Reach — Free Op…".
				{
					tag: 'meta',
					attrs: { name: 'apple-mobile-web-app-title', content: 'Reach' },
				},
			],
			customCss: ['./src/styles/custom.css'],
			components: {
				Head: './src/components/Head.astro',
			},
			logo: {
				src: './src/assets/reach-logo.png',
			},
			social: [
				{ icon: 'github', label: 'GitHub', href: 'https://github.com/alexandrosnt/Reach' },
				{ icon: 'discord', label: 'Discord', href: 'https://discord.gg/CSbEybvDVV' },
			],
			sidebar: [
				{
					label: 'Getting Started',
					items: [
						{ label: 'Download', slug: 'download' },
						{ label: 'What is Reach?', slug: 'getting-started/introduction' },
						{ label: 'Installation', slug: 'getting-started/installation' },
						{ label: 'First Run & Setup', slug: 'getting-started/first-run' },
					],
				},
				{
					label: 'Features',
					items: [
						{ label: 'SSH Terminal', slug: 'features/ssh-terminal' },
						{ label: 'Session Manager', slug: 'features/sessions' },
						{ label: 'Host Key Verification', slug: 'features/host-keys' },
						{ label: 'File Explorer (SFTP)', slug: 'features/file-explorer' },
						{ label: 'Snippets', slug: 'features/snippets' },
						{ label: 'Port Tunneling', slug: 'features/tunnels' },
						{ label: 'System Monitoring', slug: 'features/monitoring' },
						{ label: 'Serial Console', slug: 'features/serial' },
						{ label: 'Remote Desktop (RDP)', slug: 'features/remote-desktop' },
						{ label: 'Jump Hosts & SSH Config', slug: 'features/jump-hosts' },
						{ label: 'Ansible', slug: 'features/ansible' },
						{ label: 'OpenTofu', slug: 'features/opentofu' },
						{ label: 'Plugins', slug: 'features/plugins' },
						{ label: 'Themes', slug: 'features/themes' },
						{ label: 'Marketplace', slug: 'features/marketplace' },
						{ label: 'AI Assistant', slug: 'features/ai-assistant' },
						{ label: 'MCP Server', slug: 'features/mcp-server' },
						{ label: 'Recipes', slug: 'features/recipes' },
						{ label: 'Session Sharing', slug: 'features/session-sharing' },
					],
				},
				{
					label: 'Security & Vault',
					items: [
						{ label: 'How Encryption Works', slug: 'vault/encryption' },
						{ label: 'Managing Secrets', slug: 'vault/secrets' },
						{ label: 'Sharing & Collaboration', slug: 'vault/sharing' },
						{ label: 'Backup & Restore', slug: 'vault/backup' },
					],
				},
				{
					label: 'Cloud Sync (Turso)',
					items: [
						{ label: 'What is Turso?', slug: 'sync/what-is-turso' },
						{ label: 'Setting Up Cloud Sync', slug: 'sync/setup' },
						{ label: 'Syncing Shared Vaults', slug: 'sync/shared-vaults' },
						{ label: 'Troubleshooting', slug: 'sync/troubleshooting' },
					],
				},
				{
					label: 'Settings & Customization',
					items: [
						{ label: 'General Settings', slug: 'settings/general' },
						{ label: 'Keyboard Shortcuts', slug: 'settings/shortcuts' },
						{ label: 'Auto-Updates', slug: 'settings/updates' },
					],
				},
				{
					label: 'Building from Source',
					items: [
						{ label: 'Development Setup', slug: 'development/setup' },
						{ label: 'Project Architecture', slug: 'development/architecture' },
					],
				},
			],
		}),
	],
});
