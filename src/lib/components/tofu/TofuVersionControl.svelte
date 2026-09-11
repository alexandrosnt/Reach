<!--
	Which OpenTofu this project runs, said plainly and changeable in place.

	The chip reads "OpenTofu 1.11.4 · managed" (Reach downloaded and verified
	it) or "· system PATH" (whatever the machine has), with "pinned by
	.opentofu-version" when the project asked for it. The select changes the
	pin: pick a managed version, use the PATH, or install the latest.
-->
<script lang="ts">
	import { t } from '$lib/state/i18n.svelte';
	import {
		getBinaryStatus,
		isBinaryInstalling,
		getBinaryInstallMessage,
		installBinary,
		pinBinary
	} from '$lib/state/tofu.svelte';

	let status = $derived(getBinaryStatus());
	let installing = $derived(isBinaryInstalling());
	let installMessage = $derived(getBinaryInstallMessage());

	const PATH = '__path__';
	const LATEST = '__latest__';

	let choice = $state('');
	$effect(() => {
		const s = status;
		if (!s) return;
		if (s.pinned) choice = s.pinned;
		else if (s.resolved?.source === 'managed') choice = s.resolved.version ?? '';
		else choice = PATH;
	});

	async function onChange(e: Event) {
		const v = (e.currentTarget as HTMLSelectElement).value;
		if (v === LATEST) {
			await installBinary(null);
			return;
		}
		if (v === PATH) {
			await pinBinary(null);
			return;
		}
		await pinBinary(v);
	}

	let sourceLabel = $derived.by(() => {
		if (!status?.resolved) return t('tofu.version_missing');
		return status.resolved.source === 'managed' ? t('tofu.version_managed') : t('tofu.version_path');
	});
</script>

<div class="version-control">
	<span class="chip" class:missing={!status?.resolved} title={status?.resolved?.path ?? ''}>
		<span class="name">OpenTofu</span>
		{#if installing}
			<span class="spinner"></span>
			<span class="dim">{installMessage ?? t('tofu.version_installing')}</span>
		{:else}
			<span class="ver mono">{status?.resolved?.version ?? '—'}</span>
			<span class="dim">· {sourceLabel}</span>
			{#if status?.pinned && status.pinSource}
				<span class="dim">· {t('tofu.version_pinned', { file: status.pinSource })}</span>
			{/if}
		{/if}
	</span>
	{#if status}
		<select class="target-select" value={choice} disabled={installing} onchange={onChange}>
			{#each status.installed as v (v)}
				<option value={v}>{v} · {t('tofu.version_managed')}</option>
			{/each}
			{#if status.pinned && !status.installed.includes(status.pinned)}
				<option value={status.pinned}>{status.pinned} · {t('tofu.version_missing')}</option>
			{/if}
			<option value={PATH} disabled={!status.pathAvailable}>{t('tofu.version_use_path')}</option>
			<option value={LATEST}>{t('tofu.version_install_latest')}</option>
		</select>
	{/if}
</div>

<style>
	.version-control {
		display: flex;
		align-items: center;
		gap: 10px;
		min-width: 0;
	}

	.chip {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		padding: 4px 10px;
		border: 1px solid var(--color-border);
		border-radius: 10px;
		font-size: 0.75rem;
		color: var(--color-text-secondary);
		white-space: nowrap;
	}

	.chip.missing {
		border-color: color-mix(in srgb, var(--color-warning, #f59e0b) 45%, var(--color-border));
	}

	.name {
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.mono {
		font-family: var(--font-mono, monospace);
		color: var(--color-text-primary);
	}

	.dim {
		color: var(--color-text-tertiary);
	}

	.spinner {
		width: 10px;
		height: 10px;
		border: 2px solid color-mix(in srgb, var(--color-accent) 30%, transparent);
		border-top-color: var(--color-accent);
		border-radius: 50%;
		animation: spin 0.8s linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}

	.target-select {
		padding: 5px 10px;
		font-size: 0.8125rem;
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		font-family: inherit;
	}

	.target-select:disabled {
		opacity: 0.6;
	}
</style>
