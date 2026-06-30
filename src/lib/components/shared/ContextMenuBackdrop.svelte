<script lang="ts">
	// Transparent full-viewport layer rendered behind an open context menu (the
	// menu sits at z-index 1000, this at 999). A click anywhere outside the menu
	// lands here instead of on the content below, so it dismisses the menu and is
	// fully consumed — it never activates whatever was under the cursor.
	let { onclose }: { onclose: () => void } = $props();
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="context-backdrop"
	aria-hidden="true"
	onclick={(e) => {
		e.stopPropagation();
		onclose();
	}}
	oncontextmenu={(e) => {
		e.preventDefault();
		e.stopPropagation();
		onclose();
	}}
></div>

<style>
	.context-backdrop {
		position: fixed;
		inset: 0;
		z-index: 999;
		background: transparent;
	}
</style>
