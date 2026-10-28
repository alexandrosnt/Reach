<script lang="ts">
	/** No cluster yet: what this page does, one button, and where kubeconfigs live. */
	import FaIcon from '#lib/components/shared/FaIcon.svelte';
	import { faAnchor, faPlus } from '@fortawesome/free-solid-svg-icons';
	import { t } from '#lib/state/i18n.svelte.js';

	let { onadd }: { onadd: () => void } = $props();
</script>

<div class="welcome">
	<div class="card">
		<FaIcon icon={faAnchor} size={30} />
		<h2>{t('k8s.welcome_title')}</h2>
		<p>{t('k8s.welcome_body')}</p>
		<ul>
			<li>{t('k8s.welcome_point_health')}</li>
			<li>{t('k8s.welcome_point_logs')}</li>
			<li>{t('k8s.welcome_point_helm')}</li>
			<li>{t('k8s.welcome_point_safe')}</li>
		</ul>
		<button type="button" class="big" onclick={onadd}><FaIcon icon={faPlus} /> {t('k8s.add_first')}</button>
		<details>
			<summary>{t('k8s.where_kubeconfig')}</summary>
			<dl>
				<dt>{t('k8s.kc_usual')}</dt>
				<dd><code>~/.kube/config</code></dd>
				<dt>k3s</dt>
				<dd><code>/etc/rancher/k3s/k3s.yaml</code></dd>
				<dt>EKS · GKE · AKS</dt>
				<dd>
					{t('k8s.kc_cloud')}
					<code>aws eks update-kubeconfig</code>
					<code>gcloud container clusters get-credentials</code>
					<code>az aks get-credentials</code>
				</dd>
			</dl>
		</details>
	</div>
</div>

<style>
	.welcome {
		display: flex;
		align-items: center;
		justify-content: center;
		height: 100%;
		padding: 24px 16px;
		overflow: auto;
	}

	.card {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 12px;
		width: min(560px, 100%);
		padding: 28px 24px;
		border-radius: 12px;
		border: 1px solid var(--color-border);
		background: var(--color-bg-elevated);
		color: var(--color-text-secondary);
		text-align: center;
	}

	.card > :global(svg) {
		color: var(--color-accent);
	}

	h2 {
		margin: 0;
		font-size: 18px;
		font-weight: 600;
		color: var(--color-text-primary);
	}

	p {
		margin: 0;
		font-size: var(--text-sm);
		line-height: 1.5;
	}

	ul {
		margin: 0;
		padding: 0 0 0 18px;
		text-align: left;
		font-size: var(--text-sm);
		line-height: 1.7;
		color: var(--color-text-primary);
	}

	.big {
		display: inline-flex;
		align-items: center;
		gap: 8px;
		min-height: 44px;
		margin-top: 4px;
		padding: 0 20px;
		border-radius: var(--radius-btn);
		border: 1px solid var(--color-accent);
		background: var(--color-accent);
		color: #fff;
		font: inherit;
		font-size: var(--text-sm);
		font-weight: 600;
		cursor: pointer;
	}

	.big:focus-visible {
		outline: 2px solid var(--color-accent);
		outline-offset: 2px;
	}

	details {
		width: 100%;
		text-align: left;
		font-size: var(--text-sm);
	}

	summary {
		cursor: pointer;
		color: var(--color-text-secondary);
		text-align: center;
		padding: 6px 0;
	}

	dl {
		display: grid;
		/* minmax(0, 1fr): a long command wraps instead of widening the list
		   past a phone's screen. */
		grid-template-columns: max-content minmax(0, 1fr);
		gap: 6px 12px;
		margin: 8px 0 0;
	}

	/* One column on a narrow screen, each name above its paths. */
	@media (max-width: 480px) {
		dl {
			grid-template-columns: minmax(0, 1fr);
		}
	}

	dt {
		color: var(--color-text-secondary);
	}

	dd {
		margin: 0;
		display: flex;
		flex-direction: column;
		gap: 4px;
		color: var(--color-text-primary);
	}

	code {
		font-family: var(--font-mono);
		font-size: var(--text-xs);
		overflow-wrap: anywhere;
	}
</style>
