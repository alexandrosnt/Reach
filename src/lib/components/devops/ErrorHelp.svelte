<script lang="ts">
	/**
	 * A connection error, explained: what is wrong, and how to fix it, with
	 * the exact command to run on the server and a button to copy it. The
	 * least-privilege fix comes first. The original message stays below, for
	 * searching.
	 */
	import { writeText } from '@tauri-apps/plugin-clipboard-manager';
	import { t } from '#lib/state/i18n.svelte.js';
	import { addToast } from '#lib/state/toasts.svelte.js';

	let { error, onretry }: { error: string; onretry?: () => void } = $props();

	interface Help {
		title: string;
		body: string;
		commands: { label: string; command: string; warning?: string }[];
	}

	let help = $derived.by((): Help | null => {
		const e = error.toLowerCase();
		if (e.includes('docker.sock') && e.includes('permission denied')) {
			return {
				title: t('help.docker_permission_title'),
				body: t('help.docker_permission_body'),
				commands: [
					{ label: t('help.docker_rootless'), command: 'dockerd-rootless-setuptool.sh install' },
					{ label: t('help.docker_group'), command: 'sudo usermod -aG docker "$USER"', warning: t('help.docker_group_warning') }
				]
			};
		}
		const socket = /systemctl --user enable --now podman\.socket/.exec(error);
		if (socket) {
			return { title: t('help.podman_socket_title'), body: t('help.podman_socket_body'), commands: [{ label: t('help.run_on_server'), command: socket[0] }] };
		}
		if (/(docker|podman)(: | )?(command )?not found|no such file or directory.*(docker|podman)|exit 127/.test(e)) {
			return { title: t('help.engine_missing_title'), body: t('help.engine_missing_body'), commands: [] };
		}
		if (e.includes('is turned off')) {
			return { title: t('help.tool_off_title'), body: error, commands: [] };
		}
		if (e.includes('read-only')) {
			return { title: t('help.read_only_title'), body: t('help.read_only_body'), commands: [] };
		}
		if (e.includes('could not reach the api server')) {
			return { title: t('help.k8s_unreachable_title'), body: t('help.k8s_unreachable_body'), commands: [] };
		}
		if (e.includes('unauthorized') || e.includes('(401)')) {
			return { title: t('help.k8s_unauthorized_title'), body: t('help.k8s_unauthorized_body'), commands: [] };
		}
		if (e.includes('forbidden') || e.includes('(403)')) {
			return { title: t('help.k8s_forbidden_title'), body: t('help.k8s_forbidden_body'), commands: [] };
		}
		return null;
	});

	async function copy(text: string): Promise<void> {
		await writeText(text);
		addToast(t('common.copied'), 'success');
	}
</script>

<div class="help" role="alert">
	<div class="title">{help?.title ?? t('help.generic_title')}</div>
	{#if help?.body}
		<p class="body">{help.body}</p>
	{/if}
	{#each help?.commands ?? [] as c (c.command)}
		<div class="fix">
			<span class="label">{c.label}</span>
			<div class="cmd">
				<code>{c.command}</code>
				<button type="button" onclick={() => copy(c.command)}>{t('common.copy')}</button>
			</div>
			{#if c.warning}
				<span class="warning">{c.warning}</span>
			{/if}
		</div>
	{/each}
	<details>
		<summary>{t('help.details')}</summary>
		<pre>{error}</pre>
	</details>
	{#if onretry}
		<button type="button" class="retry" onclick={onretry}>{t('help.try_again')}</button>
	{/if}
</div>

<style>
	.help {
		display: flex;
		flex-direction: column;
		gap: 10px;
		max-width: 560px;
		padding: 16px;
		border-radius: 10px;
		border: 1px solid color-mix(in srgb, var(--color-danger) 35%, var(--color-border));
		background: color-mix(in srgb, var(--color-danger) 6%, var(--color-bg-elevated));
		font-size: var(--text-sm);
		color: var(--color-text-primary);
	}

	.title {
		font-weight: 600;
	}

	.body {
		margin: 0;
		line-height: 1.5;
		color: var(--color-text-secondary);
	}

	.fix {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.label {
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
	}

	.cmd {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 6px 6px 6px 10px;
		border-radius: var(--radius-sm);
		background: var(--color-bg-primary);
	}

	code {
		flex: 1;
		min-width: 0;
		overflow-x: auto;
		white-space: nowrap;
		font-family: var(--font-mono);
		font-size: var(--text-xs);
	}

	.cmd button,
	.retry {
		min-height: 28px;
		padding: 0 10px;
		border-radius: var(--radius-btn);
		border: 1px solid var(--color-border);
		background: var(--color-bg-elevated);
		color: var(--color-text-primary);
		font: inherit;
		font-size: var(--text-xs);
		cursor: pointer;
	}

	.retry {
		align-self: flex-start;
		min-height: 34px;
		padding: 0 14px;
		font-size: var(--text-sm);
		border-color: var(--color-accent);
		color: var(--color-accent);
	}

	.warning {
		font-size: var(--text-xs);
		color: var(--color-warning);
	}

	details {
		font-size: var(--text-xs);
		color: var(--color-text-tertiary);
	}

	pre {
		margin: 6px 0 0;
		white-space: pre-wrap;
		overflow-wrap: anywhere;
		font-family: var(--font-mono);
	}
</style>
