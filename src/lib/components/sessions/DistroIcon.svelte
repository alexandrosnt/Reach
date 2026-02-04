<script lang="ts">
	import { getDistroIcon } from '$lib/data/distro-icons';

	let { osId, size = 16 }: { osId: string | undefined | null; size?: number } = $props();

	let icon = $derived(getDistroIcon(osId));
	let fill = $derived(icon && icon.hex.toLowerCase() === '000000' ? '#f5f5f7' : icon ? `#${icon.hex}` : '');
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
		class="distro-icon"
	>
		<path d={icon.path} />
	</svg>
{/if}

<style>
	.distro-icon {
		display: inline-flex;
		align-items: center;
		flex-shrink: 0;
		opacity: 0.85;
		transition: opacity 150ms ease;
	}

	.distro-icon:hover {
		opacity: 1;
	}
</style>
