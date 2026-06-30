<script lang="ts">
	import Input from '$lib/components/shared/Input.svelte';
	import { inspectKeyFile, type KeyFileInfo, type KeyCandidate } from '$lib/ipc/ssh';
	import { t } from '$lib/state/i18n.svelte';

	interface Props {
		value?: string;
		label?: string;
		placeholder?: string;
		disabled?: boolean;
	}

	let {
		value = $bindable(''),
		label = '',
		placeholder = '~/.ssh/id_ed25519',
		disabled = false
	}: Props = $props();

	let info = $state<KeyFileInfo | undefined>();
	let timer: ReturnType<typeof setTimeout> | undefined;

	// Inspect the path (debounced) whenever it changes. Stale responses are
	// dropped so a fast typist never sees a result for an old path.
	$effect(() => {
		const path = value.trim();
		if (timer) clearTimeout(timer);
		if (!path) {
			info = undefined;
			return;
		}
		timer = setTimeout(async () => {
			try {
				const result = await inspectKeyFile(path);
				if (path === value.trim()) info = result;
			} catch {
				// Inspection is best-effort UX sugar — never block the form on it.
			}
		}, 350);
		return () => {
			if (timer) clearTimeout(timer);
		};
	});

	function pick(c: KeyCandidate): void {
		value = c.path;
	}

	const HINT_KINDS = ['public_key', 'not_found', 'not_a_key'];

	let suggestions = $derived<KeyCandidate[]>(
		info && HINT_KINDS.includes(info.kind)
			? [...(info.suggestedPrivateKey ? [info.suggestedPrivateKey] : []), ...info.siblingPrivateKeys]
			: []
	);

	let isPublic = $derived(info?.kind === 'public_key');
	let isNotFound = $derived(info?.kind === 'not_found' && value.trim().length > 0);
	let isNotKey = $derived(info?.kind === 'not_a_key');
	let isValid = $derived(info?.kind === 'private_key');
</script>

<div class="key-input">
	<Input {label} bind:value {placeholder} {disabled} />

	{#if isPublic}
		<div class="key-note warning">
			<span class="icon">⚠</span>
			<span>
				{t('session.key_public_warning')}{info?.algo ? ` (${info.algo})` : ''}
			</span>
		</div>
	{:else if isNotFound}
		<div class="key-note muted">{t('session.key_not_found')}</div>
	{:else if isNotKey}
		<div class="key-note warning">
			<span class="icon">⚠</span>
			<span>{t('session.key_not_recognized')}</span>
		</div>
	{:else if isValid}
		<div class="key-note ok">
			<span class="icon">✓</span>
			<span>{t('session.key_valid')}{info?.algo ? ` · ${info.algo}` : ''}</span>
			{#if info?.encrypted}
				<span class="enc">· {t('session.key_encrypted')}</span>
			{/if}
		</div>
	{/if}

	{#if suggestions.length > 0}
		<div class="suggest">
			<span class="suggest-label">{t('session.key_folder_keys')}</span>
			<div class="chips">
				{#each suggestions as cand (cand.path)}
					<button
						type="button"
						class="chip"
						class:primary={cand.path === info?.suggestedPrivateKey?.path}
						{disabled}
						title={cand.path}
						onclick={() => pick(cand)}
					>
						{cand.name}{cand.algo ? ` · ${cand.algo}` : ''}
					</button>
				{/each}
			</div>
		</div>
	{/if}
</div>

<style>
	.key-input {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.key-note {
		display: flex;
		align-items: flex-start;
		gap: 6px;
		font-size: 0.75rem;
		line-height: 1.35;
	}

	.key-note .icon {
		flex-shrink: 0;
	}

	.key-note.warning {
		color: var(--color-danger);
	}

	.key-note.ok {
		color: var(--color-success, #30d158);
	}

	.key-note.muted {
		color: var(--color-text-secondary);
		opacity: 0.8;
	}

	.key-note .enc {
		color: var(--color-text-secondary);
	}

	.suggest {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.suggest-label {
		font-size: 0.6875rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--color-text-secondary);
	}

	.chips {
		display: flex;
		flex-wrap: wrap;
		gap: 6px;
	}

	.chip {
		padding: 4px 10px;
		font-family: var(--font-mono, monospace);
		font-size: 0.75rem;
		color: var(--color-text-primary);
		background-color: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		cursor: pointer;
		transition:
			border-color var(--duration-default) var(--ease-default),
			background-color var(--duration-default) var(--ease-default);
	}

	.chip:hover:not(:disabled) {
		border-color: var(--color-accent);
		background-color: rgba(255, 255, 255, 0.04);
	}

	.chip.primary {
		border-color: var(--color-accent);
		color: var(--color-accent);
	}

	.chip:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}
</style>
