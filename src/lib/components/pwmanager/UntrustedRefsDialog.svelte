<script lang="ts">
	/**
	 * Asks before a session may send the user's own password manager entries
	 * somewhere. A session can sit in a vault other people write to, and an
	 * entry is fetched with the user's own login: whoever last edited the
	 * session decides where it goes. So each entry the user has not approved
	 * for the session as it is comes here first.
	 *
	 * Everything shown comes from the stored session, read in one go by the
	 * backend (`UntrustedRefs`), and the entry's name is looked up in the
	 * user's manager by its id: the label stored with the session is written
	 * by whoever edited it last and is never shown here. Allowing signs exactly
	 * what was shown; if the session changed meanwhile, the backend refuses.
	 */
	import Modal from '#lib/components/shared/Modal.svelte';
	import Button from '#lib/components/shared/Button.svelte';
	import { pwmanagerList } from '#lib/ipc/pwmanager.js';
	import type { RefSlot, UntrustedRefs, ViaRoute } from '#lib/ipc/sessions.js';
	import { providerName } from '#lib/state/pwmanager.svelte.js';
	import { t } from '#lib/state/i18n.svelte.js';

	interface Props {
		asked: UntrustedRefs | null;
		busy?: boolean;
		error?: string;
		onallow: () => void;
		oncancel: () => void;
	}

	let { asked, busy = false, error, onallow, oncancel }: Props = $props();

	/** Real entry names by `provider\tentry_id`; null while still being looked up. */
	let names = $state<Map<string, string> | null>(null);

	$effect(() => {
		const current = asked;
		names = null;
		if (!current) return;
		let stale = false;
		const found = new Map<string, string>();
		const providers = [...new Set(current.refs.map((r) => r.provider))];
		void Promise.all(
			providers.map(async (provider) => {
				try {
					const entries = await pwmanagerList(provider);
					for (const e of entries) found.set(`${provider}\t${e.id}`, e.name);
				} catch {
					// Not listed: the id is shown instead.
				}
			})
		).then(() => {
			if (!stale) names = found;
		});
		return () => {
			stale = true;
		};
	});

	function entryName(r: RefSlot): string {
		if (!names) return '…';
		return names.get(`${r.provider}\t${r.entry_id}`) ?? t('pwmanager.untrusted_entry_unknown', { id: r.entry_id });
	}

	function login(user: string, host: string, port: number): string {
		return `${user ? `${user}@` : ''}${host}:${port}`;
	}

	/** Where the slot's secret goes, and what it passes through on the way. */
	function target(a: UntrustedRefs, slot: string): string {
		const jump = /^jump:(\d+):/.exec(slot);
		if (jump) {
			const n = Number(jump[1]);
			const to = a.jumps[n] ?? '?';
			return n > 0 ? t('pwmanager.untrusted_via', { target: to, route: a.jumps.slice(0, n).join(' → ') }) : to;
		}
		if (slot === 'proxy') return a.proxy ?? '?';
		const to = login(a.username, a.host, a.port);
		const route = a.via ? viaRoute(a.via) : a.jumps.length > 0 ? a.jumps.join(' → ') : (a.proxy ?? '');
		return route ? t('pwmanager.untrusted_via', { target: to, route }) : to;
	}

	/** The way through the SSH session a VNC session goes through, ending at its server. */
	function viaRoute(v: ViaRoute): string {
		const before = v.jumps.length > 0 ? v.jumps : v.proxy ? [v.proxy] : [];
		return [...before, login(v.username, v.host, v.port)].join(' → ');
	}

	function line(a: UntrustedRefs, r: RefSlot): string {
		const params = { session: a.session_name, provider: providerName(r.provider), entry: entryName(r), target: target(a, r.slot) };
		return r.slot.endsWith('passphrase') ? t('pwmanager.untrusted_line_passphrase', params) : t('pwmanager.untrusted_line', params);
	}
</script>

{#snippet routeLines(session: string, lines: string[])}
	{#if lines.length > 0}
		<div class="route">
			<p class="note">{t('pwmanager.untrusted_route_lines', { session })}</p>
			<ul>
				{#each lines as l, i (i)}
					<li><code>{l}</code></li>
				{/each}
			</ul>
		</div>
	{/if}
{/snippet}

<Modal open={!!asked} onclose={oncancel} title={t('pwmanager.untrusted_title')} maxWidth="520px" zIndex={300}>
	{#if asked}
		<div class="body">
			<ul class="lines">
				{#each asked.refs as r (r.slot)}
					<li>{line(asked, r)}</li>
				{/each}
			</ul>
			{@render routeLines(asked.session_name, asked.route_lines ?? [])}
			{#if asked.via}
				<p class="note">{t('pwmanager.untrusted_via_session', { name: asked.via.session_name, route: viaRoute(asked.via) })}</p>
				{@render routeLines(asked.via.session_name, asked.via.route_lines ?? [])}
			{/if}
			<p class="warning">{t('pwmanager.untrusted_warning')}</p>
			{#if error}
				<p class="error" role="alert">{error}</p>
			{/if}
		</div>
	{/if}
	{#snippet actions()}
		<Button variant="ghost" onclick={oncancel} disabled={busy}>{t('common.cancel')}</Button>
		<Button variant="primary" onclick={onallow} disabled={busy || !names}>{t('pwmanager.untrusted_allow')}</Button>
	{/snippet}
</Modal>

<style>
	.body {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.lines {
		margin: 0;
		padding-left: 18px;
		display: flex;
		flex-direction: column;
		gap: 8px;
		font-size: 0.875rem;
		color: var(--color-text-primary);
		overflow-wrap: anywhere;
	}

	.note {
		margin: 0;
		font-size: 0.8125rem;
		color: var(--color-text-secondary);
		overflow-wrap: anywhere;
	}

	.route ul {
		margin: 4px 0 0;
		padding-left: 18px;
		font-size: 0.8125rem;
		overflow-wrap: anywhere;
	}

	.warning {
		margin: 0;
		font-size: 0.8125rem;
		color: var(--color-warning);
	}

	.error {
		margin: 0;
		font-size: 0.8125rem;
		color: var(--color-danger);
		overflow-wrap: anywhere;
	}
</style>
