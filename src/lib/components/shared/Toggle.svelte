<script lang="ts">
	interface Props {
		checked?: boolean;
		label?: string;
		disabled?: boolean;
		onchange?: (checked: boolean) => void;
	}

	let {
		checked = $bindable(false),
		label = '',
		disabled = false,
		onchange
	}: Props = $props();

	function toggle() {
		if (disabled) return;
		checked = !checked;
		onchange?.(checked);
	}

	function onKeydown(e: KeyboardEvent) {
		if (e.key === ' ' || e.key === 'Enter') {
			e.preventDefault();
			toggle();
		}
	}
</script>

<div class="toggle-wrapper" class:disabled>
	<button
		class="toggle-track"
		class:active={checked}
		role="switch"
		aria-checked={checked}
		aria-label={label || 'Toggle'}
		{disabled}
		onclick={toggle}
		onkeydown={onKeydown}
	>
		<span class="toggle-thumb" class:active={checked}></span>
	</button>

	{#if label}
		<span class="toggle-label" role="none" onclick={toggle}>{label}</span>
	{/if}
</div>

<style>
	.toggle-wrapper {
		display: inline-flex;
		align-items: center;
		gap: 10px;
	}

	.toggle-wrapper.disabled {
		opacity: 0.4;
		pointer-events: none;
	}

	.toggle-track {
		position: relative;
		width: 44px;
		height: 26px;
		border-radius: 13px;
		border: none;
		padding: 0;
		cursor: pointer;
		background-color: rgba(255, 255, 255, 0.12);
		transition: background-color 200ms var(--ease-default);
		flex-shrink: 0;
	}

	.toggle-track.active {
		background-color: var(--color-accent);
	}

	.toggle-thumb {
		position: absolute;
		top: 3px;
		left: 3px;
		width: 20px;
		height: 20px;
		border-radius: 50%;
		background-color: #fff;
		box-shadow: 0 1px 3px rgba(0, 0, 0, 0.3);
		transition: transform 200ms var(--ease-default);
	}

	.toggle-thumb.active {
		transform: translateX(18px);
	}

	.toggle-label {
		font-size: 0.875rem;
		color: var(--color-text-primary);
		cursor: pointer;
		user-select: none;
	}
</style>
