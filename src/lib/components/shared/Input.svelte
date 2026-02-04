<script lang="ts">
	interface Props {
		label?: string;
		value?: string;
		type?: 'text' | 'password' | 'number';
		placeholder?: string;
		disabled?: boolean;
		oninput?: (e: Event & { currentTarget: HTMLInputElement }) => void;
	}

	let {
		label = '',
		value = $bindable(''),
		type = 'text',
		placeholder = '',
		disabled = false,
		oninput
	}: Props = $props();

	let focused = $state(false);
	let inputId = $state(`input-${Math.random().toString(36).slice(2, 9)}`);

	let floated = $derived(focused || value.length > 0);
</script>

<div class="input-wrapper" class:disabled>
	{#if label}
		<label class="input-label" class:floated for={inputId}>
			{label}
		</label>
	{/if}

	<input
		id={inputId}
		class="input-field"
		class:has-label={!!label}
		{type}
		placeholder={label && !focused ? '' : placeholder}
		{disabled}
		bind:value
		{oninput}
		onfocus={() => (focused = true)}
		onblur={() => (focused = false)}
	/>

	<div class="input-border" class:focused></div>
</div>

<style>
	.input-wrapper {
		position: relative;
		width: 100%;
	}

	.input-wrapper.disabled {
		opacity: 0.4;
		pointer-events: none;
	}

	.input-label {
		position: absolute;
		left: 12px;
		top: 50%;
		transform: translateY(-50%);
		font-size: 0.875rem;
		color: var(--color-text-secondary);
		pointer-events: none;
		transition:
			top var(--duration-default) var(--ease-default),
			transform var(--duration-default) var(--ease-default),
			font-size var(--duration-default) var(--ease-default),
			color var(--duration-default) var(--ease-default);
	}

	.input-label.floated {
		top: 8px;
		transform: translateY(0);
		font-size: 0.625rem;
		color: var(--color-accent);
	}

	.input-field {
		width: 100%;
		padding: 12px;
		font-family: var(--font-sans);
		font-size: 0.875rem;
		color: var(--color-text-primary);
		background-color: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		outline: none;
		transition:
			border-color var(--duration-default) var(--ease-default),
			box-shadow var(--duration-default) var(--ease-default);
		box-sizing: border-box;
	}

	.input-field.has-label {
		padding-top: 20px;
		padding-bottom: 6px;
	}

	.input-field::placeholder {
		color: var(--color-text-secondary);
		opacity: 0.5;
	}

	.input-border {
		position: absolute;
		inset: 0;
		border-radius: var(--radius-btn);
		pointer-events: none;
		border: 2px solid transparent;
		transition: border-color var(--duration-default) var(--ease-default);
	}

	.input-border.focused {
		border-color: var(--color-accent);
	}
</style>
