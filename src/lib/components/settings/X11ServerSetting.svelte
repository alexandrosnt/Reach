<script lang="ts">
	/**
	 * Settings, General: the X server for X11 forwarding. On Windows Reach
	 * downloads VcXsrv only when this is turned on (a progress bar with
	 * Cancel, then it is ready); turning it off removes it. Elsewhere the
	 * system's X server is used and this only says so.
	 */
	import { onMount } from 'svelte';
	import Toggle from '#lib/components/shared/Toggle.svelte';
	import { addToast } from '#lib/state/toasts.svelte.js';
	import { t } from '#lib/state/i18n.svelte.js';
	import { isMac } from '#lib/platform.js';
	import {
		x11ServerStatus,
		x11ServerInstall,
		x11ServerCancel,
		x11ServerRemove,
		onX11ServerProgress,
		type X11ServerStatus,
		type X11ServerProgress
	} from '#lib/ipc/x11server.js';

	let status = $state<X11ServerStatus | null>(null);
	let busy = $state(false);
	let progress = $state<X11ServerProgress | null>(null);

	onMount(() => {
		void x11ServerStatus().then((s) => (status = s));
		const off = onX11ServerProgress((p) => (progress = p));
		return () => void off.then((f) => f());
	});

	let percent = $derived(progress && progress.total > 0 ? Math.min(100, Math.round((progress.done / progress.total) * 100)) : 0);

	function mb(n: number): string {
		return (n / 1048576).toFixed(1);
	}

	let stageText = $derived.by(() => {
		if (!progress) return t('settings.x11_preparing');
		if (progress.stage === 'download') return t('settings.x11_downloading', { done: mb(progress.done), total: mb(progress.total) });
		if (progress.stage === 'verify') return t('settings.x11_verifying');
		return t('settings.x11_unpacking', { done: String(progress.done), total: String(progress.total) });
	});

	async function onChange(on: boolean): Promise<void> {
		if (!status) return;
		if (on) {
			busy = true;
			progress = null;
			try {
				await x11ServerInstall();
				status = { ...status, installed: true };
				addToast(t('settings.x11_ready'), 'success');
			} catch (e) {
				const msg = String(e);
				if (!msg.includes('cancelled')) addToast(t('settings.x11_failed', { error: msg }), 'error');
				status = await x11ServerStatus();
			} finally {
				busy = false;
				progress = null;
			}
		} else {
			try {
				await x11ServerRemove();
				status = { ...status, installed: false };
			} catch (e) {
				addToast(String(e), 'error');
				status = await x11ServerStatus();
			}
		}
	}
</script>

{#if status}
	<div class="setting-row">
		<div class="setting-info">
			<span class="setting-label">{t('settings.x11_server')}</span>
			<span class="setting-description">
				{#if !status.supported}
					{isMac() ? t('settings.x11_mac') : t('settings.x11_system')}
				{:else if status.installed}
					{t('settings.x11_installed', { version: status.version })}
				{:else}
					{t('settings.x11_desc')}
				{/if}
			</span>
		</div>
		{#if status.supported}
			<div class="setting-control">
				<Toggle hideLabel checked={status.installed || busy} disabled={busy} label={t('settings.x11_server')} onchange={onChange} />
			</div>
			{#if busy}
				<div class="x11-progress" role="status" aria-live="polite">
					<div class="bar" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow={percent}>
						<div class="fill" style:width="{percent}%"></div>
					</div>
					<div class="line">
						<span class="stage">{stageText}</span>
						<button type="button" class="cancel" onclick={() => void x11ServerCancel()}>{t('common.cancel')}</button>
					</div>
				</div>
			{/if}
		{/if}
	</div>
{/if}

<style>
	.x11-progress {
		display: flex;
		flex-direction: column;
		gap: 6px;
		width: 100%;
	}

	.bar {
		height: 6px;
		border-radius: 3px;
		background: var(--color-surface-active);
		overflow: hidden;
	}

	.fill {
		height: 100%;
		background: var(--color-accent);
		transition: width 0.2s ease;
	}

	.line {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
		font-size: 0.75rem;
		color: var(--color-text-secondary);
	}

	.cancel {
		border: 1px solid var(--color-border);
		border-radius: 6px;
		background: none;
		color: var(--color-text-primary);
		padding: 3px 10px;
		font: inherit;
		cursor: pointer;
	}

	.cancel:hover {
		background: var(--color-surface-hover);
	}
</style>
