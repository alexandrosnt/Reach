<script lang="ts">
	/**
	 * Shown over the whole app while the vault is held locked (by the user or by
	 * auto-lock). Sessions that are already open keep running underneath; what
	 * this gates is the vault, and with it every saved password and key.
	 *
	 * With a master password set, a lock is only undone by it or by a device
	 * method (Windows Hello, Touch ID, a security key), never by one click: the backend
	 * refuses that too.
	 */
	import Button from '$lib/components/shared/Button.svelte';
	import Input from '$lib/components/shared/Input.svelte';
	import { hasMasterPassword } from '$lib/ipc/credentials';
	import { unlockMethods, type UnlockMethods } from '$lib/ipc/vault';
	import { t } from '$lib/state/i18n.svelte';
	import { biometricName, resume, unlock, vaultState } from '$lib/state/vault.svelte';

	type View = 'choose' | 'password' | 'key-pin';

	let view = $state<View>('choose');
	let password = $state('');
	let keyPin = $state('');
	let error = $state('');
	let busy = $state(false);
	let passwordAvailable = $state(false);
	let methods = $state<UnlockMethods | null>(null);

	const platform = $derived(methods?.methods.find((m) => m.kind !== 'fido2'));
	const hasBiometric = $derived(!!platform);
	const method = $derived(biometricName(platform?.kind as UnlockMethods['platform']));
	const hasKey = $derived(!!methods?.methods.some((m) => m.kind === 'fido2'));
	const hasDeviceMethod = $derived(hasBiometric || hasKey);
	const passwordHint = $derived(
		t('lock.set_password_hint', { place: `${t('settings.title')} → ${t('settings.security')}` })
	);

	$effect(() => {
		if (vaultState.held) {
			error = '';
			password = '';
			keyPin = '';
			view = 'choose';
			hasMasterPassword()
				.then((has) => {
					passwordAvailable = has;
				})
				.catch(() => {
					passwordAvailable = false;
				});
			unlockMethods()
				.then((m) => {
					methods = m;
				})
				.catch(() => {
					methods = null;
				});
		}
	});

	// Without a device method, the password field is all there is to show.
	const showPassword = $derived(view === 'password' || (view === 'choose' && passwordAvailable && !hasDeviceMethod));

	async function attempt(open: () => Promise<boolean>, failed: string): Promise<void> {
		busy = true;
		error = '';
		try {
			if (!(await open())) error = failed;
		} catch (err) {
			error = String(err);
		} finally {
			busy = false;
		}
	}

	function openWithKey(): void {
		if (methods?.keys_ask_pin && view !== 'key-pin') {
			view = 'key-pin';
			return;
		}
		const pin = keyPin;
		keyPin = '';
		void attempt(() => resume({ key: true, pin: pin || undefined }), t('lock.key_failed'));
	}

	async function openWithPassword(): Promise<void> {
		if (!password) return;
		const entered = password;
		password = '';
		await attempt(() => unlock(entered), t('lock.wrong_password'));
	}
</script>

{#if vaultState.held}
	<div class="lock-screen" role="dialog" aria-modal="true" aria-labelledby="lock-title">
		<div class="card">
			<svg class="icon" viewBox="0 0 24 24" aria-hidden="true">
				<path
					d="M12 2a5 5 0 0 0-5 5v3H6a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-8a2 2 0 0 0-2-2h-1V7a5 5 0 0 0-5-5Zm-3 8V7a3 3 0 1 1 6 0v3H9Z"
				/>
			</svg>
			<h2 id="lock-title">{t('lock.title')}</h2>
			<p class="subtitle">{t('lock.subtitle')}</p>

			{#if showPassword}
				<form
					onsubmit={(e) => {
						e.preventDefault();
						void openWithPassword();
					}}
				>
					<Input type="password" placeholder={t('lock.password_placeholder')} bind:value={password} />
					<Button type="submit" disabled={busy || !password}>{t('lock.unlock')}</Button>
				</form>
				{#if hasDeviceMethod}
					<button class="link" onclick={() => (view = 'choose')}>{t('lock.back')}</button>
				{/if}
			{:else if view === 'key-pin'}
				<form
					onsubmit={(e) => {
						e.preventDefault();
						openWithKey();
					}}
				>
					<Input type="password" placeholder={t('lock.key_pin_placeholder')} bind:value={keyPin} />
					<p class="hint">{t('lock.key_touch')}</p>
					<Button type="submit" disabled={busy || !keyPin}>{t('lock.unlock')}</Button>
				</form>
				<button class="link" onclick={() => (view = 'choose')}>{t('lock.back')}</button>
			{:else}
				{#if hasBiometric}
					<Button
						onclick={() => void attempt(() => resume({ biometric: true }), t('lock.biometric_failed', { method }))}
						disabled={busy}
					>
						{t('lock.unlock_biometric', { method })}
					</Button>
				{/if}
				{#if hasKey}
					<Button variant={hasBiometric ? 'secondary' : 'primary'} onclick={openWithKey} disabled={busy}>
						{t('lock.unlock_key')}
					</Button>
					{#if busy && !methods?.keys_ask_pin}
						<p class="hint">{t('lock.key_touch')}</p>
					{/if}
				{/if}
				{#if !hasDeviceMethod}
					<Button onclick={() => void attempt(() => resume(), t('lock.unlock_failed'))} disabled={busy}>
						{t('lock.unlock')}
					</Button>
					<p class="hint">{passwordHint}</p>
				{/if}
				{#if passwordAvailable && hasDeviceMethod}
					<button class="link" onclick={() => (view = 'password')}>{t('lock.use_password')}</button>
				{/if}
			{/if}

			{#if error}
				<p class="error" role="alert">{error}</p>
			{/if}
		</div>
	</div>
{/if}

<style>
	.lock-screen {
		position: fixed;
		inset: 0;
		/* Clear of the phone's status and navigation bars (0 on a desktop). */
		padding: var(--safe-top) var(--safe-right) var(--safe-bottom) var(--safe-left);
		z-index: 10000;
		display: flex;
		align-items: center;
		justify-content: center;
		background: color-mix(in srgb, var(--color-bg-primary, #111) 88%, transparent);
		backdrop-filter: blur(14px);
	}

	.card {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 12px;
		width: min(360px, calc(100vw - 32px));
		padding: 28px 24px;
		border-radius: 12px;
		background: var(--color-bg-secondary, #1b1b1b);
		border: 1px solid var(--color-border, #333);
		text-align: center;
	}

	.icon {
		width: 36px;
		height: 36px;
		fill: var(--color-accent, #4c8bf5);
	}

	h2 {
		margin: 0;
		font-size: 1.15em;
		color: var(--color-text-primary, #eee);
	}

	.subtitle {
		margin: 0 0 6px;
		font-size: 0.9em;
		color: var(--color-text-secondary, #aaa);
	}

	form {
		display: flex;
		flex-direction: column;
		gap: 8px;
		width: 100%;
	}

	.link {
		background: none;
		border: none;
		color: var(--color-accent, #4c8bf5);
		cursor: pointer;
		font-size: 0.85em;
	}

	.hint {
		margin: 0;
		font-size: 0.8em;
		color: var(--color-text-secondary, #aaa);
	}

	.error {
		margin: 0;
		font-size: 0.85em;
		color: var(--color-danger, #e5534b);
	}
</style>
