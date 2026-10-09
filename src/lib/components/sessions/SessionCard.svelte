<script lang="ts">
	import type { SessionConfig } from '$lib/ipc/sessions';
	import type { VaultInfo } from '$lib/state/vault.svelte';
	import DistroIcon from './DistroIcon.svelte';
	import { t } from '$lib/state/i18n.svelte';

	interface Props {
		session: SessionConfig;
		/** Named on the card when the list spans vaults, so a session says
		 *  where it lives instead of the person having to remember. */
		vault?: VaultInfo | null;
		onconnect: () => void;
		onedit: () => void;
		ondelete: () => void;
		oncontextmenu?: (e: MouseEvent) => void;
		ondragstart?: (e: PointerEvent) => void;
		ondragend?: () => void;
		/** Selecting several: a click selects instead of connecting. */
		selecting?: boolean;
		selected?: boolean;
		/** Ctrl/Cmd or Shift click, or any click while selecting. */
		onselect?: (e: MouseEvent) => void;
	}

	let { session, vault = null, onconnect, onedit, ondelete, oncontextmenu, ondragstart, ondragend, selecting = false, selected = false, onselect }: Props = $props();

	function onMain(e: MouseEvent): void {
		if (onselect && (selecting || e.ctrlKey || e.metaKey || e.shiftKey)) {
			e.preventDefault();
			onselect(e);
		} else {
			onconnect();
		}
	}

	let authLabel = $derived(
		session.kind === 'wsl' ? 'WSL' :
		session.auth_method.type === 'Password' ? t('session.auth_pw_label') :
		session.auth_method.type === 'Key' ? t('session.auth_key') : t('session.auth_agent')
	);
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="session-card" class:selected oncontextmenu={oncontextmenu} role="group">
	{#if selecting}
		<input
			class="select-box"
			type="checkbox"
			checked={selected}
			aria-label={t('session.select_name', { name: session.name })}
			onclick={(e) => { e.stopPropagation(); onselect?.(e); }}
		/>
	{/if}
	<span
		class="drag-handle"
		role="img"
		aria-label="Drag to reorder"
		onpointerdown={(e) => { if (ondragstart) { e.stopPropagation(); ondragstart(e); } }}
	>
		<svg width="8" height="12" viewBox="0 0 8 12" fill="currentColor">
			<circle cx="2" cy="2" r="1.2"/><circle cx="6" cy="2" r="1.2"/>
			<circle cx="2" cy="6" r="1.2"/><circle cx="6" cy="6" r="1.2"/>
			<circle cx="2" cy="10" r="1.2"/><circle cx="6" cy="10" r="1.2"/>
		</svg>
	</span>
	<button class="card-main" onclick={onMain} aria-pressed={selecting ? selected : undefined} title={selecting ? t('session.select_name', { name: session.name }) : t('session.connect_to', { name: session.name })}>
		<DistroIcon osId={session.detected_os} size={18} />
		<div class="session-info">
			<span class="session-name-row">
				<span class="session-name">{session.name}</span>
				{#if vault}
					<span class="vault-chip" class:shared={vault.vaultType === 'shared'} title={vault.name}>
						{#if vault.vaultType === 'shared'}
							<svg width="9" height="9" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
								<path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/><path d="M23 21v-2a4 4 0 0 0-3-3.87"/><path d="M16 3.13a4 4 0 0 1 0 7.75"/>
							</svg>
						{:else}
							<svg width="9" height="9" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round">
								<rect x="3" y="11" width="18" height="11" rx="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/>
							</svg>
						{/if}
						<span class="vault-chip-name">{vault.name}</span>
					</span>
				{/if}
			</span>
			{#if session.kind === 'wsl'}
				<span class="session-detail">{session.username ? `${session.username} · ` : ''}{session.wsl_distro || 'WSL'}</span>
			{:else}
				<span class="session-detail">{session.username ? `${session.username}@` : ''}{session.host}:{session.port}</span>
			{/if}
		</div>
		<span class="auth-badge" title={t('session.auth_type', { type: session.auth_method.type })}>{authLabel}</span>
	</button>

	<div class="session-actions">
		<button class="action-btn connect-btn" onclick={onconnect} title={t('session.connect')} aria-label={t('session.connect_to', { name: session.name })}>
			<svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor">
				<path d="M8 5v14l11-7z"/>
			</svg>
		</button>
		<button class="action-btn edit-btn" onclick={onedit} title={t('session.edit')} aria-label={t('session.edit_name', { name: session.name })}>
			<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
				<path d="M17 3a2.85 2.85 0 0 1 4 4L7.5 20.5 2 22l1.5-5.5Z"/>
			</svg>
		</button>
		<button class="action-btn delete-btn" onclick={ondelete} title={t('session.delete')} aria-label={t('session.delete_name', { name: session.name })}>
			<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
				<path d="M3 6h18"/>
				<path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"/>
				<path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"/>
			</svg>
		</button>
	</div>
</div>

<style>
	.drag-handle {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 16px;
		height: 24px;
		flex-shrink: 0;
		color: var(--color-text-secondary);
		opacity: 0.15;
		cursor: grab;
		touch-action: none;
		transition: opacity 0.15s ease;
	}

	.drag-handle:active {
		cursor: grabbing;
		opacity: 0.6;
	}

	.session-card:hover .drag-handle {
		opacity: 0.4;
	}

	.session-card {
		position: relative;
		display: flex;
		align-items: center;
		gap: 3px;
		padding: 4px 6px;
		border-radius: 6px;
		transition: background-color var(--duration-default) var(--ease-default);
	}

	.session-card:hover {
		background-color: var(--color-surface-hover);
	}

	.session-card.selected,
	.session-card.selected:hover {
		background-color: color-mix(in srgb, var(--color-accent) 18%, transparent);
	}

	.select-box {
		flex-shrink: 0;
		width: 15px;
		height: 15px;
		margin: 0 4px 0 2px;
		accent-color: var(--color-accent);
		cursor: pointer;
	}

	.card-main {
		flex: 1;
		min-width: 0;
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 0;
		border: none;
		background: transparent;
		color: inherit;
		cursor: pointer;
		text-align: left;
		font-family: var(--font-sans);
	}

	.session-info {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	/* Beside the name when the sidebar is wide enough, under it when not.
	   Neither the name nor the vault is ever cut to make room for the other:
	   "k8s-c…" beside "Kubernetes …" tells you nothing about either. */
	.session-name-row {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 2px 6px;
		min-width: 0;
	}

	/* Where the session lives, when the list spans vaults. Same box as the
	   auth badge so the card has one vocabulary for "a fact about this
	   session". */
	.vault-chip {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		flex: 0 0 auto;
		max-width: 100%;
		min-width: 0;
		padding: 1px 6px;
		font-size: var(--text-2xs);
		font-weight: 500;
		color: var(--color-text-secondary);
		background-color: var(--color-surface-hover);
		border-radius: 4px;
	}

	.vault-chip.shared {
		color: #10b981;
		background-color: rgba(16, 185, 129, 0.12);
	}

	.vault-chip-name {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.session-name {
		flex: 0 1 auto;
		min-width: 0;
		font-size: var(--text-sm);
		font-weight: 500;
		color: var(--color-text-primary);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.session-detail {
		font-size: var(--text-2xs);
		color: var(--color-text-secondary);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
		font-family: var(--font-mono, monospace);
	}

	.auth-badge {
		flex-shrink: 0;
		padding: 2px 6px;
		font-size: var(--text-2xs);
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.04em;
		color: var(--color-text-secondary);
		background-color: var(--color-surface-hover);
		border-radius: 4px;
	}

	/* These buttons were already meant to be hover-only, but `opacity: 0` hides
	   them without giving their width back — they kept reserving ~76px of a
	   ~209px row, which is why session names rendered as "prod…" even on a wide
	   monitor. Taking them out of flow lets the name use the full row at rest
	   and the buttons fade in over the right edge on hover. */
	.session-actions {
		position: absolute;
		right: 6px;
		top: 50%;
		transform: translateY(-50%);
		display: flex;
		align-items: center;
		gap: 2px;
		padding-left: 20px;
		opacity: 0;
		pointer-events: none;
		/* Fade the name out behind the buttons rather than letting the two overlap. */
		background: linear-gradient(
			to right,
			transparent 0,
			var(--color-bg-secondary) 20px
		);
		transition: opacity var(--duration-default) var(--ease-default);
	}

	.session-card:hover .session-actions,
	.session-card:focus-within .session-actions {
		opacity: 1;
		pointer-events: auto;
	}

	/* No hover on touch, so the actions have to stay put and visible. */
	@media (pointer: coarse) {
		.session-actions {
			position: static;
			transform: none;
			padding-left: 0;
			background: none;
			opacity: 1;
			pointer-events: auto;
		}
	}

	.action-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 24px;
		height: 24px;
		padding: 0;
		border: none;
		border-radius: 4px;
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
		transition:
			background-color var(--duration-default) var(--ease-default),
			color var(--duration-default) var(--ease-default);
	}

	.action-btn:hover {
		background-color: var(--color-surface-hover);
		color: var(--color-text-primary);
	}

	.action-btn:active {
		transform: scale(0.92);
	}

	.connect-btn:hover {
		color: var(--color-accent);
	}

	.delete-btn:hover {
		color: var(--color-danger);
	}

	/* A phone: the row is the button. Tapping it connects; a long press opens
	   the same menu a right-click does, with Edit and Delete. The drag handle,
	   the three buttons and the auth badge would otherwise leave the name two
	   letters wide ("Pro…"), and a delete button under a thumb is one slip from
	   losing a session. */
	@media (max-width: 700px) {
		.drag-handle,
		.session-actions,
		.auth-badge {
			display: none;
		}

		.session-card {
			padding-left: 4px;
		}
	}
</style>
