<!--
	The Share control in the bottom bar.

	Renders nothing at all while sharing is turned off — not a disabled
	button, nothing. Someone who never wants this never sees it. When on and
	idle it is one button; during a share it is the status, and the way back
	to the dialog.
-->
<script lang="ts">
	import ShareDialog from './ShareDialog.svelte';
	import { t } from '$lib/state/i18n.svelte';
	import * as share from '$lib/state/share.svelte';

	let open = $state(false);
	let session = $derived(share.getSession());
	let phase = $derived(session?.phase ?? 'idle');

	let label = $derived.by(() => {
		if (!session) return t('sharing.share');
		switch (phase) {
			case 'preparing':
				return t('sharing.preparing_short');
			case 'exchanging':
				return t('sharing.exchanging_short');
			case 'connecting':
				return t('sharing.connecting_short');
			case 'connected':
				return session.remote?.title || t('sharing.connected_short');
			case 'failed':
				return t('sharing.failed_short');
			case 'ended':
				return t('sharing.ended');
			default:
				return t('sharing.share');
		}
	});
</script>

{#if share.isEnabled()}
	<button
		class="share-chip"
		class:active={!!session && phase !== 'idle'}
		class:live={phase === 'connected'}
		class:bad={phase === 'failed'}
		onclick={() => (open = true)}
		title={t('sharing.title')}
	>
		<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
			<circle cx="18" cy="5" r="3" /><circle cx="6" cy="12" r="3" /><circle cx="18" cy="19" r="3" />
			<path d="M8.6 13.5l6.8 4M15.4 6.5l-6.8 4" />
		</svg>
		<span>{label}</span>
		{#if phase === 'connected' && session}
			<span class="mode">{session.localMode === 'read-write' ? 'RW' : 'RO'}</span>
		{/if}
	</button>

	<ShareDialog {open} onclose={() => (open = false)} />
{/if}

<style>
	.share-chip {
		display: inline-flex;
		align-items: center;
		gap: 5px;
		padding: 2px 8px;
		font-family: inherit;
		font-size: var(--text-2xs);
		color: var(--color-text-secondary);
		background: transparent;
		border: 1px solid var(--color-border);
		border-radius: 999px;
		cursor: pointer;
		white-space: nowrap;
		max-width: 220px;
	}

	.share-chip span:first-of-type {
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.share-chip:hover {
		color: var(--color-text-primary);
	}

	.share-chip.active {
		color: var(--color-text-primary);
		border-color: color-mix(in srgb, var(--color-accent) 50%, var(--color-border));
	}

	.share-chip.live {
		color: var(--color-success);
		border-color: color-mix(in srgb, var(--color-success) 50%, var(--color-border));
	}

	.share-chip.bad {
		color: var(--color-danger);
		border-color: var(--color-danger);
	}

	.mode {
		padding: 0 5px;
		font-weight: 700;
		letter-spacing: 0.04em;
		border-left: 1px solid currentColor;
		opacity: 0.8;
	}
</style>
