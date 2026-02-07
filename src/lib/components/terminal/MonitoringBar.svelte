<script lang="ts">
	import { getStats } from '$lib/state/monitoring.svelte';
	import { t } from '$lib/state/i18n.svelte';

	interface Props {
		connectionId?: string;
		sshUser?: string;
	}

	let { connectionId, sshUser }: Props = $props();

	let stats = $derived(connectionId ? getStats(connectionId) : undefined);

	let cpuColor = $derived.by(() => {
		if (!stats) return 'var(--color-text-secondary)';
		if (stats.cpu >= 85) return 'var(--color-danger)';
		if (stats.cpu >= 60) return 'var(--color-warning)';
		return 'var(--color-success)';
	});

	let ramPercent = $derived(stats && stats.ramTotal > 0 ? Math.round((stats.ramUsed / stats.ramTotal) * 100) : 0);

	let ramColor = $derived.by(() => {
		if (ramPercent >= 85) return 'var(--color-danger)';
		if (ramPercent >= 60) return 'var(--color-warning)';
		return 'var(--color-success)';
	});

	let diskColor = $derived.by(() => {
		if (!stats) return 'var(--color-text-secondary)';
		if (stats.disk >= 85) return 'var(--color-danger)';
		if (stats.disk >= 60) return 'var(--color-warning)';
		return 'var(--color-success)';
	});

	let usersText = $derived.by(() => {
		if (!stats || stats.users.length === 0) return t('monitoring.no_users');
		if (stats.users.length === 1) return t('monitoring.one_user');
		return t('monitoring.n_users', { count: stats.users.length });
	});

	// Format user entries for the tooltip: "root@pts/0 (you)" or "deploy@pts/1"
	let usersList = $derived.by(() => {
		if (!stats) return [];
		return stats.users.map((entry) => {
			const username = entry.split('@')[0];
			const isYou = sshUser && username === sshUser;
			return { entry, isYou };
		});
	});

	let showTooltip = $state(false);
</script>

{#if stats}
	<div class="monitoring-bar">
		<div class="gauge">
			<span class="gauge-label">{t('monitoring.cpu')}</span>
			<div class="gauge-track">
				<div
					class="gauge-fill"
					style="width:{stats.cpu}%;background-color:{cpuColor}"
				></div>
			</div>
			<span class="gauge-value" style="color:{cpuColor}">{stats.cpu}%</span>
		</div>

		<div class="gauge">
			<span class="gauge-label">{t('monitoring.ram')}</span>
			<div class="gauge-track">
				<div
					class="gauge-fill"
					style="width:{ramPercent}%;background-color:{ramColor}"
				></div>
			</div>
			<span class="gauge-value" style="color:{ramColor}">{ramPercent}%</span>
		</div>

		<div class="gauge">
			<span class="gauge-label">{t('monitoring.disk')}</span>
			<div class="gauge-track">
				<div
					class="gauge-fill"
					style="width:{stats.disk}%;background-color:{diskColor}"
				></div>
			</div>
			<span class="gauge-value" style="color:{diskColor}">{stats.disk}%</span>
		</div>

		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div
			class="users"
			onmouseenter={() => (showTooltip = true)}
			onmouseleave={() => (showTooltip = false)}
		>
			<span class="users-text">{usersText}</span>
			{#if showTooltip && usersList.length > 0}
				<div class="users-tooltip">
					{#each usersList as { entry, isYou }}
						<div class="tooltip-user" class:is-you={isYou}>
							<span class="tooltip-dot"></span>
							<span>{entry}</span>
							{#if isYou}<span class="you-tag">{t('monitoring.you')}</span>{/if}
						</div>
					{/each}
				</div>
			{/if}
		</div>
	</div>
{:else}
	<div class="monitoring-bar disconnected">
		<span class="disconnected-text">{t('monitoring.not_connected')}</span>
	</div>
{/if}

<style>
	.monitoring-bar {
		display: flex;
		flex-direction: row;
		align-items: center;
		gap: 16px;
		height: 32px;
		padding: 0 12px;
		background-color: var(--color-bg-secondary);
		border-top: 1px solid var(--color-border);
		font-family: var(--font-sans);
		overflow: visible;
	}

	.gauge {
		display: flex;
		flex-direction: row;
		align-items: center;
		gap: 6px;
		min-width: 0;
	}

	.gauge-label {
		font-size: 11px;
		line-height: 1;
		color: var(--color-text-secondary);
		flex-shrink: 0;
	}

	.gauge-track {
		width: 48px;
		height: 6px;
		border-radius: 9999px;
		background-color: var(--color-bg-primary);
		overflow: hidden;
		flex-shrink: 0;
	}

	.gauge-fill {
		height: 100%;
		border-radius: 9999px;
		transition: width var(--duration-default) var(--ease-default),
			background-color var(--duration-default) var(--ease-default);
	}

	.gauge-value {
		font-size: 11px;
		line-height: 1;
		font-family: var(--font-mono);
		min-width: 32px;
		text-align: right;
		flex-shrink: 0;
	}

	.users {
		margin-left: auto;
		flex-shrink: 0;
		position: relative;
		cursor: default;
	}

	.users-text {
		font-size: 11px;
		line-height: 1;
		color: var(--color-text-secondary);
	}

	.users-tooltip {
		position: absolute;
		bottom: calc(100% + 8px);
		right: 0;
		width: max-content;
		padding: 8px 0;
		background-color: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
		border-radius: 8px;
		box-shadow: 0 8px 32px rgba(0, 0, 0, 0.25);
		z-index: 100;
	}

	.tooltip-user {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 5px 12px;
		font-size: 11px;
		font-family: var(--font-mono);
		color: var(--color-text-secondary);
		white-space: nowrap;
	}

	.tooltip-user.is-you {
		color: var(--color-accent);
	}

	.tooltip-dot {
		width: 5px;
		height: 5px;
		border-radius: 50%;
		background-color: var(--color-success);
		flex-shrink: 0;
	}

	.you-tag {
		font-family: var(--font-sans);
		font-size: 10px;
		color: var(--color-accent);
		opacity: 0.7;
	}

	.disconnected {
		justify-content: center;
	}

	.disconnected-text {
		font-size: 11px;
		line-height: 1;
		color: var(--color-text-secondary);
	}
</style>
