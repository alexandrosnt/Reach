<script lang="ts">
	/**
	 * Settings → Password managers.
	 *
	 * A saved session can take a password or passphrase from an entry in a
	 * password manager. The secret is fetched by the backend when connecting
	 * and never stored in the session or shown here.
	 *
	 * The provider's own secrets (Passbolt: private key, passphrase, TOTP
	 * secret) are write-only. This page never receives them: it shows whether
	 * one is saved, takes a new one, or removes it.
	 *
	 * The passphrase need not be saved at all: with "Ask when needed" Reach
	 * asks for it (PassphrasePrompt) when a sign-in needs it and keeps it in
	 * memory only, for the time chosen here or not at all. Switching to it
	 * from a stored passphrase removes the stored one.
	 *
	 * The CLI path is the one setting kept on this device only (the backend
	 * stores it outside the synced vault), since the program it names is
	 * handed the private key and passphrase.
	 *
	 * The settings live in the vault: while it is locked there is nothing to
	 * show and nothing can be saved (the backend refuses to hand out
	 * defaults in their place, which a save could write back over the real
	 * ones). The page says to unlock first, and loads once it is open.
	 */
	import { untrack } from 'svelte';
	import { open as openDialog } from '@tauri-apps/plugin-dialog';
	import { openUrl } from '@tauri-apps/plugin-opener';
	import Toggle from '$lib/components/shared/Toggle.svelte';
	import Input from '$lib/components/shared/Input.svelte';
	import Button from '$lib/components/shared/Button.svelte';
	import {
		configInputFromView,
		pwmanagerForgetPassphrase,
		pwmanagerGetConfig,
		pwmanagerSaveConfig,
		pwmanagerTest,
		REMEMBER_CHOICES,
		REMEMBER_DEFAULT,
		type PassboltCredentials,
		type PassboltMfaMode,
		type PassboltPassphraseMode,
		type PasswordManagersConfigInput,
		type PasswordManagersView,
		type TestResult
	} from '$lib/ipc/pwmanager';
	import { pwManagerProviders, pwManagersLoaded, refreshPwManagers, rememberLabel } from '$lib/state/pwmanager.svelte';
	import { addToast } from '$lib/state/toasts.svelte';
	import { t } from '$lib/state/i18n.svelte';
	import { isLocked } from '$lib/state/vault.svelte';

	const RELEASES_URL = 'https://github.com/passbolt/go-passbolt-cli/releases';
	const TIMEOUT_MIN = 10;
	const TIMEOUT_MAX = 300;
	const TIMEOUT_DEFAULT = 60;

	let view = $state<PasswordManagersView | null>(null);
	let loadError = $state<string | null>(null);
	let busy = $state(false);
	let checking = $state(false);
	let testing = $state(false);
	let testResult = $state<TestResult | null>(null);
	let testError = $state<string | null>(null);
	let saveError = $state<string | null>(null);
	/** Redraws the switch after a refused save, so it shows what is stored. */
	let toggleKey = $state(0);

	// The form: non-secret settings, as edited.
	let cliPath = $state('');
	let credentials = $state<PassboltCredentials>('reach');
	let serverAddress = $state('');
	let mfaMode = $state<PassboltMfaMode>('none');
	let timeoutStr = $state(String(TIMEOUT_DEFAULT));
	let passphraseMode = $state<PassboltPassphraseMode>('ask');
	let rememberSecs = $state<number>(REMEMBER_DEFAULT);
	let forgetting = $state(false);

	// New secrets, sent once on Save and then forgotten.
	let privateKeyFile = $state('');
	let privateKeyText = $state('');
	let pasteKey = $state(false);
	let passphrase = $state('');
	let totpSecret = $state('');

	let status = $derived(pwManagerProviders().find((p) => p.id === 'passbolt') ?? null);
	let enabled = $derived(view?.enabled ?? false);
	let reachCreds = $derived(credentials === 'reach');

	let timeoutSecs = $derived.by(() => {
		const n = parseInt(timeoutStr, 10);
		if (!Number.isFinite(n)) return TIMEOUT_DEFAULT;
		return Math.min(TIMEOUT_MAX, Math.max(TIMEOUT_MIN, n));
	});

	let serverError = $derived(
		reachCreds && serverAddress.trim() !== '' && !/^https:\/\/[^\s/@]+/i.test(serverAddress.trim())
			? t('pwmanager.server_https')
			: null
	);

	let asking = $derived(passphraseMode === 'ask');

	/** The fixed choices, plus a stored value that is none of them (set by hand). */
	let rememberOptions = $derived(
		(REMEMBER_CHOICES as readonly number[]).includes(rememberSecs)
			? [...REMEMBER_CHOICES]
			: [...REMEMBER_CHOICES, rememberSecs].sort((a, b) => a - b)
	);

	let pendingSecrets = $derived(
		!!privateKeyFile ||
			(pasteKey && privateKeyText.trim() !== '') ||
			(!asking && passphrase !== '') ||
			totpSecret.trim() !== ''
	);

	let dirty = $derived.by(() => {
		if (!view) return false;
		const p = view.passbolt;
		return (
			pendingSecrets ||
			(cliPath.trim() || null) !== (p.cli_path?.trim() || null) ||
			credentials !== p.credentials ||
			serverAddress.trim() !== p.server_address ||
			mfaMode !== p.mfa_mode ||
			timeoutSecs !== p.timeout_secs ||
			passphraseMode !== p.passphrase_mode ||
			rememberSecs !== p.passphrase_remember_secs
		);
	});

	// Load once the vault is open (now or later); forget the page's state
	// when it locks, so nothing stale can be saved.
	$effect(() => {
		const locked = isLocked();
		untrack(() => {
			if (locked) {
				view = null;
				loadError = null;
				saveError = null;
				testResult = null;
				testError = null;
				forgetSecrets();
			} else {
				void load();
			}
		});
	});

	function fill(v: PasswordManagersView): void {
		view = v;
		cliPath = v.passbolt.cli_path ?? '';
		credentials = v.passbolt.credentials;
		serverAddress = v.passbolt.server_address;
		mfaMode = v.passbolt.mfa_mode;
		timeoutStr = String(v.passbolt.timeout_secs || TIMEOUT_DEFAULT);
		passphraseMode = v.passbolt.passphrase_mode;
		rememberSecs = v.passbolt.passphrase_remember_secs;
	}

	/** Whether a passphrase is in memory now: it changes behind this page's back (asked, expired). */
	async function refreshCached(): Promise<void> {
		try {
			const fresh = await pwmanagerGetConfig();
			if (view) view = { ...view, passbolt: { ...view.passbolt, passphrase_cached: fresh.passbolt.passphrase_cached } };
		} catch {
			// Shown as it was.
		}
	}

	async function forgetNow(): Promise<void> {
		forgetting = true;
		try {
			await pwmanagerForgetPassphrase();
			addToast(t('pwmanager.passphrase_forgotten'), 'success');
		} catch (e) {
			saveError = String(e);
		} finally {
			forgetting = false;
			await refreshCached();
		}
	}

	function forgetSecrets(): void {
		privateKeyFile = '';
		privateKeyText = '';
		pasteKey = false;
		passphrase = '';
		totpSecret = '';
	}

	async function load(): Promise<void> {
		loadError = null;
		try {
			fill(await pwmanagerGetConfig());
		} catch (e) {
			// Locked meanwhile: the page says so instead.
			loadError = isLocked() ? null : String(e);
		}
		await checkStatus();
	}

	async function checkStatus(): Promise<void> {
		checking = true;
		try {
			await refreshPwManagers();
		} finally {
			checking = false;
		}
	}

	/** Save what is stored now with one change, leaving the form's edits pending. */
	async function saveStored(change: (input: PasswordManagersConfigInput) => void): Promise<boolean> {
		if (!view || isLocked()) return false;
		const input = configInputFromView(view);
		change(input);
		busy = true;
		saveError = null;
		try {
			view = await pwmanagerSaveConfig(input);
			testResult = null;
			await checkStatus();
			return true;
		} catch (e) {
			saveError = String(e);
			return false;
		} finally {
			busy = false;
		}
	}

	async function setEnabled(on: boolean): Promise<void> {
		const ok = await saveStored((input) => (input.enabled = on));
		// The switch shows what is stored, so a refused save puts it back.
		if (!ok) toggleKey++;
	}

	async function removeSecret(which: 'private_key' | 'passphrase' | 'totp_secret'): Promise<void> {
		const ok = await saveStored((input) => {
			if (which === 'private_key') input.passbolt.clear_private_key = true;
			else if (which === 'passphrase') input.passbolt.clear_passphrase = true;
			else input.passbolt.clear_totp_secret = true;
		});
		if (ok) addToast(t('pwmanager.secret_removed'), 'success');
	}

	function formInput(): PasswordManagersConfigInput {
		const pasted = pasteKey ? privateKeyText.trim() : '';
		return {
			enabled,
			passbolt: {
				cli_path: cliPath.trim() || null,
				credentials,
				server_address: serverAddress.trim(),
				mfa_mode: mfaMode,
				timeout_secs: timeoutSecs,
				passphrase_mode: passphraseMode,
				passphrase_remember_secs: rememberSecs,
				private_key: pasted || null,
				private_key_file: !pasted && privateKeyFile ? privateKeyFile : null,
				// Asked for: never sent, and the backend removes a stored one
				// when this switches from stored to asked.
				passphrase: !asking && passphrase ? passphrase : null,
				totp_secret: totpSecret.trim() || null
			}
		};
	}

	async function save(): Promise<boolean> {
		if (!view || isLocked() || serverError) return false;
		busy = true;
		saveError = null;
		try {
			const saved = await pwmanagerSaveConfig(formInput());
			forgetSecrets();
			fill(saved);
			testResult = null;
			testError = null;
			await checkStatus();
			return true;
		} catch (e) {
			saveError = String(e);
			return false;
		} finally {
			busy = false;
		}
	}

	async function handleSave(): Promise<void> {
		if (await save()) addToast(t('pwmanager.saved'), 'success');
	}

	/** Test what is saved; unsaved changes are saved first, since that is what is meant. */
	async function test(): Promise<void> {
		if (dirty && !(await save())) return;
		testing = true;
		testResult = null;
		testError = null;
		try {
			testResult = await pwmanagerTest('passbolt');
		} catch (e) {
			testError = String(e);
		} finally {
			testing = false;
		}
		// The test may have asked for the passphrase and kept it.
		await refreshCached();
	}

	async function browseCli(): Promise<void> {
		try {
			const picked = await openDialog({ multiple: false, directory: false, title: t('pwmanager.cli_path_pick') });
			if (typeof picked === 'string') cliPath = picked;
		} catch {
			// Cancelled.
		}
	}

	async function browseKey(): Promise<void> {
		try {
			const picked = await openDialog({
				multiple: false,
				directory: false,
				title: t('pwmanager.private_key_pick'),
				filters: [
					{ name: t('pwmanager.private_key_filter'), extensions: ['asc', 'key', 'gpg', 'pgp', 'txt'] },
					{ name: t('pwmanager.all_files'), extensions: ['*'] }
				]
			});
			if (typeof picked === 'string') {
				privateKeyFile = picked;
				pasteKey = false;
				privateKeyText = '';
			}
		} catch {
			// Cancelled.
		}
	}

	function fileName(path: string): string {
		return path.split(/[\\/]/).pop() || path;
	}
</script>

<div class="tab-content">
	<p class="intro">{t('pwmanager.intro')}</p>

	{#if isLocked()}
		<p class="hint warn" role="status">{t('pwmanager.unlock_first')}</p>
	{:else if loadError}
		<div class="form-error" role="alert">{loadError}</div>
	{/if}

	<div class="setting-row">
		<div class="setting-info">
			<span class="setting-label">{t('pwmanager.enable')}</span>
			<span class="setting-description">{t('pwmanager.enable_desc')}</span>
		</div>
		<div class="setting-control">
			{#key toggleKey}
				<Toggle hideLabel checked={enabled} label={t('pwmanager.enable')} disabled={!view || busy} onchange={(v) => void setEnabled(v)} />
			{/key}
		</div>
	</div>

	{#if view}
		<div class="section provider" class:dimmed={!enabled}>
			<h3 class="section-title">Passbolt</h3>

			<!-- Status: is the CLI here, which version. -->
			<div class="setting-row">
				<div class="setting-info">
					<span class="setting-label">{t('pwmanager.cli')}</span>
					{#if checking && !pwManagersLoaded()}
						<span class="setting-description">{t('pwmanager.checking')}</span>
					{:else if status?.available}
						<span class="setting-description">
							{status.version ? t('pwmanager.cli_found_version', { version: status.version }) : t('pwmanager.cli_found')}
						</span>
						{#if enabled && !status.configured}
							<span class="setting-description warn">{t('pwmanager.not_configured')}</span>
						{/if}
					{:else}
						<span class="setting-description warn">{status?.detail || t('pwmanager.cli_missing')}</span>
					{/if}
				</div>
				<Button variant="secondary" size="sm" onclick={() => void checkStatus()} disabled={checking}>
					{checking ? t('pwmanager.checking') : t('pwmanager.check_again')}
				</Button>
			</div>

			{#if pwManagersLoaded() && !status?.available}
				<div class="install">
					<span class="setting-description">{t('pwmanager.install_hint')}</span>
					<code class="cmd">brew install go-passbolt-cli</code>
					<span class="setting-description">{t('pwmanager.install_release')}</span>
					<div>
						<Button variant="ghost" size="sm" onclick={() => void openUrl(RELEASES_URL)}>{t('pwmanager.open_releases')}</Button>
					</div>
				</div>
			{/if}

			<div class="field-row">
				<div class="grow">
					<Input label={t('pwmanager.cli_path')} bind:value={cliPath} placeholder="passbolt" disabled={busy} />
				</div>
				<Button variant="secondary" size="sm" onclick={browseCli} disabled={busy}>{t('pwmanager.browse')}</Button>
			</div>
			<p class="hint">{t('pwmanager.cli_path_hint')} {t('pwmanager.cli_path_device')}</p>

			<div class="group" role="radiogroup" aria-label={t('pwmanager.credentials')}>
				<span class="group-label">{t('pwmanager.credentials')}</span>
				<div class="segmented">
					<button type="button" role="radio" aria-checked={credentials === 'reach'} class:active={credentials === 'reach'} disabled={busy} onclick={() => (credentials = 'reach')}>
						{t('pwmanager.credentials_reach')}
					</button>
					<button type="button" role="radio" aria-checked={credentials === 'cli_config'} class:active={credentials === 'cli_config'} disabled={busy} onclick={() => (credentials = 'cli_config')}>
						{t('pwmanager.credentials_cli')}
					</button>
				</div>
				<p class="hint">
					{reachCreds
						? asking
							? t('pwmanager.credentials_reach_desc_ask')
							: t('pwmanager.credentials_reach_desc')
						: t('pwmanager.credentials_cli_desc')}
				</p>
			</div>

			{#if reachCreds}
				<div>
					<Input label={t('pwmanager.server')} bind:value={serverAddress} placeholder="https://passbolt.example.com" disabled={busy} />
					{#if serverError}
						<p class="hint error">{serverError}</p>
					{/if}
				</div>

				<!-- Private key: from a file the backend reads, or pasted. -->
				<div class="secret">
					<div class="secret-head">
						<span class="secret-label">{t('pwmanager.private_key')}</span>
						<span class="badge" class:saved={view.passbolt.has_private_key}>
							{view.passbolt.has_private_key ? t('pwmanager.saved_state') : t('pwmanager.not_set')}
						</span>
						{#if view.passbolt.has_private_key}
							<button type="button" class="link-btn danger" onclick={() => void removeSecret('private_key')} disabled={busy}>{t('pwmanager.remove')}</button>
						{/if}
					</div>
					<p class="hint">{t('pwmanager.private_key_hint')}</p>
					<div class="secret-actions">
						<Button variant="secondary" size="sm" onclick={browseKey} disabled={busy}>{t('pwmanager.private_key_file')}</Button>
						<Button variant="ghost" size="sm" onclick={() => { pasteKey = !pasteKey; if (pasteKey) privateKeyFile = ''; }} disabled={busy}>
							{pasteKey ? t('pwmanager.private_key_paste_cancel') : t('pwmanager.private_key_paste')}
						</Button>
					</div>
					{#if privateKeyFile}
						<div class="pending">
							<span class="mono" title={privateKeyFile}>{t('pwmanager.private_key_selected', { name: fileName(privateKeyFile) })}</span>
							<button type="button" class="link-btn" onclick={() => (privateKeyFile = '')} disabled={busy}>{t('common.cancel')}</button>
						</div>
					{/if}
					{#if pasteKey}
						<textarea
							class="key-text"
							bind:value={privateKeyText}
							rows="5"
							spellcheck="false"
							autocomplete="off"
							aria-label={t('pwmanager.private_key')}
							placeholder="-----BEGIN PGP PRIVATE KEY BLOCK-----"
							disabled={busy}
						></textarea>
					{/if}
				</div>

				<!-- Passphrase: asked for when needed (kept in memory only), or stored. -->
				<div class="group" role="radiogroup" aria-label={t('pwmanager.passphrase_mode')}>
					<span class="group-label">{t('pwmanager.passphrase_mode')}</span>
					<div class="segmented">
						<button type="button" role="radio" aria-checked={asking} class:active={asking} disabled={busy} onclick={() => (passphraseMode = 'ask')}>
							{t('pwmanager.passphrase_mode_ask')}
						</button>
						<button type="button" role="radio" aria-checked={!asking} class:active={!asking} disabled={busy} onclick={() => (passphraseMode = 'stored')}>
							{t('pwmanager.passphrase_mode_stored')}
						</button>
					</div>
					<p class="hint">{asking ? t('pwmanager.passphrase_mode_ask_desc') : t('pwmanager.passphrase_mode_stored_desc')}</p>
					{#if asking && view.passbolt.has_passphrase && view.passbolt.passphrase_mode === 'stored'}
						<p class="hint warn" role="note">{t('pwmanager.passphrase_mode_ask_removes')}</p>
					{/if}
				</div>

				{#if asking}
					<div class="secret">
						<label class="secret-label" for="pwm-remember">{t('pwmanager.remember')}</label>
						<div class="field-row">
							<select id="pwm-remember" class="select" bind:value={rememberSecs} disabled={busy}>
								{#each rememberOptions as secs (secs)}
									<option value={secs}>{rememberLabel(secs)}</option>
								{/each}
							</select>
							<Button
								variant="secondary"
								size="sm"
								onclick={() => void forgetNow()}
								disabled={busy || forgetting || !view.passbolt.passphrase_cached}
							>
								{t('pwmanager.forget_now')}
							</Button>
						</div>
						<p class="hint">
							{view.passbolt.passphrase_cached ? t('pwmanager.passphrase_in_memory') : t('pwmanager.passphrase_not_in_memory')}
						</p>
					</div>
				{:else}
					<div class="secret">
						<div class="secret-head">
							<span class="secret-label">{t('pwmanager.passphrase')}</span>
							<span class="badge" class:saved={view.passbolt.has_passphrase}>
								{view.passbolt.has_passphrase ? t('pwmanager.saved_state') : t('pwmanager.not_set')}
							</span>
							{#if view.passbolt.has_passphrase}
								<button type="button" class="link-btn danger" onclick={() => void removeSecret('passphrase')} disabled={busy}>{t('pwmanager.remove')}</button>
							{/if}
						</div>
						<Input
							label={view.passbolt.has_passphrase ? t('pwmanager.passphrase_replace') : t('pwmanager.passphrase')}
							type="password"
							bind:value={passphrase}
							disabled={busy}
						/>
					</div>
				{/if}

				<div class="group" role="radiogroup" aria-label={t('pwmanager.mfa')}>
					<span class="group-label">{t('pwmanager.mfa')}</span>
					<div class="segmented">
						<button type="button" role="radio" aria-checked={mfaMode === 'none'} class:active={mfaMode === 'none'} disabled={busy} onclick={() => (mfaMode = 'none')}>
							{t('pwmanager.mfa_none')}
						</button>
						<button type="button" role="radio" aria-checked={mfaMode === 'totp_secret'} class:active={mfaMode === 'totp_secret'} disabled={busy} onclick={() => (mfaMode = 'totp_secret')}>
							{t('pwmanager.mfa_totp')}
						</button>
					</div>
				</div>

				{#if mfaMode === 'totp_secret'}
					<div class="secret">
						<div class="secret-head">
							<span class="secret-label">{t('pwmanager.totp_secret')}</span>
							<span class="badge" class:saved={view.passbolt.has_totp_secret}>
								{view.passbolt.has_totp_secret ? t('pwmanager.saved_state') : t('pwmanager.not_set')}
							</span>
							{#if view.passbolt.has_totp_secret}
								<button type="button" class="link-btn danger" onclick={() => void removeSecret('totp_secret')} disabled={busy}>{t('pwmanager.remove')}</button>
							{/if}
						</div>
						<Input
							label={view.passbolt.has_totp_secret ? t('pwmanager.totp_secret_replace') : t('pwmanager.totp_secret')}
							type="password"
							bind:value={totpSecret}
							disabled={busy}
						/>
						<p class="hint">{t('pwmanager.totp_hint')}</p>
						<p class="hint warn" role="note">{t('pwmanager.totp_warning')}</p>
					</div>
				{/if}
			{/if}

			<div class="timeout">
				<Input label={t('pwmanager.timeout')} type="number" bind:value={timeoutStr} placeholder={String(TIMEOUT_DEFAULT)} disabled={busy} />
			</div>
			<p class="hint">{t('pwmanager.timeout_hint', { min: String(TIMEOUT_MIN), max: String(TIMEOUT_MAX) })}</p>

			{#if saveError}
				<div class="form-error" role="alert">{saveError}</div>
			{/if}

			{#if testResult || testError}
				<div class="test" class:ok={testResult?.ok} role="status">
					{#if testResult?.ok}
						{t('pwmanager.test_ok', { count: String(testResult.entries) })}
					{:else}
						{testResult?.message || testError}
					{/if}
				</div>
			{/if}

			<div class="action-row">
				{#if !enabled}
					<span class="hint">{t('pwmanager.test_needs_enable')}</span>
				{/if}
				<Button variant="secondary" size="sm" onclick={() => void test()} disabled={!enabled || busy || testing || !!serverError}>
					{testing ? t('pwmanager.testing') : t('pwmanager.test')}
				</Button>
				<Button variant="primary" size="sm" onclick={() => void handleSave()} disabled={!dirty || busy || !!serverError}>
					{busy ? t('pwmanager.saving') : t('pwmanager.save')}
				</Button>
			</div>

			<p class="note">{t('pwmanager.note')}</p>
		</div>
	{/if}
</div>

<style>
	.tab-content {
		gap: 4px;
	}

	.intro {
		margin: 0 0 4px;
		font-size: 0.75rem;
		line-height: 1.45;
		color: var(--color-text-secondary);
	}

	.provider {
		padding-top: 12px;
		transition: opacity var(--duration-default) var(--ease-default);
	}

	/* Off: still editable, so it can be set up before it is switched on. */
	.provider.dimmed {
		opacity: 0.7;
	}

	.setting-description.warn {
		color: var(--color-warning);
	}

	.install {
		display: flex;
		flex-direction: column;
		gap: 6px;
		padding: 10px 12px;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		background: var(--color-surface-hover);
	}

	.cmd {
		align-self: flex-start;
		padding: 3px 8px;
		font-family: var(--font-mono);
		font-size: 0.75rem;
		color: var(--color-text-primary);
		background: var(--color-bg-primary);
		border-radius: 4px;
		user-select: all;
	}

	/* Wraps on a narrow screen instead of running past it. */
	.field-row {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 8px;
	}

	.field-row > .select {
		max-width: 100%;
	}

	.grow {
		flex: 1;
		min-width: 0;
	}

	.hint {
		margin: -6px 0 0;
		font-size: 0.6875rem;
		line-height: 1.45;
		color: var(--color-text-secondary);
	}

	.hint.error {
		margin-top: 4px;
		color: var(--color-danger);
	}

	.hint.warn {
		color: var(--color-warning);
	}

	.action-row .hint {
		margin: 0 auto 0 0;
	}

	.group {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.group .hint {
		margin: 0;
	}

	.group-label,
	.secret-label {
		font-size: 0.6875rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--color-text-secondary);
	}

	.segmented {
		display: flex;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		overflow: hidden;
	}

	.segmented button {
		flex: 1;
		padding: 7px 12px;
		font-family: var(--font-sans);
		font-size: 0.8125rem;
		font-weight: 500;
		border: none;
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
		transition:
			background-color var(--duration-default) var(--ease-default),
			color var(--duration-default) var(--ease-default);
	}

	.segmented button + button {
		border-left: 1px solid var(--color-border);
	}

	.segmented button:hover:not(:disabled):not(.active) {
		background-color: var(--color-surface-hover);
	}

	.segmented button.active {
		background-color: var(--color-accent);
		color: #fff;
	}

	.segmented button:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}

	.secret {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.secret .hint {
		margin: 0;
	}

	.secret-head {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.badge {
		padding: 1px 8px;
		font-size: 0.6875rem;
		font-weight: 500;
		border-radius: 999px;
		background: var(--color-surface-active);
		color: var(--color-text-secondary);
	}

	.badge.saved {
		background: color-mix(in srgb, var(--color-success) 15%, transparent);
		color: var(--color-success);
	}

	.link-btn {
		margin-left: auto;
		padding: 2px 6px;
		font-family: var(--font-sans);
		font-size: 0.6875rem;
		color: var(--color-accent);
		background: transparent;
		border: none;
		border-radius: 4px;
		cursor: pointer;
	}

	.link-btn.danger {
		color: var(--color-danger);
	}

	.link-btn:hover:not(:disabled) {
		background: var(--color-surface-hover);
	}

	.link-btn:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}

	.secret-actions {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
	}

	.pending {
		display: flex;
		align-items: center;
		gap: 8px;
		font-size: 0.75rem;
		color: var(--color-text-primary);
	}

	.pending .mono {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-family: var(--font-mono);
	}

	.key-text {
		width: 100%;
		box-sizing: border-box;
		padding: 8px 10px;
		font-family: var(--font-mono);
		font-size: 0.6875rem;
		color: var(--color-text-primary);
		background: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		resize: vertical;
		outline: none;
	}

	.key-text:focus {
		border-color: var(--color-accent);
	}

	.timeout {
		width: 180px;
	}

	.select {
		min-width: 180px;
		padding: 7px 10px;
		border-radius: var(--radius-btn);
		border: 1px solid var(--color-border);
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		font-family: inherit;
		font-size: 0.8125rem;
	}

	.select:focus {
		outline: none;
		border-color: var(--color-accent);
	}

	.select:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}

	.form-error {
		padding: 8px 10px;
		font-size: 0.75rem;
		color: var(--color-danger);
		background: color-mix(in srgb, var(--color-danger) 10%, transparent);
		border-radius: var(--radius-btn);
		word-break: break-word;
	}

	.test {
		padding: 8px 10px;
		font-size: 0.75rem;
		border-radius: var(--radius-btn);
		background: color-mix(in srgb, var(--color-danger) 12%, transparent);
		color: var(--color-danger);
		word-break: break-word;
	}

	.test.ok {
		background: color-mix(in srgb, var(--color-success) 12%, transparent);
		color: var(--color-success);
	}

	.note {
		margin: 4px 0 0;
		padding: 10px 12px;
		font-size: 0.6875rem;
		line-height: 1.5;
		color: var(--color-text-secondary);
		border-left: 2px solid var(--color-accent);
		background: var(--color-surface-hover);
		border-radius: 0 var(--radius-btn) var(--radius-btn) 0;
	}
</style>
