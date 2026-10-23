<script lang="ts">
	/** The time at the end of the window's bottom row: the status bar's, or
	 *  the session monitor bar's when that one is the bottom row. Hours and
	 *  minutes: a seconds count ticking in the corner was movement beside the
	 *  terminal all day, for nothing anyone reads. */
	function formatTime(): string {
		return new Date().toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' });
	}

	let time = $state(formatTime());

	$effect(() => {
		const interval = setInterval(() => (time = formatTime()), 1000);
		return () => clearInterval(interval);
	});
</script>

<span class="clock">{time}</span>

<style>
	.clock {
		white-space: nowrap;
		font-family: var(--font-mono);
		font-size: 0.625rem;
		color: var(--color-text-secondary);
	}
</style>
