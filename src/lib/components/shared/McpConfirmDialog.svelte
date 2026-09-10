<script lang="ts">
	/**
	 * The human decision point. Nothing an AI proposes reaches a shell without
	 * passing through here.
	 *
	 * Design rules, all of them load-bearing:
	 *
	 * - **The command is the largest thing on screen**, verbatim and monospaced.
	 *   Everything else is context for judging it.
	 * - **Reject is the default focus**, and there is no auto-approve. A dialog
	 *   that trains you to hit Enter is worse than no dialog, because it
	 *   launders an unread command into an approved one.
	 * - **Closing without answering is a rejection.** The backend treats a
	 *   timeout as refusal; the UI must not imply anything softer.
	 * - Danger is a colour *and* a word. Colour alone fails for anyone who
	 *   cannot distinguish them, on the one dialog where that matters most.
	 */
	import { listen, type UnlistenFn } from '@tauri-apps/api/event';
	import { onMount } from 'svelte';
	import Button from './Button.svelte';
	import { mcpConfirmResponse, type McpConfirmRequest } from '$lib/ipc/mcp';
	import { t } from '$lib/state/i18n.svelte';

	let queue = $state<McpConfirmRequest[]>([]);
	let current = $derived(queue[0] ?? null);
	// Wrapper rather than the button itself: Button renders its own <button>,
	// so focus is taken on the real element inside.
	let rejectWrap: HTMLDivElement | undefined = $state();

	onMount(() => {
		let unlisten: UnlistenFn | undefined;
		listen<McpConfirmRequest>('mcp-confirm-request', (e) => {
			// Queued rather than replaced: two agents, or one agent racing
			// itself, must not silently drop a request the user never saw.
			queue = [...queue, e.payload];
		})
			.then((fn) => (unlisten = fn))
			.catch(() => {});
		return () => unlisten?.();
	});

	// Focus lands on Reject whenever a new request surfaces.
	$effect(() => {
		if (current) queueMicrotask(() => rejectWrap?.querySelector('button')?.focus());
	});

	async function answer(approved: boolean): Promise<void> {
		const req = current;
		if (!req) return;
		queue = queue.slice(1);
		try {
			await mcpConfirmResponse(req.promptId, approved);
		} catch {
			// The backend fails closed on its own timeout, so a lost reply is
			// still a refusal. Nothing to recover.
		}
	}

	function onKeydown(e: KeyboardEvent): void {
		if (!current) return;
		if (e.key === 'Escape') {
			e.stopPropagation();
			void answer(false);
		}
	}

	let severity = $derived((current?.danger ?? 'Benign').toLowerCase());
</script>

<svelte:window on:keydown={onKeydown} />

{#if current}
	<div class="backdrop" role="dialog" aria-modal="true" aria-labelledby="mcp-confirm-title">
		<div class="dialog">
			<div class="head">
				<span class="badge {severity}">{current.danger}</span>
				<h2 id="mcp-confirm-title" class="title">{t('mcp.confirm_title')}</h2>
				<p class="sub">
					{t('mcp.confirm_subtitle', { agent: current.agentName, host: current.host })}
				</p>
			</div>

			{#if current.dangerReason}
				<p class="danger-reason {severity}">{current.dangerReason}</p>
			{/if}

			<!-- The command, verbatim. Wraps rather than truncating: an ellipsis
			     here would hide the part that matters. -->
			<pre class="command">{current.command}</pre>

			<div class="field">
				<span class="label">{t('mcp.confirm_rationale')}</span>
				<p class="value">{current.rationale}</p>
			</div>

			{#if current.references.length > 0}
				<div class="field">
					<span class="label">{t('mcp.confirm_references')}</span>
					<ul class="refs">
						{#each current.references as ref (ref)}
							<li>{ref}</li>
						{/each}
					</ul>
				</div>
			{/if}

			{#if current.rollback}
				<div class="field">
					<span class="label">{t('mcp.confirm_rollback')}</span>
					<p class="value mono">{current.rollback}</p>
				</div>
			{/if}

			<p class="notice">{t('mcp.confirm_notice')}</p>

			<div class="actions">
				<!-- Reject first in the DOM and focused: the safe answer should be
				     the one a hurried person gives. -->
				<div bind:this={rejectWrap}>
					<Button variant="secondary" size="md" onclick={() => answer(false)}>
						{t('mcp.confirm_reject')}
					</Button>
				</div>
				<Button variant={severity === 'destructive' ? 'danger' : 'primary'} size="md" onclick={() => answer(true)}>
					{t('mcp.confirm_approve')}
				</Button>
			</div>

			{#if queue.length > 1}
				<p class="queued">{t('mcp.confirm_queued', { count: queue.length - 1 })}</p>
			{/if}
		</div>
	</div>
{/if}

<style>
	.backdrop {
		position: fixed;
		inset: 0;
		z-index: 300;
		display: flex;
		align-items: center;
		justify-content: center;
		padding: var(--space-4);
		background: var(--color-surface-sunken);
		backdrop-filter: blur(16px);
		-webkit-backdrop-filter: blur(16px);
	}

	.dialog {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
		width: 100%;
		max-width: 560px;
		max-height: calc(100dvh - 2 * var(--space-4));
		overflow-y: auto;
		padding: var(--space-5);
		background: var(--color-bg-secondary);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-modal);
		box-shadow: var(--shadow-elevated);
	}

	.head {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
	}

	.badge {
		align-self: flex-start;
		padding: 2px 8px;
		font-size: var(--text-2xs);
		font-weight: 700;
		text-transform: uppercase;
		letter-spacing: 0.04em;
		border-radius: var(--radius-sm);
		color: var(--color-bg-primary);
		background: var(--color-text-secondary);
	}

	.badge.sensitive { background: var(--color-warning); }
	.badge.destructive { background: var(--color-danger); color: #fff; }

	.title {
		margin: 0;
		font-size: var(--text-lg);
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.sub {
		margin: 0;
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
	}

	.danger-reason {
		margin: 0;
		padding: var(--space-2) var(--space-3);
		font-size: var(--text-xs);
		border-radius: var(--radius-sm);
		color: var(--color-text-primary);
		background: var(--color-surface-hover);
		border-left: 3px solid var(--color-text-secondary);
	}

	.danger-reason.sensitive { border-left-color: var(--color-warning); }
	.danger-reason.destructive { border-left-color: var(--color-danger); }

	.command {
		margin: 0;
		padding: var(--space-3);
		font-family: var(--font-mono);
		font-size: var(--text-md);
		line-height: 1.5;
		color: var(--color-text-primary);
		background: var(--color-surface-sunken);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-sm);
		/* Wrap, never truncate: an ellipsis would hide the dangerous half. */
		white-space: pre-wrap;
		overflow-wrap: anywhere;
	}

	.field {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.label {
		font-size: var(--text-2xs);
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.04em;
		color: var(--color-text-tertiary);
	}

	.value {
		margin: 0;
		font-size: var(--text-sm);
		color: var(--color-text-primary);
	}

	.value.mono {
		font-family: var(--font-mono);
		font-size: var(--text-xs);
	}

	.refs {
		margin: 0;
		padding-left: var(--space-4);
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
	}

	.notice {
		margin: 0;
		font-size: var(--text-2xs);
		color: var(--color-text-tertiary);
	}

	.actions {
		display: flex;
		justify-content: flex-end;
		gap: var(--space-2);
		margin-top: var(--space-1);
	}

	.queued {
		margin: 0;
		text-align: right;
		font-size: var(--text-2xs);
		color: var(--color-text-tertiary);
	}
</style>
