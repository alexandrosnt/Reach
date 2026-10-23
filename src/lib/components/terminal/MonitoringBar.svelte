<script lang="ts">
	import { getStats } from '$lib/state/monitoring.svelte';
	import { t } from '$lib/state/i18n.svelte';
	import McpSessionBar from './McpSessionBar.svelte';
	import ShareBar from '$lib/components/share/ShareBar.svelte';
	import Clock from '$lib/components/layout/Clock.svelte';
	import { getActiveTab } from '$lib/state/tabs.svelte';

	interface Props {
		connectionId?: string;
		sshUser?: string;
		/** The id this session is shared under, for the MCP controls. */
		sessionId?: string;
	}

	let { connectionId, sshUser, sessionId }: Props = $props();

	let stats = $derived(connectionId ? getStats(connectionId) : undefined);

	// This bar is the window's bottom row while it shows (monitor-bar.svelte.ts),
	// so it carries what the status bar said: the connection's state, as the
	// tab's mark shows it, and the time.
	let tab = $derived(getActiveTab());
	let link = $derived(tab?.type === 'ssh' ? tab.link : undefined);
	let linkLabel = $derived(
		link === 'down'
			? t('tabs.disconnected')
			: link === 'reconnecting'
				? t('terminal.reconnecting')
				: tab?.type === 'ssh' && tab.connectionId
					? t('statusbar.connected_to', { title: tab.title })
					: t('statusbar.local')
	);

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

	function formatSpeed(bytesPerSec: number): string {
		if (bytesPerSec < 1024) return `${bytesPerSec} B/s`;
		if (bytesPerSec < 1024 * 1024) return `${(bytesPerSec / 1024).toFixed(1)} KB/s`;
		if (bytesPerSec < 1024 * 1024 * 1024) return `${(bytesPerSec / (1024 * 1024)).toFixed(1)} MB/s`;
		return `${(bytesPerSec / (1024 * 1024 * 1024)).toFixed(2)} GB/s`;
	}

	let netDown = $derived(stats ? formatSpeed(stats.netDown ?? 0) : '0 B/s');
	let netUp = $derived(stats ? formatSpeed(stats.netUp ?? 0) : '0 B/s');

	let showTooltip = $state(false);
</script>

<!-- Sized by its own width, not the window's: beside an open sidebar, at a
     larger interface size, or in a split, the pane is narrower than the
     window says. -->
<div class="monitoring-wrap">
{#snippet linkState()}
	<span
		class="link"
		class:connected={tab?.type === 'ssh' && !link}
		class:down={link === 'down'}
		class:reconnecting={link === 'reconnecting'}
		role="status"
		title={linkLabel}
		aria-label={linkLabel}
	>
		<span class="link-dot"></span>
		<!-- Said in words only when something is wrong: green needs no caption. -->
		{#if link}<span class="link-text">{linkLabel}</span>{/if}
	</span>
{/snippet}

{#if stats}
	<!-- Dimmed while the connection is down: the figures are the last ones
	     read, not the server's now. -->
	<div class="monitoring-bar" class:stale={link === 'down'}>
		{@render linkState()}
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

		<div class="net-stats">
			<span class="net-item">
				<svg width="10" height="10" viewBox="0 0 10 10" fill="none" class="net-icon down">
					<path d="M5 1v8M2 6l3 3 3-3" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>
				</svg>
				{netDown}
			</span>
			<span class="net-item">
				<svg width="10" height="10" viewBox="0 0 10 10" fill="none" class="net-icon up">
					<path d="M5 9V1M2 4l3-3 3 3" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>
				</svg>
				{netUp}
			</span>
		</div>

		<!-- Per-session MCP controls live in the gap the bar already has between
		     the network stats and the user count. They belong on this row
		     because it is the one piece of chrome already scoped to the session
		     in front of you, and they render nothing when MCP is off or this
		     session is not shared. -->
		<McpSessionBar {sessionId} />
		<ShareBar />

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
		<Clock />
	</div>
{:else}
	<!-- No stats means no SSH monitoring, not no session: a local shell has
	     nothing to monitor and is still something you might share. -->
	<div class="monitoring-bar disconnected">
		{@render linkState()}
		<span class="disconnected-text">{t('monitoring.not_connected')}</span>
		<ShareBar />
		<Clock />
	</div>
{/if}
</div>

<style>
	.monitoring-wrap {
		container-type: inline-size;
	}

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

	.net-stats {
		display: flex;
		align-items: center;
		gap: 10px;
		flex-shrink: 0;
	}

	.net-item {
		display: flex;
		align-items: center;
		gap: 3px;
		font-size: 0.625rem;
		font-variant-numeric: tabular-nums;
		color: var(--color-text-secondary);
		white-space: nowrap;
	}

	.net-icon {
		flex-shrink: 0;
	}

	.net-icon.down {
		color: var(--color-success, var(--color-success));
	}

	.net-icon.up {
		color: var(--color-accent, var(--color-accent));
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
		box-shadow: var(--shadow-elevated);
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
		gap: var(--space-3);
	}

	.disconnected-text {
		flex: 1;
		min-width: 0;
		text-align: center;
		font-size: 11px;
		line-height: 1;
		color: var(--color-text-secondary);
	}

	.link {
		display: flex;
		align-items: center;
		gap: 6px;
		flex-shrink: 0;
		font-size: 11px;
		line-height: 1;
		white-space: nowrap;
		color: var(--color-text-secondary);
	}

	.link-dot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background-color: var(--color-text-secondary);
		transition: background-color var(--duration-default) var(--ease-default);
	}

	.link.connected .link-dot {
		background-color: var(--color-success);
	}

	.link.down .link-dot {
		background-color: var(--color-danger);
	}

	.link.down .link-text {
		color: var(--color-danger);
	}

	.link.reconnecting .link-dot {
		background-color: var(--color-warning);
	}

	.monitoring-bar.stale .gauge,
	.monitoring-bar.stale .net-stats,
	.monitoring-bar.stale .users {
		opacity: 0.4;
		filter: grayscale(1);
	}

	.monitoring-bar > :global(.clock) {
		flex-shrink: 0;
	}

	/* A short screen (a phone on its side) gets the shorter bar. */
	@media (max-height: 480px) {
		.monitoring-bar {
			height: 26px;
		}
	}

	/* A narrow pane cannot hold the gauges: they shrank into one another
	   and the figures were drawn over each other (#127). Figures only, then,
	   at their own width, and the bar scrolls sideways if they still do not
	   fit. The full row needs about 640px. */
	@container (max-width: 640px) {
		.monitoring-bar {
			height: 26px;
			gap: 12px;
			overflow-x: auto;
			scrollbar-width: none;
		}

		.monitoring-bar::-webkit-scrollbar {
			display: none;
		}

		.gauge,
		.users {
			flex-shrink: 0;
		}

		.gauge-track {
			display: none;
		}

		.gauge-value {
			min-width: 0;
		}
	}
</style>
