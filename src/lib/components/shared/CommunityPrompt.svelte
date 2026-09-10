<!--
	A one-time nudge towards the Discord community.

	The design constraint is that this is an advert for our own server, shown
	to someone who opened a terminal client to do a job. So it holds itself
	back until the third launch, "Not now" defers it for three weeks rather
	than asking again tomorrow, and the checkbox ends it permanently. Joining
	also ends it — someone who clicked through does not need asking again.

	Dismissing never removes the route: Settings -> General keeps a permanent
	link, so this can be closed forever without losing anything.
-->
<script lang="ts">
	import { open as shellOpen } from '@tauri-apps/plugin-shell';
	import Modal from './Modal.svelte';
	import Button from './Button.svelte';
	import DiscordIcon from './DiscordIcon.svelte';
	import { t } from '$lib/state/i18n.svelte';
	import { COMMUNITY } from '$lib/data/community';
	import {
		shouldShowCommunityPrompt,
		snoozeCommunityPrompt,
		dismissCommunityPrompt
	} from '$lib/state/settings.svelte';

	let open = $state(false);
	let neverAgain = $state(false);

	// Checked once, on mount. Re-evaluating reactively would make the dialog
	// reappear the instant the underlying settings change.
	$effect(() => {
		open = shouldShowCommunityPrompt();
	});

	function close(): void {
		if (neverAgain) {
			dismissCommunityPrompt();
		} else {
			snoozeCommunityPrompt();
		}
		open = false;
	}

	async function join(): Promise<void> {
		// Joining answers the question for good, whatever the checkbox says.
		dismissCommunityPrompt();
		open = false;
		await shellOpen(COMMUNITY.discordInvite);
	}
</script>

<Modal {open} onclose={close} maxWidth="440px" zIndex={90}>
	<div class="community">
		<div class="mark">
			<DiscordIcon size={28} brand />
		</div>

		<h2>{t('community.title')}</h2>
		<p class="lede">{t('community.body')}</p>

		<ul class="points">
			<li>{t('community.point_help')}</li>
			<li>{t('community.point_releases')}</li>
			<li>{t('community.point_build')}</li>
		</ul>

		<label class="never">
			<input type="checkbox" bind:checked={neverAgain} />
			<span>{t('community.never_again')}</span>
		</label>
	</div>

	{#snippet actions()}
		<Button variant="secondary" onclick={close}>{t('community.not_now')}</Button>
		<Button variant="primary" onclick={join}>{t('community.join')}</Button>
	{/snippet}
</Modal>

<style>
	.community {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--space-3);
		text-align: center;
	}

	.mark {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 52px;
		height: 52px;
		border-radius: var(--radius-card);
		background: color-mix(in srgb, #5865f2 14%, transparent);
	}

	h2 {
		margin: 0;
		font-size: var(--text-lg);
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.lede {
		margin: 0;
		max-width: 34ch;
		font-size: var(--text-sm);
		line-height: 1.5;
		color: var(--color-text-secondary);
	}

	.points {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		margin: 0;
		padding: 0;
		list-style: none;
		text-align: left;
		font-size: var(--text-sm);
		color: var(--color-text-secondary);
	}

	.points li {
		position: relative;
		padding-left: 18px;
	}

	.points li::before {
		content: '';
		position: absolute;
		left: 4px;
		top: 0.55em;
		width: 4px;
		height: 4px;
		border-radius: 50%;
		background: var(--color-accent);
	}

	.never {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		margin-top: var(--space-1);
		font-size: var(--text-xs);
		color: var(--color-text-tertiary);
		cursor: pointer;
		user-select: none;
	}

	.never input {
		cursor: pointer;
		accent-color: var(--color-accent);
	}

	.never:hover span {
		color: var(--color-text-secondary);
	}
</style>
