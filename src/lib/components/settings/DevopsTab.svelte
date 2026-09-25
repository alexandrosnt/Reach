<script lang="ts">
	/**
	 * DevOps settings: one switch per workspace.
	 *
	 * A tool that is off is gone from the tab bar and its commands are refused
	 * by the backend, but nothing it saved is touched. Each row also says
	 * whether the tool is installed here, since that is usually the question
	 * behind switching it on.
	 */
	import { onMount } from 'svelte';
	import Toggle from '$lib/components/shared/Toggle.svelte';
	import { toolchainCheck, type ToolStatus } from '$lib/ipc/toolchain';
	import { DEVOPS_TOOLS, isDevopsEnabled, setDevopsEnabled, type DevopsTool } from '$lib/state/devops.svelte';
	import { t } from '$lib/state/i18n.svelte';

	let found = $state<Record<string, ToolStatus | null>>({});

	onMount(() => {
		for (const tool of DEVOPS_TOOLS.filter((d) => d.binary)) {
			toolchainCheck(tool.binary)
				.then((status) => (found[tool.id] = status))
				.catch(() => (found[tool.id] = null));
		}
	});

	function status(tool: DevopsTool): string {
		if (isDevopsEnabled(tool.id) && tool.isBusy()) return t('devops.busy');
		if (!tool.binary) return t('devops.built_in');
		const s = found[tool.id];
		if (s === undefined) return '';
		if (!s?.installed) return t('devops.not_installed');
		return s.version ? t('devops.installed_version', { version: s.version }) : t('devops.installed');
	}
</script>

<div class="tab-content">
	<p class="intro">{t('devops.intro')}</p>

	{#each DEVOPS_TOOLS as tool (tool.id)}
		{@const on = isDevopsEnabled(tool.id)}
		<div class="setting-row">
			<div class="setting-info">
				<span class="setting-label">{tool.label()}</span>
				<span class="setting-description">{tool.description()}</span>
				{#if status(tool)}
					<span class="setting-description mono">{status(tool)}</span>
				{/if}
			</div>
			<div class="setting-control">
				<Toggle
					hideLabel
					checked={on}
					label={tool.label()}
					disabled={on && tool.isBusy()}
					onchange={(value) => setDevopsEnabled(tool.id, value)}
				/>
			</div>
		</div>
	{/each}
</div>

<style>
	.intro {
		margin: 0 0 4px;
		font-size: 0.75rem;
		line-height: 1.45;
		color: var(--color-text-secondary);
	}
</style>
