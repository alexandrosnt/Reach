// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

// https://astro.build/config
export default defineConfig({
	site: 'https://alexandrosnt.github.io',
	base: '/Reach',
	integrations: [
		starlight({
			title: 'Reach Docs',
			logo: {
				src: './src/assets/reach-logo.png',
			},
			social: [{ icon: 'github', label: 'GitHub', href: 'https://github.com/alexandrosnt/Reach' }],
			sidebar: [
				{
					label: 'Getting Started',
					items: [
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
						{ label: 'File Explorer (SFTP)', slug: 'features/file-explorer' },
						{ label: 'Port Tunneling', slug: 'features/tunnels' },
						{ label: 'Playbooks', slug: 'features/playbooks' },
						{ label: 'System Monitoring', slug: 'features/monitoring' },
						{ label: 'Serial Console', slug: 'features/serial' },
						{ label: 'AI Assistant', slug: 'features/ai-assistant' },
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
