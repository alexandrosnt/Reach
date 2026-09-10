<!--
	Marks something the user has not been shown yet.

	Two shapes for two amounts of room. `dot` is for an icon in a 48px rail,
	where a word will not fit; `badge` is for a labelled row, where the word
	"NEW" says what a coloured circle only implies.

	Colour is never the only signal. The dot carries an accessible label and a
	tooltip naming the release, so it still means something to someone who
	cannot tell it from the icon behind it.
-->
<script lang="ts">
	import { t } from '$lib/state/i18n.svelte';

	interface Props {
		/** 'dot' for cramped spots, 'badge' where a word fits. */
		variant?: 'dot' | 'badge';
		/** The release this arrived in, shown in the tooltip. */
		since?: string | null;
	}

	let { variant = 'dot', since = null }: Props = $props();

	let label = $derived(since ? t('common.new_since', { version: since }) : t('common.new'));
</script>

{#if variant === 'badge'}
	<span class="badge" title={label}>{t('common.new')}</span>
{:else}
	<span class="dot" title={label} aria-label={label} role="img"></span>
{/if}

<style>
	.dot {
		position: absolute;
		top: 6px;
		right: 6px;
		width: 7px;
		height: 7px;
		border-radius: 50%;
		background: var(--color-accent);
		/* A ring in the surface colour keeps the dot legible over whatever icon
		   it sits on, without needing to know what that icon looks like.

		   It has to be the actual surface, though. This was --color-bg-elevated,
		   which is #1c1c1e against a #141414 rail in dark — near enough to pass
		   a screenshot — and #ffffff against #f5f5f7 in light, which is a white
		   halo. Both call sites sit on --color-bg-secondary; anywhere else can
		   override --new-dot-ring rather than guess. */
		box-shadow: 0 0 0 2px var(--new-dot-ring, var(--color-bg-secondary));
		pointer-events: none;
	}

	.badge {
		display: inline-flex;
		align-items: center;
		padding: 1px 5px;
		font-size: 9px;
		font-weight: 700;
		letter-spacing: 0.04em;
		text-transform: uppercase;
		color: var(--color-accent);
		background: color-mix(in srgb, var(--color-accent) 15%, transparent);
		border: 1px solid color-mix(in srgb, var(--color-accent) 45%, transparent);
		border-radius: 999px;
		white-space: nowrap;
	}
</style>
