<script lang="ts">
	import Modal from '$lib/components/shared/Modal.svelte';
	import Button from '$lib/components/shared/Button.svelte';
	import Input from '$lib/components/shared/Input.svelte';
	import { setMasterPassword } from '$lib/ipc/credentials';
	import { isPasswordPromptOpen, closePasswordPrompt } from '$lib/state/password-prompt.svelte';
	import { addToast } from '$lib/state/toasts.svelte';
	import { t } from '$lib/state/i18n.svelte';
	import { tick } from 'svelte';

	let open = $derived(isPasswordPromptOpen());
	let password = $state('');
	let confirm = $state('');
	let error = $state('');
	let saving = $state(false);
	let form = $state<HTMLFormElement>();

	$effect(() => {
		if (!open) return;
		password = '';
		confirm = '';
		error = '';
		void tick().then(() => form?.querySelector('input')?.focus());
	});

	// The same rules as Settings > Security, where it can be changed later.
	async function save(): Promise<void> {
		error = '';
		if (password.length < 8) {
			error = t('security.password_too_short');
			return;
		}
		if (password !== confirm) {
			error = t('security.passwords_mismatch');
			return;
		}
		saving = true;
		try {
			await setMasterPassword(password);
			password = '';
			confirm = '';
			closePasswordPrompt();
			addToast(t('security.prompt_done'), 'success');
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		}
		saving = false;
	}
</script>

<Modal {open} onclose={closePasswordPrompt} title={t('security.prompt_title')} maxWidth="440px">
	<form
		bind:this={form}
		class="prompt"
		onsubmit={(e) => {
			e.preventDefault();
			void save();
		}}
	>
		<p class="why">{t('security.prompt_desc')}</p>
		<Input label={t('security.new_password')} type="password" placeholder={t('settings.new_password_placeholder')} bind:value={password} />
		<Input label={t('security.confirm_password')} type="password" placeholder={t('common.reenter_password')} bind:value={confirm} />
		{#if error}
			<p class="error" role="alert">{error}</p>
		{/if}
		<!-- Enter in either field sets it, as a form does. -->
		<button type="submit" hidden aria-hidden="true" tabindex="-1"></button>
	</form>
	{#snippet actions()}
		<Button variant="secondary" size="sm" onclick={closePasswordPrompt}>{t('security.prompt_later')}</Button>
		<Button variant="primary" size="sm" disabled={saving || password.length === 0} onclick={() => void save()}>
			{saving ? t('security.saving') : t('security.save_password')}
		</Button>
	{/snippet}
</Modal>

<style>
	.prompt {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.why {
		margin: 0;
		font-size: var(--text-sm);
		line-height: 1.5;
		color: var(--color-text-secondary);
	}

	.error {
		margin: 0;
		font-size: var(--text-xs);
		color: var(--color-danger);
	}
</style>
