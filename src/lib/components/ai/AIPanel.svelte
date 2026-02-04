<script lang="ts">
	import { getAIChatState, sendMessage, executeCommand, closeAIPanel, clearChat, type TerminalInfo } from '$lib/state/ai-chat.svelte';
	import { getAISettings } from '$lib/state/ai.svelte';
	import { getSessionList } from '$lib/state/sessions.svelte';
	import { readTerminalBuffer } from '$lib/state/terminal-buffer.svelte';
	import { sshSend } from '$lib/ipc/ssh';
	import { ptyWrite } from '$lib/ipc/pty';

	interface Props {
		connectionId?: string;
		activeTabId?: string;
		activeTabType?: 'local' | 'ssh';
	}

	let { connectionId, activeTabId, activeTabType }: Props = $props();

	let chatState = $derived(getAIChatState());
	let aiSettings = $derived(getAISettings());
	let isConfigured = $derived(aiSettings.enabled && aiSettings.apiKey && aiSettings.selectedModel);

	let inputValue = $state('');
	let messagesContainer: HTMLDivElement | undefined = $state();

	$effect(() => {
		if (chatState.messages.length && messagesContainer) {
			messagesContainer.scrollTop = messagesContainer.scrollHeight;
		}
	});

	$effect(() => {
		const el = messagesContainer;
		if (!el) return;
		const handler = (e: MouseEvent) => {
			const target = e.target as HTMLElement;
			if (target.classList.contains('copy-btn')) {
				const code = decodeURIComponent(target.dataset.code ?? '');
				navigator.clipboard.writeText(code);
			} else if (target.classList.contains('run-btn')) {
				const command = decodeURIComponent(target.dataset.code ?? '');
				const bufferId = activeTabType === 'ssh' && connectionId ? connectionId : activeTabId;
				if (bufferId) {
					let ctx: { host?: string; username?: string; os?: string } | undefined;
					if (connectionId) {
						const sessions = getSessionList();
						const session = sessions.find((s) => s.id === connectionId);
						if (session) {
							ctx = { host: session.host, username: session.username, os: session.detected_os ?? undefined };
						}
					}
					executeCommand(command, bufferId, activeTabType ?? 'local', connectionId, ctx);
				} else {
					// No terminal, just copy to clipboard
					navigator.clipboard.writeText(command);
				}
			}
		};
		el.addEventListener('click', handler);
		return () => el.removeEventListener('click', handler);
	});

	function handleSend(): void {
		const text = inputValue.trim();
		if (!text || chatState.loading) return;
		inputValue = '';

		let context: { connectionId?: string; host?: string; username?: string; os?: string; terminalOutput?: string } | undefined;
		let terminal: TerminalInfo | undefined;

		if (connectionId || activeTabId) {
			context = { connectionId };

			// Resolve session details if we have a connectionId
			if (connectionId) {
				const sessions = getSessionList();
				const session = sessions.find((s) => s.id === connectionId);
				if (session) {
					context.host = session.host;
					context.username = session.username;
					context.os = session.detected_os ?? undefined;
				}
			}

			// Read terminal buffer
			const bufferId = activeTabType === 'ssh' && connectionId ? connectionId : activeTabId;
			if (bufferId) {
				context.terminalOutput = readTerminalBuffer(bufferId);

				// Provide terminal info for auto-execution
				terminal = {
					bufferId,
					tabType: activeTabType ?? 'local',
					connectionId
				};
			}
		}

		sendMessage(text, context, terminal);
	}

	function handleKeydown(e: KeyboardEvent): void {
		if (e.key === 'Enter' && !e.shiftKey) {
			e.preventDefault();
			handleSend();
		}
	}

	function renderContent(content: string): string {
		const escaped = content
			.replace(/&/g, '&amp;')
			.replace(/</g, '&lt;')
			.replace(/>/g, '&gt;');

		const hasTerminal = !!(activeTabId || connectionId);

		return escaped.replace(
			/```(\w*)\n?([\s\S]*?)```/g,
			(_, lang, code) =>
				`<div class="code-block"><div class="code-header"><span>${lang || 'code'}</span><div class="code-actions">${hasTerminal ? `<button class="run-btn" data-code="${encodeURIComponent(code.trim())}">Run</button>` : ''}<button class="copy-btn" data-code="${encodeURIComponent(code.trim())}">Copy</button></div></div><pre><code>${code.trim()}</code></pre></div>`
		).replace(
			/`([^`]+)`/g,
			'<code class="inline-code">$1</code>'
		).replace(/\n/g, '<br>');
	}

</script>

{#if chatState.panelOpen}
	<aside class="ai-panel">
		<div class="panel-header">
			<span class="panel-title">AI Assistant</span>
			<div class="header-actions">
				<button class="icon-btn" onclick={clearChat} aria-label="Clear chat" title="Clear chat">
					<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
						<polyline points="3 6 5 6 21 6" /><path d="M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2" />
					</svg>
				</button>
				<button class="icon-btn" onclick={closeAIPanel} aria-label="Close panel" title="Close panel">
					<svg width="14" height="14" viewBox="0 0 10 10" fill="none">
						<path d="M1 1L9 9M9 1L1 9" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
					</svg>
				</button>
			</div>
		</div>

		{#if !isConfigured}
			<div class="panel-disabled">
				<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
					<path d="M12 2a10 10 0 1010 10A10 10 0 0012 2zm1 15h-2v-2h2zm0-4h-2V7h2z" />
				</svg>
				<p>AI is not configured.</p>
				<p class="hint">Go to Settings &rarr; AI to set up your API key and model.</p>
			</div>
		{:else}
			<div class="messages" bind:this={messagesContainer}>
				{#if chatState.messages.length === 0}
					<div class="empty-state">
						<p>Ask anything about your server, SSH, networking, or shell commands.</p>
					</div>
				{/if}
				{#each chatState.messages as msg (msg.id)}
					<div class="message message-{msg.role}">
						{@html renderContent(msg.content)}
						{#if msg.role === 'assistant' && !msg.content && chatState.loading}
							<span class="typing-indicator">
								<span></span><span></span><span></span>
							</span>
						{/if}
					</div>
				{/each}
				{#if chatState.error}
					<div class="message message-error">{chatState.error}</div>
				{/if}
			</div>

			<div class="input-area">
				<textarea
					class="chat-input"
					bind:value={inputValue}
					onkeydown={handleKeydown}
					placeholder="Type a message..."
					rows="1"
					disabled={chatState.loading}
				></textarea>
				<button class="send-btn" onclick={handleSend} disabled={chatState.loading || !inputValue.trim()} aria-label="Send message">
					<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
						<line x1="22" y1="2" x2="11" y2="13" /><polygon points="22 2 15 22 11 13 2 9 22 2" />
					</svg>
				</button>
			</div>
		{/if}
	</aside>
{/if}

<style>
	.ai-panel {
		width: 320px;
		min-width: 320px;
		height: 100%;
		display: flex;
		flex-direction: column;
		background-color: var(--color-bg-elevated);
		border-left: 1px solid var(--color-border);
		animation: slideInPanel 200ms var(--ease-default);
	}

	.panel-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 10px 12px;
		border-bottom: 1px solid var(--color-border);
		flex-shrink: 0;
	}

	.panel-title {
		font-size: 0.8125rem;
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.header-actions {
		display: flex;
		gap: 4px;
	}

	.icon-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 28px;
		height: 28px;
		border: none;
		border-radius: var(--radius-btn);
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
		transition: background-color 150ms ease;
	}

	.icon-btn:hover {
		background-color: rgba(255, 255, 255, 0.08);
		color: var(--color-text-primary);
	}

	.panel-disabled {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 8px;
		padding: 24px;
		color: var(--color-text-secondary);
		text-align: center;
	}

	.panel-disabled p {
		margin: 0;
		font-size: 0.8125rem;
	}

	.hint {
		opacity: 0.6;
		font-size: 0.75rem !important;
	}

	.messages {
		flex: 1;
		overflow-y: auto;
		padding: 12px;
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.empty-state {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: center;
		color: var(--color-text-secondary);
		font-size: 0.75rem;
		text-align: center;
		padding: 24px;
	}

	.empty-state p {
		margin: 0;
	}

	.message {
		padding: 8px 12px;
		border-radius: 12px;
		font-size: 0.8125rem;
		line-height: 1.5;
		word-break: break-word;
		max-width: 90%;
	}

	.message-user {
		align-self: flex-end;
		background-color: var(--color-accent);
		color: #fff;
	}

	.message-assistant {
		align-self: flex-start;
		background-color: var(--color-bg-secondary);
		color: var(--color-text-primary);
	}

	.message-error {
		align-self: center;
		background-color: rgba(255, 59, 48, 0.1);
		color: var(--color-danger);
		font-size: 0.75rem;
		text-align: center;
		max-width: 100%;
	}

	.message :global(.code-block) {
		margin: 6px 0;
		border-radius: 6px;
		overflow: hidden;
		background-color: var(--color-bg-primary);
		border: 1px solid var(--color-border);
	}

	.message :global(.code-header) {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 4px 8px;
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
		border-bottom: 1px solid var(--color-border);
	}

	.message :global(.code-actions) {
		display: flex;
		gap: 4px;
	}

	.message :global(.copy-btn) {
		border: none;
		background: transparent;
		color: var(--color-accent);
		cursor: pointer;
		font-size: 0.6875rem;
		padding: 2px 6px;
		border-radius: 4px;
	}

	.message :global(.copy-btn:hover) {
		background-color: rgba(255, 255, 255, 0.06);
	}

	.message :global(.run-btn) {
		border: none;
		background: transparent;
		color: var(--color-success, #30d158);
		cursor: pointer;
		font-size: 0.6875rem;
		padding: 2px 6px;
		border-radius: 4px;
	}

	.message :global(.run-btn:hover) {
		background-color: rgba(48, 209, 88, 0.1);
	}

	.message :global(pre) {
		margin: 0;
		padding: 8px;
		overflow-x: auto;
	}

	.message :global(code) {
		font-family: var(--font-mono, monospace);
		font-size: 0.75rem;
	}

	.message :global(.inline-code) {
		padding: 1px 4px;
		border-radius: 3px;
		background-color: var(--color-bg-primary);
		font-size: 0.75rem;
	}

	.typing-indicator {
		display: inline-flex;
		gap: 3px;
		padding: 4px 0;
	}

	.typing-indicator span {
		width: 5px;
		height: 5px;
		border-radius: 50%;
		background-color: var(--color-text-secondary);
		animation: typingBounce 1s infinite;
	}

	.typing-indicator span:nth-child(2) { animation-delay: 0.15s; }
	.typing-indicator span:nth-child(3) { animation-delay: 0.3s; }

	.input-area {
		display: flex;
		gap: 6px;
		padding: 10px 12px;
		border-top: 1px solid var(--color-border);
		flex-shrink: 0;
	}

	.chat-input {
		flex: 1;
		padding: 8px 12px;
		font-size: 0.8125rem;
		font-family: inherit;
		color: var(--color-text-primary);
		background-color: var(--color-bg-primary);
		border: 1px solid var(--color-border);
		border-radius: 16px;
		outline: none;
		resize: none;
		line-height: 1.4;
		transition: border-color 150ms ease;
	}

	.chat-input:focus {
		border-color: var(--color-accent);
	}

	.chat-input::placeholder {
		color: var(--color-text-secondary);
		opacity: 0.5;
	}

	.send-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 34px;
		height: 34px;
		border: none;
		border-radius: 50%;
		background-color: var(--color-accent);
		color: #fff;
		cursor: pointer;
		flex-shrink: 0;
		transition: opacity 150ms ease;
	}

	.send-btn:disabled {
		opacity: 0.4;
		cursor: default;
	}

	@keyframes slideInPanel {
		from { transform: translateX(100%); }
		to { transform: translateX(0); }
	}

	@keyframes typingBounce {
		0%, 60%, 100% { transform: translateY(0); }
		30% { transform: translateY(-4px); }
	}
</style>
