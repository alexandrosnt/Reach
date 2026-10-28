<script lang="ts">
	/** What a host or cluster is for, as a small label; production in red. */
	import type { Environment } from '#lib/ipc/containers.js';
	import { t } from '#lib/state/i18n.svelte.js';

	let { environment, readOnly = false }: { environment: Environment; readOnly?: boolean } = $props();
</script>

{#if environment !== 'none'}
	<span class="env {environment}">{t(`env.${environment}`)}</span>
{/if}
{#if readOnly}
	<span class="env ro" title={t('env.read_only_hint')}>{t('env.read_only')}</span>
{/if}

<style>
	.env {
		display: inline-block;
		padding: 1px 6px;
		border-radius: 4px;
		font-size: 0.6875rem;
		font-weight: 600;
		letter-spacing: 0.02em;
		text-transform: uppercase;
		white-space: nowrap;
		color: var(--tone);
		border: 1px solid color-mix(in srgb, var(--tone) 45%, transparent);
		background: color-mix(in srgb, var(--tone) 10%, transparent);
	}

	.development {
		--tone: var(--color-success);
	}
	.staging {
		--tone: var(--color-warning);
	}
	.production {
		--tone: var(--color-danger);
	}
	.ro {
		--tone: var(--color-text-secondary);
	}
</style>
