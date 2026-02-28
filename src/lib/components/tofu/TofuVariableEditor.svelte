<script lang="ts">
	import type { TofuVariable, TofuVarType } from '$lib/ipc/tofu';
	import { t } from '$lib/state/i18n.svelte';
	import Button from '$lib/components/shared/Button.svelte';
	import Modal from '$lib/components/shared/Modal.svelte';

	interface Props {
		variable: TofuVariable | null;
		existingNames: string[];
		onsave: (v: TofuVariable) => void;
		onclose: () => void;
	}

	let { variable, existingNames, onsave, onclose }: Props = $props();

	const varTypeOptions: TofuVarType[] = ['string', 'number', 'bool', 'list', 'map'];

	let name = $state('');
	let varType: TofuVarType = $state('string');
	let description = $state('');
	let defaultValue = $state('');
	let sensitive = $state(false);

	let isEditing = $derived(variable !== null);
	let title = $derived(isEditing ? t('tofu.edit_variable') : t('tofu.add_variable'));

	$effect(() => {
		if (variable) {
			name = variable.name;
			varType = variable.varType;
			description = variable.description;
			defaultValue = variable.defaultValue ?? '';
			sensitive = variable.sensitive;
		}
	});

	let nameRegex = /^[a-zA-Z_]\w*$/;
	let nameTouched = $state(false);

	let nameInvalid = $derived(nameTouched && name.length > 0 && !nameRegex.test(name));
	let nameDuplicate = $derived(
		nameTouched &&
			name.length > 0 &&
			nameRegex.test(name) &&
			existingNames
				.filter((n) => (isEditing ? n !== variable!.name : true))
				.includes(name)
	);

	let nameError = $derived(
		nameInvalid
			? t('tofu.variable_name_invalid')
			: nameDuplicate
				? t('tofu.variable_name_exists')
				: ''
	);

	let canSave = $derived(
		name.trim().length > 0 && nameRegex.test(name) && !nameDuplicate
	);

	function handleSave() {
		nameTouched = true;
		if (!canSave) return;

		onsave({
			name: name.trim(),
			varType,
			description: description.trim(),
			defaultValue: defaultValue.trim() || null,
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

<Modal open={true} onclose={onclose} title={title} maxWidth="450px">
	<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
	<form class="form" onsubmit={(e) => { e.preventDefault(); handleSave(); }} onkeydown={handleKeydown}>
		<div class="form-field">
			<label class="form-label" for="var-name">{t('tofu.variable_name')}</label>
			<input
				id="var-name"
				class="form-input"
				class:error={nameError.length > 0}
				type="text"
				bind:value={name}
				oninput={() => { nameTouched = true; }}
				placeholder="my_variable"
			/>
			{#if nameError}
				<span class="form-error">{nameError}</span>
			{/if}
		</div>

		<div class="form-field">
			<label class="form-label" for="var-type">{t('tofu.variable_type')}</label>
			<select id="var-type" class="form-input" bind:value={varType}>
				{#each varTypeOptions as opt (opt)}
					<option value={opt}>{opt}</option>
				{/each}
			</select>
		</div>

		<div class="form-field">
			<label class="form-label" for="var-default">{t('tofu.variable_default')}</label>
			<input
				id="var-default"
				class="form-input"
				type="text"
				bind:value={defaultValue}
				placeholder=""
			/>
		</div>

		<div class="form-field">
			<label class="form-label" for="var-description">{t('tofu.variable_description')}</label>
			<input
				id="var-description"
				class="form-input"
				type="text"
				bind:value={description}
				placeholder=""
			/>
		</div>

		<div class="form-field">
			<label class="toggle-row">
				<input type="checkbox" class="toggle-checkbox" bind:checked={sensitive} />
				<span class="toggle-label">{t('tofu.variable_sensitive')}</span>
			</label>
		</div>
	</form>

	{#snippet actions()}
		<Button variant="secondary" size="sm" onclick={onclose}>
			{t('tofu.cancel')}
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

	select.form-input {
		appearance: none;
		background-image: url("data:image/svg+xml,%3Csvg width='10' height='6' viewBox='0 0 10 6' fill='none' xmlns='http://www.w3.org/2000/svg'%3E%3Cpath d='M1 1L5 5L9 1' stroke='%23888' stroke-width='1.5' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E");
		background-repeat: no-repeat;
		background-position: right 12px center;
		padding-right: 32px;
		cursor: pointer;
	}

	select.form-input option {
		background-color: var(--color-bg-elevated);
		color: var(--color-text-primary);
	}

	.form-error {
		font-size: 0.6875rem;
		color: var(--color-danger);
	}

	.toggle-row {
		display: flex;
		align-items: center;
		gap: 10px;
		cursor: pointer;
	}

	.toggle-checkbox {
		width: 16px;
		height: 16px;
		accent-color: var(--color-accent);
		cursor: pointer;
	}

	.toggle-label {
		font-size: 0.8125rem;
		color: var(--color-text-primary);
	}
</style>
