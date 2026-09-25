<script lang="ts">
	import type { Engine } from '$lib/ipc/db';
	import { engineInfo } from '$lib/db/engines';

	interface Props {
		engine: Engine;
		size?: number;
	}

	let { engine, size = 18 }: Props = $props();
	let info = $derived(engineInfo(engine));
</script>

<span
	class="badge"
	style:--c={info.color}
	style:width="{size}px"
	style:height="{size}px"
	style:font-size="{Math.round(size * 0.46)}px"
	title={info.name}
	aria-label={info.name}
>
	{info.short}
</span>

<style>
	.badge {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
		border-radius: 4px;
		background: color-mix(in srgb, var(--c) 22%, transparent);
		color: color-mix(in srgb, var(--c) 70%, white);
		border: 1px solid color-mix(in srgb, var(--c) 45%, transparent);
		font-weight: 700;
		letter-spacing: -0.02em;
		line-height: 1;
	}
</style>
