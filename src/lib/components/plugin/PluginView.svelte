<script lang="ts">
	import PluginUIRenderer from './PluginUIRenderer.svelte';
	import { pluginGetUi, pluginCallAction, type PluginUiState } from '$lib/ipc/plugin';
	import { getPluginUiStates, setPluginUiState } from '$lib/state/plugin.svelte';
	import { addToast } from '$lib/state/toasts.svelte';
	import { t } from '$lib/state/i18n.svelte';
	import { listen, type UnlistenFn } from '@tauri-apps/api/event';
	import { untrack } from 'svelte';

	interface Props {
		pluginId: string;
		connectionId: string | undefined;
	}

	let { pluginId, connectionId }: Props = $props();

	let uiState: PluginUiState | undefined = $derived(getPluginUiStates().get(pluginId));

	$effect(() => {
		const id = pluginId;
		let unlisten: UnlistenFn | undefined;

		untrack(() => {
			pluginGetUi(id)
				.then((state) => {
					if (state) {
						setPluginUiState(id, state);
					}
				})
				.catch((err) => {
					addToast(String(err), 'error');
				});
		});

		listen<PluginUiState>('plugin-ui-update', (event) => {
			if (event.payload.pluginId === id) {
				setPluginUiState(id, event.payload);
			}
		}).then((fn) => {
			unlisten = fn;
		});

		return () => {
			unlisten?.();
		};
	});

	async function handleAction(action: string): Promise<void> {
		try {
			const result = await pluginCallAction(pluginId, action, {});
			if (result) {
				setPluginUiState(pluginId, result);
			}
		} catch (err) {
			addToast(String(err), 'error');
		}
	}
</script>

<div class="plugin-view">
	{#if uiState}
		<PluginUIRenderer {uiState} onAction={handleAction} />
	{:else}
		<p class="no-ui">{t('plugin.noUi')}</p>
	{/if}
</div>

<style>
	.plugin-view {
		padding: 12px;
		overflow-y: auto;
		height: 100%;
	}

	.no-ui {
		color: var(--color-text-muted);
		font-size: 0.8125rem;
		margin: 0;
	}
</style>
