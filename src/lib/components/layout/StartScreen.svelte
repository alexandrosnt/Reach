<script lang="ts">
	/**
	 * What the terminal page shows with no tab open: a way to connect right
	 * here (an address, a session used lately, a local shell), not a single
	 * button and a sentence in the middle of the window.
	 */
	import { createTab } from '$lib/state/tabs.svelte';
	import { requestSessionAction, openPalette } from '$lib/state/palette.svelte';
	import { recentSessionIds } from '$lib/state/recents.svelte';
	import { shortcutLabel } from '$lib/state/shortcuts.svelte';
	import { sessionList, type SessionConfig } from '$lib/ipc/sessions';
	import { parseAddress } from '$lib/utils/address';
	import { isMobile } from '$lib/platform';
	import { t } from '$lib/state/i18n.svelte';

	let address = $state('');
	let sessions = $state<SessionConfig[]>([]);

	let typed = $derived(parseAddress(address));

	$effect(() => {
		void sessionList()
			.then((s) => (sessions = s))
			.catch(() => (sessions = []));
	});

	// Sessions used lately on this device; before any, the first few saved.
	let shown = $derived.by(() => {
		const byId = new Map(sessions.map((s) => [s.id, s]));
		const recent = recentSessionIds()
			.map((id) => byId.get(id))
			.filter((s): s is SessionConfig => !!s);
		if (recent.length) return { recent: true, list: recent.slice(0, 5) };
		return { recent: false, list: [...sessions].sort((a, b) => a.name.localeCompare(b.name)).slice(0, 5) };
	});

	function connectTyped(): void {
		if (!typed) return;
		requestSessionAction({ kind: 'quick-connect', prefill: typed });
		address = '';
	}

	const localShell = !isMobile();
</script>

<div class="start">
	<div class="column">
		<header>
			<svg width="28" height="28" viewBox="0 0 24 24" fill="none" class="mark" aria-hidden="true">
				<path d="M4 17l6-5-6-5M12 19h8" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" />
			</svg>
			<h1>{t('start.title')}</h1>
			<p>{t('start.subtitle')}</p>
		</header>

		<form class="connect" onsubmit={(e) => { e.preventDefault(); connectTyped(); }}>
			<input
				bind:value={address}
				placeholder={t('start.address_placeholder')}
				aria-label={t('start.address_placeholder')}
				autocomplete="off"
				autocapitalize="off"
				spellcheck="false"
			/>
			<button type="submit" disabled={!typed}>{t('start.connect')}</button>
		</form>

		{#if shown.list.length}
			<section>
				<h2>{shown.recent ? t('start.recent') : t('start.saved')}</h2>
				<ul>
					{#each shown.list as s (s.id)}
						<li>
							<button class="session" onclick={() => requestSessionAction({ kind: 'connect', sessionId: s.id })}>
								<span class="name">{s.name}</span>
								<span class="addr">{s.username ? `${s.username}@` : ''}{s.host}</span>
								<svg width="14" height="14" viewBox="0 0 24 24" fill="none" class="go" aria-hidden="true">
									<path d="M9 6l6 6-6 6" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
								</svg>
							</button>
						</li>
					{/each}
				</ul>
			</section>
		{/if}

		<div class="actions">
			{#if localShell}
				<button class="action" onclick={() => createTab('local')}>
					<span>{t('start.local')}</span>
					<kbd>{shortcutLabel('T', { ctrl: true })}</kbd>
				</button>
			{/if}
			<button class="action" onclick={() => requestSessionAction({ kind: 'new-session' })}>
				<span>{t('session.new_session')}</span>
				<kbd>{shortcutLabel('N', { ctrl: true })}</kbd>
			</button>
			<button class="action" onclick={openPalette}>
				<span>{t('palette.title')}</span>
				<kbd>{shortcutLabel('P', { ctrl: true, shift: true })}</kbd>
			</button>
		</div>
	</div>
</div>

<style>
	.start {
		flex: 1;
		display: flex;
		justify-content: center;
		overflow-y: auto;
		padding: clamp(var(--space-4), 8dvh, 96px) var(--space-4) var(--space-5);
	}

	.column {
		width: min(520px, 100%);
		display: flex;
		flex-direction: column;
		gap: var(--space-5);
	}

	header {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}

	.mark {
		color: var(--color-accent);
		margin-bottom: var(--space-1);
	}

	h1 {
		margin: 0;
		font-size: var(--text-xl);
		font-weight: 600;
		letter-spacing: -0.01em;
		color: var(--color-text-primary);
	}

	header p {
		margin: 0;
		font-size: var(--text-sm);
		line-height: 1.5;
		color: var(--color-text-secondary);
	}

	.connect {
		display: flex;
		gap: var(--space-2);
	}

	.connect input {
		flex: 1;
		min-width: 0;
		height: 40px;
		padding: 0 var(--space-3);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		background: var(--color-bg-secondary);
		color: var(--color-text-primary);
		font-family: var(--font-mono);
		font-size: var(--text-sm);
		outline: none;
		transition: border-color var(--duration-default) var(--ease-default);
	}

	.connect input:focus {
		border-color: var(--color-accent);
	}

	.connect input::placeholder {
		color: var(--color-text-tertiary);
		font-family: var(--font-sans);
	}

	.connect button {
		flex-shrink: 0;
		height: 40px;
		padding: 0 var(--space-4);
		border: none;
		border-radius: var(--radius-btn);
		background: var(--color-accent);
		color: #fff;
		font-family: var(--font-sans);
		font-size: var(--text-sm);
		font-weight: 500;
		cursor: pointer;
		transition: opacity var(--duration-default) var(--ease-default);
	}

	.connect button:disabled {
		opacity: 0.4;
		cursor: default;
	}

	h2 {
		margin: 0 0 var(--space-2);
		font-size: var(--text-2xs);
		font-weight: 600;
		letter-spacing: 0.04em;
		text-transform: uppercase;
		color: var(--color-text-tertiary);
	}

	ul {
		margin: 0;
		padding: 0;
		list-style: none;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-card);
		overflow: hidden;
	}

	li + li {
		border-top: 1px solid var(--color-border);
	}

	.session {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		width: 100%;
		min-height: 44px;
		padding: 0 var(--space-3) 0 var(--space-4);
		border: none;
		background: transparent;
		color: var(--color-text-primary);
		font-family: var(--font-sans);
		font-size: var(--text-sm);
		text-align: left;
		cursor: pointer;
		transition: background-color var(--duration-default) var(--ease-default);
	}

	.session:hover,
	.session:focus-visible {
		background: var(--color-surface-hover);
		outline: none;
	}

	.name {
		flex-shrink: 0;
		max-width: 45%;
		font-weight: 500;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.addr {
		flex: 1;
		min-width: 0;
		color: var(--color-text-secondary);
		font-family: var(--font-mono);
		font-size: var(--text-xs);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.go {
		flex-shrink: 0;
		color: var(--color-text-tertiary);
		opacity: 0;
		transition: opacity var(--duration-default) var(--ease-default);
	}

	.session:hover .go,
	.session:focus-visible .go {
		opacity: 1;
	}

	.actions {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
	}

	.action {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		height: 32px;
		padding: 0 var(--space-3);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		background: transparent;
		color: var(--color-text-secondary);
		font-family: var(--font-sans);
		font-size: var(--text-xs);
		cursor: pointer;
		transition: color var(--duration-default) var(--ease-default), border-color var(--duration-default) var(--ease-default);
	}

	.action:hover {
		color: var(--color-text-primary);
		border-color: var(--color-surface-active);
	}

	kbd {
		font-family: var(--font-mono);
		font-size: var(--text-2xs);
		color: var(--color-text-tertiary);
	}

	/* A phone: no keys to learn, and room for a thumb. */
	@media (pointer: coarse) {
		kbd {
			display: none;
		}

		.action {
			height: 40px;
		}

		.go {
			opacity: 1;
		}
	}
</style>
