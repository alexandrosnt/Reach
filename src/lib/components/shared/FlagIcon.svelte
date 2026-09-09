<script lang="ts">
	/**
	 * A flag for a locale code.
	 *
	 * Inline SVG rather than emoji: Windows ships no colour flag glyphs, so
	 * 🇪🇸 renders as the letters "ES" in a box there — which is exactly the
	 * platform most Reach users are on. Drawn at a 3:2 ratio and scaled by the
	 * `size` prop so callers can put the same flag in a 20px dropdown row and a
	 * 48px card without a second asset.
	 *
	 * A code with no flag here renders nothing rather than a placeholder; the
	 * language name beside it already carries the meaning.
	 */
	interface Props {
		code: string;
		/** Width in px. Height follows the 3:2 ratio. */
		size?: number;
	}

	let { code, size = 24 }: Props = $props();
</script>

<svg
	class="flag"
	style="width: {size}px; height: {(size * 2) / 3}px"
	viewBox="0 0 60 40"
	fill="none"
	xmlns="http://www.w3.org/2000/svg"
	role="presentation"
	aria-hidden="true"
>
	{#if code === 'en'}
		<!-- Union Jack -->
		<rect width="60" height="40" fill="#012169" />
		<path d="M0 0L60 40M60 0L0 40" stroke="#fff" stroke-width="6.5" />
		<path d="M0 0L60 40M60 0L0 40" stroke="#C8102E" stroke-width="4" />
		<path d="M30 0V40M0 20H60" stroke="#fff" stroke-width="10" />
		<path d="M30 0V40M0 20H60" stroke="#C8102E" stroke-width="6" />
	{:else if code === 'de'}
		<!-- Germany -->
		<rect width="60" height="13.33" fill="#000" />
		<rect y="13.33" width="60" height="13.34" fill="#DD0000" />
		<rect y="26.67" width="60" height="13.33" fill="#FFCC00" />
	{:else if code === 'es'}
		<!-- Spain: 1:2:1 bands, the coat of arms reduced to its gold suggestion
		     at this size — the real escutcheon is unreadable below ~64px wide. -->
		<rect width="60" height="10" fill="#AA151B" />
		<rect y="10" width="60" height="20" fill="#F1BF00" />
		<rect y="30" width="60" height="10" fill="#AA151B" />
		<rect x="12" y="16" width="6" height="8" rx="1" fill="#AD1519" opacity="0.85" />
		<rect x="13.5" y="17.5" width="3" height="5" rx="0.5" fill="#F1BF00" opacity="0.9" />
	{:else if code === 'fr'}
		<!-- France -->
		<rect width="20" height="40" fill="#002395" />
		<rect x="20" width="20" height="40" fill="#fff" />
		<rect x="40" width="20" height="40" fill="#ED2939" />
	{:else if code === 'el'}
		<!-- Greece -->
		<rect width="60" height="40" fill="#0D5EAF" />
		<rect y="4.44" width="60" height="4.44" fill="#fff" />
		<rect y="13.33" width="60" height="4.44" fill="#fff" />
		<rect y="22.22" width="60" height="4.44" fill="#fff" />
		<rect y="31.11" width="60" height="4.44" fill="#fff" />
		<rect width="22.22" height="17.78" fill="#0D5EAF" />
		<rect x="8.89" y="0" width="4.44" height="17.78" fill="#fff" />
		<rect x="0" y="6.67" width="22.22" height="4.44" fill="#fff" />
	{:else if code === 'it'}
		<!-- Italy -->
		<rect width="20" height="40" fill="#009246" />
		<rect x="20" width="20" height="40" fill="#fff" />
		<rect x="40" width="20" height="40" fill="#CE2B37" />
	{:else if code === 'bg'}
		<!-- Bulgaria -->
		<rect width="60" height="13.33" fill="#fff" />
		<rect y="13.33" width="60" height="13.34" fill="#00966E" />
		<rect y="26.67" width="60" height="13.33" fill="#D62612" />
	{:else if code === 'ru'}
		<!-- Russia -->
		<rect width="60" height="13.33" fill="#fff" />
		<rect y="13.33" width="60" height="13.34" fill="#0039A6" />
		<rect y="26.67" width="60" height="13.33" fill="#D52B1E" />
	{:else if code === 'zh'}
		<!-- China -->
		<rect width="60" height="40" fill="#EE1C25" />
		<!-- From https://flagpedia.net/the-people-s-republic-of-china, scale(0.0666667) because original image is 900x600, 60 / 900 = 0.0666667 -->
		<g transform="scale(0.0666667)">
			<path id="star-{code}" fill="#FF0" d="m0-30 17.634 54.27-46.166-33.54h57.064l-46.166 33.54Z" />
			<use href="#star-{code}" transform="matrix(3 0 0 3 150 150)" />
			<use href="#star-{code}" transform="rotate(23.036 2.784 766.082)" />
			<use href="#star-{code}" transform="rotate(45.87 38.201 485.396)" />
			<use href="#star-{code}" transform="rotate(69.945 29.892 362.328)" />
			<use href="#star-{code}" transform="rotate(20.66 -590.66 957.955)" />
		</g>
	{/if}
</svg>

<style>
	.flag {
		flex-shrink: 0;
		border-radius: 3px;
		/* The white bands in several flags vanish against a light surface. */
		box-shadow: 0 0 0 1px var(--color-border);
	}
</style>
