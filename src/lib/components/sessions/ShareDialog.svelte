<script lang="ts">
	import { vaultState } from '$lib/state/vault.svelte';
	import { sessionShare, type SessionConfig } from '$lib/ipc/sessions';
	import { t } from '$lib/state/i18n.svelte';

	interface Props {
		session: SessionConfig;
		onclose: () => void;
	}

	let { session, onclose }: Props = $props();

	let recipientUuid = $state('');
	let recipientPublicKey = $state('');
	let expiresInHours = $state<number | undefined>(undefined);
	let sharing = $state(false);
	let error = $state('');
	let success = $state(false);
	let shareResult = $state<{ shareId: string; shareUrl: string } | null>(null);

	// Show user's own public key for sharing
	let myPublicKey = $derived(vaultState.publicKey ?? '');
	let myUuid = $derived(vaultState.userUuid ?? '');

	async function handleShare() {
		if (!recipientUuid.trim() || !recipientPublicKey.trim()) {
			error = t('session.recipient_required');
			return;
		}

		// Validate public key is base64 and 32 bytes
		try {
			const decoded = atob(recipientPublicKey.trim());
			if (decoded.length !== 32) {
				error = t('session.invalid_public_key_length');
				return;
			}
		} catch {
			error = t('session.invalid_public_key_base64');
			return;
		}

		sharing = true;
		error = '';

		try {
			const result = await sessionShare(
				session.id,
				recipientUuid.trim(),
				recipientPublicKey.trim(),
				expiresInHours
			);
			shareResult = { shareId: result.shareId, shareUrl: result.shareUrl };
			success = true;
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			sharing = false;
		}
	}

	function copyToClipboard(text: string) {
		navigator.clipboard.writeText(text);
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="share-overlay" onclick={onclose} onkeydown={(e) => { if (e.key === 'Escape') onclose(); }} role="presentation">
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div class="share-dialog" onclick={(e) => e.stopPropagation()} onkeydown={() => {}} role="dialog" aria-modal="true" tabindex="-1">
		<div class="dialog-header">
			<h3>{t('session.share_session')}</h3>
			<button class="close-btn" onclick={onclose} aria-label={t('common.close')}>
				<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M18 6L6 18M6 6l12 12"/>
				</svg>
			</button>
		</div>

		{#if success && shareResult}
			<div class="success-content">
				<div class="success-icon">
					<svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="#10b981" stroke-width="2">
						<path d="M20 6L9 17l-5-5"/>
					</svg>
				</div>
				<p class="success-text">{t('session.share_success')}</p>
				<p class="share-id">{t('session.share_id')} <code>{shareResult.shareId}</code></p>
				<p class="share-hint">{t('session.share_hint')}</p>
				<button class="primary-btn" onclick={onclose}>{t('session.done')}</button>
			</div>
		{:else}
			<div class="dialog-content">
				<p class="share-info">{t('session.sharing_name')} <strong>{session.name}</strong></p>

				<div class="my-info">
					<p class="info-label">{t('session.your_identity')}</p>
					<div class="info-row">
						<span class="info-key">{t('session.uuid')}</span>
						<code class="info-value">{myUuid}</code>
						<button class="copy-btn" onclick={() => copyToClipboard(myUuid)} title={t('common.copy')}>
							<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
								<rect x="9" y="9" width="13" height="13" rx="2"/>
								<path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
							</svg>
						</button>
					</div>
					<div class="info-row">
						<span class="info-key">{t('session.public_key')}</span>
						<code class="info-value">{myPublicKey}</code>
						<button class="copy-btn" onclick={() => copyToClipboard(myPublicKey)} title={t('common.copy')}>
							<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
								<rect x="9" y="9" width="13" height="13" rx="2"/>
								<path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
							</svg>
						</button>
					</div>
				</div>

				<div class="form-group">
					<label for="recipient-uuid">{t('session.recipient_uuid')}</label>
					<input
						id="recipient-uuid"
						type="text"
						bind:value={recipientUuid}
						placeholder="e.g., 550e8400-e29b-41d4-a716-446655440000"
						disabled={sharing}
					/>
				</div>

				<div class="form-group">
					<label for="recipient-key">{t('session.recipient_public_key')}</label>
					<input
						id="recipient-key"
						type="text"
						bind:value={recipientPublicKey}
						placeholder="X25519 public key in base64"
						disabled={sharing}
					/>
				</div>

				<div class="form-group">
					<label for="expires">{t('session.expires_hours')}</label>
					<input
						id="expires"
						type="number"
						bind:value={expiresInHours}
						placeholder="Never expires"
						min="1"
						disabled={sharing}
					/>
				</div>

				{#if error}
					<p class="error-text">{error}</p>
				{/if}
			</div>

			<div class="dialog-actions">
				<button class="secondary-btn" onclick={onclose} disabled={sharing}>{t('common.cancel')}</button>
				<button class="primary-btn" onclick={handleShare} disabled={sharing}>
					{#if sharing}{t('session.sharing')}{:else}{t('session.share')}{/if}
				</button>
			</div>
		{/if}
	</div>
</div>

<style>
	.share-overlay {
		position: fixed;
		inset: 0;
		background: rgba(0, 0, 0, 0.6);
		backdrop-filter: blur(4px);
		display: flex;
		align-items: center;
		justify-content: center;
		z-index: 1000;
	}

	.share-dialog {
		background: var(--color-bg-secondary);
		border: 1px solid var(--color-border);
		border-radius: 12px;
		width: 100%;
		max-width: 480px;
		max-height: 90vh;
		overflow: auto;
	}

	.dialog-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 16px 20px;
		border-bottom: 1px solid var(--color-border);
	}

	.dialog-header h3 {
		margin: 0;
		font-size: 1rem;
		font-weight: 600;
	}

	.close-btn {
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
		transition: background-color 0.15s, color 0.15s;
	}

	.close-btn:hover {
		background: rgba(255, 255, 255, 0.1);
		color: var(--color-text-primary);
	}

	.dialog-content {
		padding: 20px;
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.share-info {
		margin: 0;
		font-size: 0.875rem;
		color: var(--color-text-secondary);
	}

	.share-info strong {
		color: var(--color-text-primary);
	}

	.my-info {
		background: rgba(255, 255, 255, 0.03);
		border: 1px solid var(--color-border);
		border-radius: 8px;
		padding: 12px;
	}

	.info-label {
		margin: 0 0 8px;
		font-size: 0.75rem;
		color: var(--color-text-secondary);
	}

	.info-row {
		display: flex;
		align-items: center;
		gap: 8px;
		margin-bottom: 6px;
	}

	.info-row:last-child {
		margin-bottom: 0;
	}

	.info-key {
		font-size: 0.75rem;
		color: var(--color-text-secondary);
		flex-shrink: 0;
	}

	.info-value {
		flex: 1;
		font-size: 0.6875rem;
		font-family: var(--font-mono);
		background: rgba(0, 0, 0, 0.2);
		padding: 4px 8px;
		border-radius: 4px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.copy-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 24px;
		height: 24px;
		border: none;
		border-radius: 4px;
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
		flex-shrink: 0;
	}

	.copy-btn:hover {
		background: rgba(255, 255, 255, 0.1);
		color: var(--color-text-primary);
	}

	.form-group {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.form-group label {
		font-size: 0.75rem;
		font-weight: 500;
		color: var(--color-text-secondary);
	}

	.form-group input {
		padding: 10px 12px;
		border: 1px solid var(--color-border);
		border-radius: 8px;
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		font-size: 0.875rem;
		font-family: var(--font-mono);
	}

	.form-group input:focus {
		outline: none;
		border-color: var(--color-accent);
	}

	.form-group input::placeholder {
		color: var(--color-text-tertiary);
	}

	.error-text {
		margin: 0;
		font-size: 0.75rem;
		color: var(--color-danger);
	}

	.dialog-actions {
		display: flex;
		justify-content: flex-end;
		gap: 8px;
		padding: 16px 20px;
		border-top: 1px solid var(--color-border);
	}

	.primary-btn, .secondary-btn {
		padding: 8px 16px;
		border-radius: 8px;
		font-size: 0.875rem;
		font-weight: 500;
		cursor: pointer;
		transition: background-color 0.15s, opacity 0.15s;
	}

	.primary-btn {
		background: var(--color-accent);
		border: none;
		color: white;
	}

	.primary-btn:hover:not(:disabled) {
		opacity: 0.9;
	}

	.primary-btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.secondary-btn {
		background: transparent;
		border: 1px solid var(--color-border);
		color: var(--color-text-primary);
	}

	.secondary-btn:hover:not(:disabled) {
		background: rgba(255, 255, 255, 0.05);
	}

	.success-content {
		padding: 32px 20px;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 12px;
		text-align: center;
	}

	.success-icon {
		width: 48px;
		height: 48px;
		background: rgba(16, 185, 129, 0.1);
		border-radius: 50%;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.success-text {
		margin: 0;
		font-size: 1rem;
		font-weight: 500;
	}

	.share-id {
		margin: 0;
		font-size: 0.75rem;
		color: var(--color-text-secondary);
	}

	.share-id code {
		font-family: var(--font-mono);
		background: rgba(0, 0, 0, 0.2);
		padding: 2px 6px;
		border-radius: 4px;
	}

	.share-hint {
		margin: 0;
		font-size: 0.75rem;
		color: var(--color-text-tertiary);
	}
</style>
