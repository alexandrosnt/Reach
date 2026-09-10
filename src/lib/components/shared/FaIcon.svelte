<!--
	A Font Awesome icon, drawn from the icon's path data rather than a font or
	a stylesheet: nothing to load, nothing to flash, tree-shaken to the icons
	actually imported. Pass any definition from @fortawesome/free-solid-svg-icons.
-->
<script lang="ts">
	import type { IconDefinition } from '@fortawesome/free-solid-svg-icons';

	interface Props {
		icon: IconDefinition;
		size?: number;
	}

	let { icon, size = 14 }: Props = $props();

	let width = $derived(icon.icon[0]);
	let height = $derived(icon.icon[1]);
	// Solid icons carry one path; duotone would carry two. Draw whatever is there.
	let paths = $derived(Array.isArray(icon.icon[4]) ? icon.icon[4] : [icon.icon[4]]);
</script>

<svg
	width={size}
	height={size}
	viewBox="0 0 {width} {height}"
	fill="currentColor"
	aria-hidden="true"
	focusable="false"
>
	{#each paths as d, i (i)}
		<path {d} />
	{/each}
</svg>
