<script lang="ts">
	/**
	 * A theme rendered in its own colours.
	 *
	 * You choose a theme by looking at it, so the card paints itself with the
	 * theme's tokens rather than the app's current ones: a title bar, a line of
	 * body text, an accent dot and two terminal colours — enough to tell two
	 * dark themes apart, which a swatch of three flat rectangles cannot.
	 *
	 * `theme` is null for "System", which has no colours of its own; that card
	 * shows a split of the two built-ins instead.
	 */
	import type { Theme } from '$lib/state/theme.svelte';
	import type { ThemeEntry } from '$lib/ipc/theme';

	interface Props {
		theme: Theme | null;
		/** The two built-ins, used to draw the split preview for "System". */
		dark: Theme;
		light: Theme;
		/** Set for a registry theme that is not installed yet. Its colours live
		 *  in a file we have not downloaded, so all that can honestly be shown
		 *  is whether it is dark or light, plus that it needs fetching. */
		entry?: ThemeEntry | null;
	}

	let { theme, dark, light, entry = null }: Props = $props();
</script>

{#if theme}
	<div
		class="preview"
		style="background: {theme.colors['bg-primary']}; border-color: {theme.colors.border}"
	>
		<div class="bar" style="background: {theme.colors['bg-secondary']}">
			<span class="dot" style="background: {theme.colors.accent}"></span>
			<span class="rule" style="background: {theme.colors['text-tertiary']}"></span>
		</div>
		<div class="body">
			<span class="line" style="background: {theme.colors['text-primary']}; width: 70%"></span>
			<span class="line" style="background: {theme.colors['text-secondary']}; width: 45%"></span>
			<div class="ansi">
				<span style="background: {theme.terminal.green}"></span>
				<span style="background: {theme.terminal.yellow}"></span>
				<span style="background: {theme.terminal.blue}"></span>
				<span style="background: {theme.terminal.magenta}"></span>
			</div>
		</div>
	</div>
{:else if entry}
	<!-- Not installed: honest placeholder in the right appearance, with a
	     download glyph. Inventing colours here would preview a lie. -->
	<div
		class="preview placeholder"
		style="background: {(entry.appearance === 'light' ? light : dark).colors['bg-primary']};
		       border-color: {(entry.appearance === 'light' ? light : dark).colors.border};
		       color: {(entry.appearance === 'light' ? light : dark).colors['text-secondary']}"
	>
		<svg width="18" height="18" viewBox="0 0 24 24" fill="none" aria-hidden="true">
			<path d="M12 3v12m0 0l-4-4m4 4l4-4" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" />
			<path d="M4 17v2a2 2 0 002 2h12a2 2 0 002-2v-2" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
		</svg>
	</div>
{:else}
	<!-- System: half of each built-in, split down the middle. -->
	<div class="preview split" style="border-color: {dark.colors.border}">
		<div class="half" style="background: {light.colors['bg-primary']}">
			<span class="line" style="background: {light.colors['text-primary']}; width: 60%"></span>
			<span class="line" style="background: {light.colors['text-secondary']}; width: 40%"></span>
		</div>
		<div class="half" style="background: {dark.colors['bg-primary']}">
			<span class="line" style="background: {dark.colors['text-primary']}; width: 60%"></span>
			<span class="line" style="background: {dark.colors['text-secondary']}; width: 40%"></span>
		</div>
	</div>
{/if}

<style>
	.preview {
		display: flex;
		flex-direction: column;
		width: 100%;
		height: 62px;
		border: 1px solid;
		border-radius: var(--radius-sm);
		overflow: hidden;
	}

	.bar {
		display: flex;
		align-items: center;
		gap: 4px;
		height: 13px;
		padding: 0 5px;
		flex-shrink: 0;
	}

	.dot {
		width: 5px;
		height: 5px;
		border-radius: 50%;
		flex-shrink: 0;
	}

	.rule {
		height: 3px;
		width: 22px;
		border-radius: 2px;
		opacity: 0.6;
	}

	.body {
		display: flex;
		flex-direction: column;
		justify-content: center;
		gap: 4px;
		flex: 1;
		padding: 0 6px;
	}

	.line {
		height: 3px;
		border-radius: 2px;
		opacity: 0.85;
	}

	.ansi {
		display: flex;
		gap: 3px;
		margin-top: 2px;
	}

	.ansi span {
		width: 7px;
		height: 4px;
		border-radius: 1px;
	}

	.placeholder {
		align-items: center;
		justify-content: center;
	}

	.split {
		flex-direction: row;
	}

	.half {
		display: flex;
		flex-direction: column;
		justify-content: center;
		gap: 4px;
		flex: 1;
		padding: 0 6px;
	}
</style>
