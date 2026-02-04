import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vite';

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
	plugins: [tailwindcss(), sveltekit()],
	clearScreen: false,
	build: {
		chunkSizeWarningLimit: 1000
	},
	ssr: {
		noExternal: [
			'@xterm/xterm',
			'@xterm/addon-fit',
			'@xterm/addon-webgl',
			'@xterm/addon-web-links',
			'@xterm/addon-unicode11',
			'@codemirror/view',
			'@codemirror/state',
			'@codemirror/commands',
			'@codemirror/language',
			'@codemirror/language-data',
			'@codemirror/theme-one-dark',
			'@codemirror/autocomplete',
			'@codemirror/search',
			'@codemirror/lint'
		]
	},
	server: {
		port: 1420,
		strictPort: true,
		host: host || false,
		hmr: host
			? {
					protocol: 'ws',
					host,
					port: 1421
				}
			: undefined,
		watch: {
			ignored: ['**/src-tauri/**']
		}
	}
});
