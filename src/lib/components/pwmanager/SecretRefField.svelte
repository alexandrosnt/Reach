<script lang="ts">
	/**
	 * A secret field (password, passphrase) that can instead point at an entry
	 * in a password manager.
	 *
	 * With a reference set, the field shows "Passbolt · <entry>" and the typed
	 * secret is cleared: the backend fetches the secret when connecting, so it
	 * is never stored in the session and never comes to the webview. The key
	 * button appears only when a password manager is turned on and configured.
	 *
	 * The plaintext input is the shared Input by default; a form with its own
	 * input style passes it as the `input` snippet, bound to the same value.
	 */
	import { onMount, type Snippet } from 'svelte';
	import Input from '$lib/components/shared/Input.svelte';
	import SecretRefPicker from './SecretRefPicker.svelte';
	import type { EntrySummary, ProviderStatus, SecretRef } from '$lib/ipc/pwmanager';
	import {
		ensurePwManagers,
		providerName,
		pwManagerProviders,
		pwManagersLoaded,
		pwManagerStatusError,
		refreshPwManagers,
		usableProviders
	} from '$lib/state/pwmanager.svelte';
	import { t } from '$lib/state/i18n.svelte';

	interface Props {
		/** The typed secret. Cleared when a reference is picked. */
		value?: string;
		secretRef?: SecretRef | null;
		label?: string;
		placeholder?: string;
		disabled?: boolean;
		/** Called after a pick, e.g. to fill an empty user name from the entry. */
		onpick?: (entry: EntrySummary) => void;
		/** A custom plaintext input, for forms that do not use the shared Input. */
		input?: Snippet;
		/**
		 * A saved session's field: its entry counts only once the user has
		 * approved it for the session (the backend signs it), so an entry
		 * someone else put there is marked as not confirmed.
		 */
		approvable?: boolean;
		/**
		 * Only a reference, no typed secret: the "Password manager" login
		 * method. Shows an entry chooser, or why none can be chosen.
		 */
		refOnly?: boolean;
	}

	let {
		value = $bindable(''),
		secretRef = $bindable(null),
		label = '',
		placeholder = '',
		disabled = false,
		onpick,
		input,
		approvable = false,
		refOnly = false
	}: Props = $props();

	let pickerOpen = $state(false);

	onMount(() => {
		void ensurePwManagers();
	});

	let providers = $derived(usableProviders());
	let canPick = $derived(providers.length > 0);
	/** A reference whose provider cannot be reached now: it is kept, but say so. */
	let unavailable = $derived(!!secretRef && pwManagersLoaded() && !providers.some((p) => p.id === secretRef?.provider));
	let refText = $derived(
		secretRef ? `${providerName(secretRef.provider)} · ${secretRef.label?.trim() || secretRef.entry_id}` : ''
	);

	/**
	 * Not approved by this user for the session as it is: someone else put it
	 * there, or the session changed since. The label is whatever the last
	 * editor wrote, so it says nothing about which entry this really is.
	 */
	/** Why nothing can be picked right now, for the reference-only field. */
	let reason = $derived.by(() => {
		if (canPick) return '';
		if (!pwManagersLoaded()) return t('pwmanager.reason_checking');
		const err = pwManagerStatusError();
		if (err) return t('pwmanager.reason_error', { error: err });
		const p = pwManagerProviders()[0];
		if (!p) return t('pwmanager.reason_none');
		if (!p.enabled) return t('pwmanager.reason_off');
		if (!p.available) return t('pwmanager.reason_missing', { name: p.name });
		return t('pwmanager.reason_unconfigured', { name: p.name });
	});

	let unconfirmed = $derived(approvable && !!secretRef && !secretRef.trusted && !secretRef.confirm);

	function pick(entry: EntrySummary, provider: ProviderStatus): void {
		// Picked here by the user: saving approves it for this session.
		secretRef = { provider: provider.id, entry_id: entry.id, label: entry.name || null, confirm: true };
		value = '';
		pickerOpen = false;
		onpick?.(entry);
	}

	function clearRef(): void {
		secretRef = null;
	}

	/**
	 * The picker is drawn on <body>, not here: a field sits inside the
	 * editor's <form> (whose buttons would submit it) and inside its dialog.
	 */
	function portal(node: HTMLElement): { destroy: () => void } {
		document.body.appendChild(node);
		return { destroy: () => node.remove() };
	}
</script>

<div class="srf" class:compact={!!input}>
	{#if secretRef}
		<div
			class="ref-box"
			class:compact={!!input}
			class:disabled
			class:unconfirmed
			title={unconfirmed ? t('pwmanager.ref_unconfirmed') : unavailable ? t('pwmanager.ref_unavailable') : refText}
		>
			{#if label && !input}
				<span class="ref-label">{label}</span>
			{/if}
			<span class="ref-row">
				<svg class="ref-icon" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
					<circle cx="7.5" cy="15.5" r="4.5" />
					<path d="M10.7 12.3L21 2M17 6l3 3M14.5 8.5l2.5 2.5" />
				</svg>
				<span class="ref-text">{refText}</span>
				{#if unconfirmed}
					<span class="ref-warn">{t('pwmanager.ref_unconfirmed_short')}</span>
				{:else if unavailable}
					<span class="ref-warn">{t('pwmanager.ref_unavailable_short')}</span>
				{/if}
				<button
					type="button"
					class="ref-clear"
					onclick={clearRef}
					{disabled}
					aria-label={t('pwmanager.ref_clear', { name: label || t('session.password') })}
					title={t('pwmanager.ref_clear', { name: label || t('session.password') })}
				>
					<svg width="10" height="10" viewBox="0 0 14 14" fill="none" aria-hidden="true">
						<path d="M1 1L13 13M13 1L1 13" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" />
					</svg>
				</button>
			</span>
		</div>
	{:else if refOnly}
		<div class="ref-box choose" class:disabled>
			{#if label}
				<span class="ref-label">{label}</span>
			{/if}
			{#if canPick}
				<button type="button" class="choose-btn" onclick={() => (pickerOpen = true)} {disabled}>
					<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
						<circle cx="7.5" cy="15.5" r="4.5" />
						<path d="M10.7 12.3L21 2M17 6l3 3M14.5 8.5l2.5 2.5" />
					</svg>
					{t('pwmanager.choose_entry')}
				</button>
			{:else}
				<span class="reason">
					{reason}
					{#if pwManagersLoaded()}
						<button type="button" class="link-btn" onclick={() => void refreshPwManagers()} {disabled}>{t('pwmanager.check_again_short')}</button>
					{/if}
				</span>
			{/if}
		</div>
	{:else}
		<div class="input-area">
			{#if input}
				{@render input()}
			{:else}
				<Input {label} bind:value type="password" {placeholder} {disabled} />
			{/if}
		</div>
		{#if canPick}
			<button
				type="button"
				class="pick-btn"
				onclick={() => (pickerOpen = true)}
				{disabled}
				aria-label={t('pwmanager.from_manager_for', { name: label || t('session.password') })}
				title={t('pwmanager.from_manager')}
			>
				<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
					<circle cx="7.5" cy="15.5" r="4.5" />
					<path d="M10.7 12.3L21 2M17 6l3 3M14.5 8.5l2.5 2.5" />
				</svg>
			</button>
		{/if}
	{/if}
</div>

{#if pickerOpen}
	<div class="picker-host" use:portal>
		<SecretRefPicker open={pickerOpen} {providers} onpick={pick} onclose={() => (pickerOpen = false)} />
	</div>
{/if}

<style>
	.picker-host {
		display: contents;
	}

	.srf {
		display: flex;
		align-items: stretch;
		gap: 6px;
		width: 100%;
		min-width: 0;
	}

	.input-area {
		flex: 1;
		min-width: 0;
	}

	.pick-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 38px;
		flex-shrink: 0;
		padding: 0;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		background: var(--color-bg-elevated);
		color: var(--color-text-secondary);
		cursor: pointer;
		transition:
			background-color var(--duration-default) var(--ease-default),
			color var(--duration-default) var(--ease-default);
	}

	.srf.compact .pick-btn {
		width: 32px;
		border-radius: var(--radius-sm, var(--radius-btn));
	}

	.pick-btn:hover:not(:disabled) {
		color: var(--color-accent);
		background: var(--color-surface-hover);
	}

	.pick-btn:focus-visible {
		outline: 2px solid var(--color-accent);
		outline-offset: 1px;
	}

	.pick-btn:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}

	/* Dressed as the Input it replaces: same frame, label on top. */
	.ref-box {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		justify-content: center;
		gap: 2px;
		padding: 6px 8px 6px 12px;
		min-height: 46px;
		box-sizing: border-box;
		background-color: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
	}

	.ref-box.compact {
		min-height: 0;
		padding: 4px 6px 4px 9px;
		border-radius: var(--radius-sm, var(--radius-btn));
		background: var(--color-surface-sunken, var(--color-bg-primary));
	}

	.ref-box.disabled {
		opacity: 0.4;
	}

	.ref-box.unconfirmed {
		border-color: var(--color-warning);
	}

	.ref-box.choose {
		align-items: flex-start;
	}

	.choose-btn {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		padding: 2px 0;
		border: none;
		background: none;
		color: var(--color-accent);
		font: inherit;
		font-size: 0.8125rem;
		cursor: pointer;
	}

	.choose-btn:hover:not(:disabled) {
		text-decoration: underline;
	}

	.choose-btn:focus-visible,
	.link-btn:focus-visible {
		outline: 2px solid var(--color-accent);
		outline-offset: 1px;
	}

	.reason {
		font-size: 0.75rem;
		color: var(--color-text-secondary);
	}

	.link-btn {
		margin-left: 4px;
		padding: 0;
		border: none;
		background: none;
		color: var(--color-accent);
		font: inherit;
		cursor: pointer;
	}

	.ref-label {
		font-size: 0.625rem;
		color: var(--color-accent);
	}

	.ref-row {
		display: flex;
		align-items: center;
		gap: 6px;
		min-width: 0;
	}

	.ref-icon {
		flex-shrink: 0;
		color: var(--color-accent);
	}

	.ref-text {
		flex: 1;
		min-width: 0;
		font-size: 0.8125rem;
		color: var(--color-text-primary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.ref-warn {
		flex-shrink: 0;
		font-size: 0.6875rem;
		color: var(--color-warning);
	}

	.ref-clear {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 22px;
		height: 22px;
		flex-shrink: 0;
		padding: 0;
		border: none;
		border-radius: 4px;
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
	}

	.ref-clear:hover:not(:disabled) {
		background: var(--color-surface-active);
		color: var(--color-text-primary);
	}

	.ref-clear:disabled {
		cursor: not-allowed;
	}
</style>
