<script lang="ts">
	/**
	 * Optional wizard step: install plugins from the marketplace.
	 *
	 * The registry is empty at the time of writing, and an empty registry is
	 * indistinguishable from an unreachable one to a first-time user — both
	 * would read as "the app is broken" on the second screen they ever see. So
	 * neither is an error here: both land on the same calm empty state, and the
	 * step is skippable in one click. The wizard never blocks on the network.
	 */
	import { onMount } from 'svelte';
	import Button from '$lib/components/shared/Button.svelte';
	import { t } from '$lib/state/i18n.svelte';
	import { addToast } from '$lib/state/toasts.svelte';
	import { marketplaceFetch, marketplaceInstall, type MarketplaceEntry } from '$lib/ipc/marketplace';
	import { pluginList } from '$lib/ipc/plugin';
	import { setPlugins } from '$lib/state/plugin.svelte';

	interface Props {
		onNext: () => void;
		onBack: () => void;
	}

	let { onNext, onBack }: Props = $props();

	let entries = $state<MarketplaceEntry[]>([]);
	let loading = $state(true);
	let installingId = $state<string | null>(null);
	let installed = $state(new Set<string>());

	onMount(async () => {
		try {
			entries = await marketplaceFetch();
		} catch {
			// Deliberately swallowed — see the note at the top. The empty state
			// covers "registry unreachable" as well as "registry empty".
			entries = [];
		} finally {
			loading = false;
		}
	});

	async function install(entry: MarketplaceEntry): Promise<void> {
		installingId = entry.id;
		try {
			await marketplaceInstall(entry);
			installed = new Set([...installed, entry.id]);
			addToast(t('marketplace.installed', { name: entry.name }), 'info');
			setPlugins(await pluginList());
		} catch (err) {
			addToast(String(err), 'error');
		} finally {
			installingId = null;
		}
	}
</script>

<div class="step">
	<div class="step-header">
		<h2 class="step-title">{t('setup.plugins_title')}</h2>
		<p class="step-subtitle">{t('setup.plugins_subtitle')}</p>
	</div>

	<div class="panel">
		{#if loading}
			<div class="empty">
				<p class="empty-text">{t('marketplace.loading')}</p>
			</div>
		{:else if entries.length === 0}
			<div class="empty">
				<svg class="empty-icon" width="32" height="32" viewBox="0 0 24 24" fill="none" aria-hidden="true">
					<path d="M4 7l8-4 8 4v10l-8 4-8-4V7z" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round" />
					<path d="M4 7l8 4 8-4M12 11v10" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round" />
				</svg>
				<p class="empty-text">{t('setup.plugins_empty')}</p>
				<p class="empty-hint">{t('setup.plugins_empty_hint')}</p>
			</div>
		{:else}
			<ul class="list">
				{#each entries as entry (entry.id)}
					<li class="entry">
						<div class="entry-text">
							<span class="entry-name">{entry.name}</span>
							<span class="entry-desc">{entry.description}</span>
						</div>
						{#if installed.has(entry.id)}
							<span class="entry-installed">{t('marketplace.installed_label')}</span>
						{:else}
							<Button
								variant="secondary"
								size="sm"
								disabled={installingId === entry.id}
								onclick={() => install(entry)}
							>
								{installingId === entry.id ? t('marketplace.installing') : t('marketplace.install')}
							</Button>
						{/if}
					</li>
				{/each}
			</ul>
		{/if}
	</div>

	<div class="step-actions">
		<Button variant="ghost" size="md" onclick={onBack}>{t('setup.back')}</Button>
		<Button variant="primary" size="md" onclick={onNext}>
			{entries.length === 0 ? t('setup.skip') : t('setup.next')}
		</Button>
	</div>
</div>

<style>
	.step {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--space-5);
		width: 100%;
	}

	.step-header {
		text-align: center;
	}

	.step-title {
		margin: 0;
		font-size: var(--text-lg);
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.step-subtitle {
		margin: 6px 0 0;
		font-size: var(--text-sm);
		color: var(--color-text-secondary);
	}

	.panel {
		width: 100%;
		/* A fixed floor keeps the wizard from resizing between the loading,
		   empty and populated states, which otherwise jumps the buttons. */
		min-height: 168px;
		display: flex;
		background: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-card);
		overflow: hidden;
	}

	.empty {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: var(--space-2);
		flex: 1;
		padding: var(--space-5) var(--space-4);
		text-align: center;
	}

	.empty-icon {
		color: var(--color-text-tertiary);
		opacity: 0.5;
	}

	.empty-text {
		margin: 0;
		font-size: var(--text-sm);
		color: var(--color-text-secondary);
	}

	.empty-hint {
		margin: 0;
		font-size: var(--text-xs);
		color: var(--color-text-tertiary);
		max-width: 34ch;
	}

	.list {
		flex: 1;
		max-height: 240px;
		overflow-y: auto;
		margin: 0;
		padding: var(--space-2);
		list-style: none;
	}

	.entry {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		padding: var(--space-2);
		border-radius: var(--radius-sm);
	}

	.entry + .entry {
		border-top: 1px solid var(--color-border);
	}

	.entry-text {
		display: flex;
		flex-direction: column;
		gap: 2px;
		flex: 1;
		min-width: 0;
	}

	.entry-name {
		font-size: var(--text-sm);
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.entry-desc {
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.entry-installed {
		flex-shrink: 0;
		font-size: var(--text-2xs);
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.03em;
		color: var(--color-success);
	}

	.step-actions {
		display: flex;
		justify-content: center;
		gap: var(--space-3);
		width: 100%;
		/* The step body scrolls (see WelcomeScreen .step-content), so the
		   primary action pins to the bottom of that scroller instead of
		   disappearing under the fold on a short window. */
		position: sticky;
		bottom: 0;
		padding-top: var(--space-3);
		background: linear-gradient(
			to bottom,
			transparent,
			var(--color-bg-secondary) 40%
		);
	}
</style>
