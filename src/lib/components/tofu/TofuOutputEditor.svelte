<script lang="ts">
	import type { TofuOutput } from '$lib/ipc/tofu';
	import { t } from '$lib/state/i18n.svelte';
	import Button from '$lib/components/shared/Button.svelte';
	import Modal from '$lib/components/shared/Modal.svelte';

	interface Props {
		open: boolean;
		output: TofuOutput | null;
		existingNames: string[];
		onclose: () => void;
		onsave: (output: TofuOutput) => void;
	}

	let { open, output, existingNames, onclose, onsave }: Props = $props();

	let name = $state('');
	let value = $state('');
	let description = $state('');
	let sensitive = $state(false);

	let isEditing = $derived(output !== null);
	let title = $derived(isEditing ? t('tofu.edit_output') : t('tofu.add_output'));

	$effect(() => {
		if (output) {
			name = output.name;
			value = output.value;
			description = output.description;
			sensitive = output.sensitive;
		} else {
			name = '';
			value = '';
			description = '';
			sensitive = false;
		}
	});

	const nameRegex = /^[a-zA-Z_][a-zA-Z0-9_]*$/;
	let nameTouched = $state(false);
	let valueTouched = $state(false);

	let nameInvalid = $derived(nameTouched && name.length > 0 && !nameRegex.test(name));
	let nameDuplicate = $derived(
		nameTouched &&
			name.length > 0 &&
			nameRegex.test(name) &&
			existingNames
				.filter((n) => (isEditing ? n !== output!.name : true))
				.includes(name)
	);

	let nameError = $derived(
		nameInvalid
			? t('tofu.output_name_invalid')
			: nameDuplicate
				? t('tofu.output_name_exists')
				: ''
	);

	let valueEmpty = $derived(valueTouched && value.trim().length === 0);

	let canSave = $derived(
		name.trim().length > 0 &&
			nameRegex.test(name) &&
			!nameDuplicate &&
			value.trim().length > 0
	);

	function handleSave() {
		nameTouched = true;
		valueTouched = true;
		if (!canSave) return;

		onsave({
			name: name.trim(),
			value: value.trim(),
			description: description.trim(),
			sensitive
		});
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter') {
			e.preventDefault();
			handleSave();
		}
	}
</script>

<Modal {open} {onclose} {title} maxWidth="450px">
	<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
	<form class="form" onsubmit={(e) => { e.preventDefault(); handleSave(); }} onkeydown={handleKeydown}>
		<div class="form-field">
			<label class="form-label" for="output-name">{t('tofu.output_name')}</label>
			<input
				id="output-name"
				class="form-input"
				class:error={nameError.length > 0}
				type="text"
				bind:value={name}
				oninput={() => { nameTouched = true; }}
				placeholder="my_output"
			/>
			{#if nameError}
				<span class="form-error">{nameError}</span>
			{/if}
		</div>

		<div class="form-field">
			<label class="form-label" for="output-value">{t('tofu.output_value')}</label>
			<input
				id="output-value"
				class="form-input mono"
				class:error={valueEmpty}
				type="text"
				bind:value={value}
				oninput={() => { valueTouched = true; }}
				placeholder={t('tofu.output_value_placeholder')}
			/>
		</div>

		<div class="form-field">
			<label class="form-label" for="output-description">{t('tofu.output_description')}</label>
			<input
				id="output-description"
				class="form-input"
				type="text"
				bind:value={description}
				placeholder=""
			/>
		</div>

		<div class="form-field">
			<label class="toggle-label">
				<span class="toggle-wrapper">
					<input
						type="checkbox"
						class="toggle-input"
						bind:checked={sensitive}
					/>
					<span class="toggle-track">
						<span class="toggle-thumb"></span>
					</span>
				</span>
				<span class="toggle-text">{t('tofu.output_sensitive')}</span>
			</label>
		</div>
	</form>

	{#snippet actions()}
		<Button variant="secondary" size="sm" onclick={onclose}>
			{t('common.cancel')}
		</Button>
		<Button variant="primary" size="sm" onclick={handleSave} disabled={!canSave}>
			{t('common.save')}
		</Button>
	{/snippet}
</Modal>

<style>
	.form {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.form-field {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.form-label {
		font-size: 0.75rem;
		font-weight: 500;
		color: var(--color-text-secondary);
	}

	.form-input {
		width: 100%;
		padding: 10px 12px;
		font-family: var(--font-sans);
		font-size: 0.8125rem;
		color: var(--color-text-primary);
		background-color: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		outline: none;
		transition: border-color var(--duration-default) var(--ease-default);
		box-sizing: border-box;
	}

	.form-input:focus {
		border-color: var(--color-accent);
	}

	.form-input.error {
		border-color: var(--color-danger);
	}

	.form-input::placeholder {
		color: var(--color-text-secondary);
		opacity: 0.5;
	}

	.form-input.mono {
		font-family: var(--font-mono, monospace);
	}

	.form-error {
		font-size: 0.6875rem;
		color: var(--color-danger);
	}

	.toggle-label {
		display: inline-flex;
		align-items: center;
		gap: 8px;
		cursor: pointer;
		user-select: none;
	}

	.toggle-wrapper {
		position: relative;
		display: inline-flex;
		align-items: center;
	}

	.toggle-input {
		position: absolute;
		opacity: 0;
		width: 0;
		height: 0;
		pointer-events: none;
	}

	.toggle-track {
		display: inline-block;
		width: 34px;
		height: 18px;
		background: var(--color-border);
		border-radius: 9px;
		position: relative;
		transition: background-color 0.2s ease;
	}

	.toggle-input:checked + .toggle-track {
		background: var(--color-accent);
	}

	.toggle-thumb {
		position: absolute;
		top: 2px;
		left: 2px;
		width: 14px;
		height: 14px;
		background: #fff;
		border-radius: 50%;
		transition: transform 0.2s ease;
	}

	.toggle-input:checked + .toggle-track .toggle-thumb {
		transform: translateX(16px);
	}

	.toggle-text {
		font-size: 0.8125rem;
		color: var(--color-text-secondary);
		font-weight: 500;
	}
</style>
