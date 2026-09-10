<script lang="ts">
	/**
	 * Brand mark for an MCP client, following the same shape as DistroIcon.
	 *
	 * Falls back to a neutral terminal glyph when Simple Icons has no mark for
	 * the client — it dropped Visual Studio Code and OpenAI over trademark
	 * policy. Borrowing an adjacent company's logo would be both wrong and
	 * actively misleading in a picker whose whole job is "find your tool".
	 *
	 * Pure-black marks are lightened, because several are `#000000` and would
	 * disappear against a dark theme.
	 */
	import { clientIcons } from '$lib/data/mcp-client-icons';

	let { slug, size = 20 }: { slug: string; size?: number } = $props();

	let icon = $derived(clientIcons[slug]);
	let fill = $derived(
		icon ? (icon.hex.toLowerCase() === '000000' ? 'currentColor' : `#${icon.hex}`) : ''
	);
</script>

{#if icon}
	<svg
		xmlns="http://www.w3.org/2000/svg"
		viewBox="0 0 24 24"
		width={size}
		height={size}
		fill={fill}
		aria-label={icon.title}
		role="img"
		class="client-icon"
	>
		<path d={icon.path} />
	</svg>
{:else}
	<!-- No brand mark available. A generic terminal reads as "a tool", which is
	     honest, rather than implying a brand we do not have. -->
	<svg
		xmlns="http://www.w3.org/2000/svg"
		viewBox="0 0 24 24"
		width={size}
		height={size}
		fill="none"
		role="img"
		aria-hidden="true"
		class="client-icon"
	>
		<rect x="2.5" y="4" width="19" height="16" rx="2.5" stroke="currentColor" stroke-width="1.6" />
		<path
			d="M7 10l2.5 2L7 14M12.5 14.5h4.5"
			stroke="currentColor"
			stroke-width="1.6"
			stroke-linecap="round"
			stroke-linejoin="round"
		/>
	</svg>
{/if}

<style>
	.client-icon {
		display: inline-flex;
		flex-shrink: 0;
		/* Several marks are pure black and vanish on a dark surface; those
		   inherit currentColor instead of their brand hex. */
		color: var(--color-text-primary);
	}
</style>
