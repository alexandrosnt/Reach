<script lang="ts">
	import { onMount } from 'svelte';
	import { t } from '$lib/state/i18n.svelte';
	import {
		getActiveProviders,
		getProviderCatalog,
		updateProviderConfig
	} from '$lib/state/tofu.svelte';
	import Button from '$lib/components/shared/Button.svelte';

	interface Props {
		providerId: string;
		onclose: () => void;
	}

	let { providerId, onclose }: Props = $props();

	let providerConfig = $derived(getActiveProviders().find((p) => p.providerId === providerId));
	let catalogEntry = $derived(getProviderCatalog().find((c) => c.id === providerId));

	let fields = $state<Record<string, unknown>>({});
	let version = $state('');

	onMount(() => {
		const config = getActiveProviders().find((p) => p.providerId === providerId);
		if (config) {
			fields = { ...config.fields };
			version = config.version;
		}
	});

	function setField(name: string, value: unknown) {
		fields = { ...fields, [name]: value };
	}

	async function handleSave() {
		if (!providerConfig) return;
		await updateProviderConfig({
			providerId,
			source: providerConfig.source,
			version,
			fields
		});
		onclose();
	}

	function handleBackdropClick(e: MouseEvent) {
		if (e.target === e.currentTarget) {
			onclose();
		}
	}

	$effect(() => {
		function onKeydown(e: KeyboardEvent) {
			if (e.key === 'Escape') {
				onclose();
			}
		}

		document.addEventListener('keydown', onKeydown);

		return () => {
			document.removeEventListener('keydown', onKeydown);
		};
	});
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="overlay" onclick={handleBackdropClick} onkeydown={() => {}}>
	<div class="modal" role="dialog" aria-modal="true" aria-label={catalogEntry?.name ?? t('tofu.configure_provider')}>
		<header class="modal-header">
			<h2 class="modal-title">{catalogEntry?.name ?? providerId}</h2>
			<button type="button" class="modal-close" onclick={onclose} aria-label={t('common.close_dialog')}>
				<svg width="14" height="14" viewBox="0 0 14 14" fill="none">
					<path d="M1 1L13 13M13 1L1 13" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
				</svg>
			</button>
		</header>

		<div class="modal-body">
			<div class="field-group">
				<label class="field-label" for="provider-version">{t('tofu.version')}</label>
				<input
					id="provider-version"
					type="text"
					class="field-input"
					placeholder={t('tofu.provider_version_placeholder')}
					bind:value={version}
				/>
			</div>

			{#if catalogEntry}
				{#each catalogEntry.fields as schema (schema.name)}
					<div class="field-group">
						<label class="field-label" for="field-{schema.name}">
							{schema.label}
							{#if schema.required}
								<span class="required">*</span>
							{/if}
						</label>

						{#if schema.fieldType === 'string'}
							<input
								id="field-{schema.name}"
								type="text"
								class="field-input"
								value={fields[schema.name] ?? schema.defaultValue ?? ''}
								oninput={(e) => setField(schema.name, e.currentTarget.value)}
							/>
						{:else if schema.fieldType === 'number'}
							<input
								id="field-{schema.name}"
								type="number"
								class="field-input"
								value={fields[schema.name] ?? schema.defaultValue ?? ''}
								oninput={(e) => setField(schema.name, Number(e.currentTarget.value))}
							/>
						{:else if schema.fieldType === 'bool'}
							<label class="toggle-label">
								<span class="toggle-wrapper">
									<input
										type="checkbox"
										class="toggle-input"
										checked={fields[schema.name] === true}
										onchange={(e) => setField(schema.name, e.currentTarget.checked)}
									/>
									<span class="toggle-track">
										<span class="toggle-thumb"></span>
									</span>
								</span>
							</label>
						{:else if schema.fieldType === 'select'}
							<select
								id="field-{schema.name}"
								class="field-input"
								value={fields[schema.name] ?? schema.defaultValue ?? ''}
								onchange={(e) => setField(schema.name, e.currentTarget.value)}
							>
								{#each schema.options as opt (opt.value)}
									<option value={opt.value}>{opt.label}</option>
								{/each}
							</select>
						{:else if schema.fieldType === 'sensitive'}
							<input
								id="field-{schema.name}"
								type="password"
								class="field-input"
								value={fields[schema.name] ?? ''}
								oninput={(e) => setField(schema.name, e.currentTarget.value)}
							/>
						{/if}

						{#if schema.helpText}
							<p class="field-help">{schema.helpText}</p>
						{/if}
					</div>
				{/each}
			{/if}
		</div>

		<footer class="modal-actions">
			<Button variant="secondary" size="sm" onclick={onclose}>
				{t('tofu.cancel')}
			</Button>
			<Button variant="primary" size="sm" onclick={handleSave}>
				{t('tofu.save')}
			</Button>
		</footer>
	</div>
</div>

<style>
	.overlay {
		position: fixed;
		inset: 0;
		z-index: 100;
		display: flex;
		align-items: center;
		justify-content: center;
		background: rgba(0, 0, 0, 0.5);
		backdrop-filter: blur(4px);
		animation: fadeIn var(--duration-default) var(--ease-default);
	}

	.modal {
		background-color: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-modal);
		box-shadow: var(--shadow-elevated);
		min-width: 320px;
		max-width: 500px;
		width: 90%;
		max-height: 85vh;
		display: flex;
		flex-direction: column;
		overflow: hidden;
		animation: scaleIn var(--duration-default) var(--ease-default);
	}

	.modal-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 16px 20px;
		border-bottom: 1px solid var(--color-border);
	}

	.modal-title {
		font-size: 1rem;
		font-weight: 600;
		color: var(--color-text-primary);
		margin: 0;
	}

	.modal-close {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 28px;
		height: 28px;
		border: none;
		border-radius: 6px;
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
		transition: background-color var(--duration-default) var(--ease-default);
	}

	.modal-close:hover {
		background-color: rgba(255, 255, 255, 0.08);
		color: var(--color-text-primary);
	}

	.modal-body {
		padding: 20px;
		overflow-y: auto;
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.modal-actions {
		display: flex;
		align-items: center;
		justify-content: flex-end;
		gap: 8px;
		padding: 16px 20px;
		border-top: 1px solid var(--color-border);
	}

	.field-group {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.field-label {
		font-size: 0.8125rem;
		font-weight: 500;
		color: var(--color-text-secondary);
	}

	.required {
		color: var(--color-danger);
		margin-left: 2px;
	}

	.field-input {
		background: var(--color-bg-primary);
		border: 1px solid var(--color-border);
		color: var(--color-text-primary);
		border-radius: var(--radius-btn);
		padding: 8px 10px;
		font-size: 0.8125rem;
		font-family: var(--font-sans);
		outline: none;
		transition: border-color var(--duration-default) var(--ease-default);
		width: 100%;
		box-sizing: border-box;
	}

	.field-input:focus {
		border-color: var(--color-accent);
	}

	.field-help {
		margin: 0;
		font-size: 0.75rem;
		color: var(--color-text-secondary);
		opacity: 0.7;
		line-height: 1.4;
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

	@keyframes fadeIn {
		from {
			opacity: 0;
		}
		to {
			opacity: 1;
		}
	}

	@keyframes scaleIn {
		from {
			opacity: 0;
			transform: scale(0.95);
		}
		to {
			opacity: 1;
			transform: scale(1);
		}
	}
</style>
