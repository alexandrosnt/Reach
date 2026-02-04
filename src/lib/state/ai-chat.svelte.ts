import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { getAISettings } from './ai.svelte';
import { readTerminalBuffer } from './terminal-buffer.svelte';
import { sshSend } from '$lib/ipc/ssh';
import { ptyWrite } from '$lib/ipc/pty';

export interface AIChatMessage {
	id: string;
	role: 'user' | 'assistant';
	content: string;
	timestamp: number;
}

let messages = $state<AIChatMessage[]>([]);
let loading = $state(false);
let error = $state<string | undefined>();
let panelOpen = $state(false);

let cancelled = false;

export function getAIChatState() {
	return { messages, loading, error, panelOpen };
}

export function toggleAIPanel(): void {
	panelOpen = !panelOpen;
}

export function openAIPanel(): void {
	panelOpen = true;
}

export function closeAIPanel(): void {
	panelOpen = false;
}

export function clearChat(): void {
	messages.length = 0;
	error = undefined;
	loading = false;
	cancelled = true;
}

function buildSystemPrompt(context?: { connectionId?: string; host?: string; username?: string; os?: string; terminalOutput?: string }): string {
	let prompt = 'You are a helpful assistant embedded in Reach, an SSH & remote server management tool. Provide concise, technical answers about Linux/Unix administration, SSH, networking, and shell scripting. When suggesting commands, use fenced code blocks so the user can run them directly.';
	if (context?.host || context?.username || context?.os) {
		prompt += '\n\nCurrent session context:';
		if (context.host) prompt += `\n- Host: ${context.host}`;
		if (context.username) prompt += `\n- User: ${context.username}`;
		if (context.os) prompt += `\n- OS: ${context.os}`;
	}
	if (context?.terminalOutput) {
		prompt += `\n\nRecent terminal output:\n\`\`\`\n${context.terminalOutput}\n\`\`\``;
	}
	return prompt;
}

async function aiInvoke(apiMessages: { role: string; content: string }[]): Promise<string> {
	const settings = getAISettings();
	if (!settings.enabled || !settings.apiKey || !settings.selectedModel) {
		throw new Error('AI is not configured.');
	}

	return invoke<string>('ai_chat', {
		request: {
			url: 'https://openrouter.ai/api/v1/chat/completions',
			apiKey: settings.apiKey,
			model: settings.selectedModel,
			messages: apiMessages
		}
	});
}

export interface TerminalInfo {
	bufferId: string;
	tabType: 'ssh' | 'local';
	connectionId?: string;
}

function extractCodeBlocks(content: string): string[] {
	const blocks: string[] = [];
	const regex = /```(?:bash|sh|shell|zsh|console|terminal)?\n([\s\S]*?)```/g;
	let match;
	while ((match = regex.exec(content)) !== null) {
		const code = match[1].trim();
		if (code) blocks.push(code);
	}
	return blocks;
}

function decodePayload(payload: unknown): string {
	if (payload instanceof Uint8Array) {
		return new TextDecoder().decode(payload);
	}
	if (Array.isArray(payload)) {
		return new TextDecoder().decode(new Uint8Array(payload));
	}
	if (typeof payload === 'string') {
		return payload;
	}
	return '';
}

// Strip ANSI escape codes for cleaner output
function stripAnsi(str: string): string {
	return str.replace(/\x1b\[[0-9;]*[a-zA-Z]/g, '').replace(/\x1b\][^\x07]*\x07/g, '').replace(/\r/g, '');
}

async function captureCommandOutput(
	command: string,
	terminal: TerminalInfo
): Promise<string> {
	const eventId = terminal.tabType === 'ssh' && terminal.connectionId
		? terminal.connectionId
		: terminal.bufferId;
	const eventName = terminal.tabType === 'ssh'
		? `ssh-data-${eventId}`
		: `pty-data-${eventId}`;

	// Start capturing data events BEFORE sending the command
	const chunks: string[] = [];
	let unlisten: UnlistenFn | undefined;

	unlisten = await listen<unknown>(eventName, (event) => {
		const text = decodePayload(event.payload);
		if (text) chunks.push(text);
	});

	// Send the command
	const encoded = Array.from(new TextEncoder().encode(command + '\n'));
	if (terminal.tabType === 'ssh' && terminal.connectionId) {
		await sshSend(terminal.connectionId, encoded);
	} else {
		await ptyWrite(terminal.bufferId, encoded);
	}

	// Wait for output to stabilize: no new data for 1 second, or max 15 seconds
	let lastLength = 0;
	let stableCount = 0;

	for (let i = 0; i < 30; i++) {
		await new Promise((r) => setTimeout(r, 500));

		const currentLength = chunks.length;
		if (currentLength > lastLength) {
			lastLength = currentLength;
			stableCount = 0;
		} else if (currentLength > 0) {
			stableCount++;
			if (stableCount >= 2) break; // Stable for 1 second
		}
	}

	// Stop listening
	unlisten?.();

	// Combine all captured data
	const rawOutput = chunks.join('');
	const cleanOutput = stripAnsi(rawOutput);

	// Extract just the command output (skip the echoed command and trailing prompt)
	const lines = cleanOutput.split('\n');
	const cmdTrimmed = command.trim();

	// Find the command echo (first line usually)
	let startIdx = 0;
	for (let i = 0; i < Math.min(lines.length, 5); i++) {
		if (lines[i].includes(cmdTrimmed)) {
			startIdx = i + 1;
			break;
		}
	}

	// Remove trailing prompt line(s) — lines containing $ or # at the end
	let endIdx = lines.length;
	while (endIdx > startIdx && (!lines[endIdx - 1].trim() || /[$#]\s*$/.test(lines[endIdx - 1]))) {
		endIdx--;
	}

	const outputLines = lines.slice(startIdx, endIdx);
	return outputLines.join('\n').trim() || cleanOutput.trim() || '(no output)';
}

async function runCommandInTerminal(command: string, terminal: TerminalInfo): Promise<string> {
	return captureCommandOutput(command, terminal);
}

export async function sendMessage(
	prompt: string,
	context?: { connectionId?: string; host?: string; username?: string; os?: string; terminalOutput?: string },
	terminal?: TerminalInfo
): Promise<void> {
	const settings = getAISettings();
	if (!settings.enabled || !settings.apiKey || !settings.selectedModel) {
		error = 'AI is not configured. Go to Settings → AI to set up your API key and model.';
		return;
	}

	const userMsg: AIChatMessage = {
		id: crypto.randomUUID(),
		role: 'user',
		content: prompt,
		timestamp: Date.now()
	};
	messages.push(userMsg);

	const assistantId = crypto.randomUUID();
	messages.push({
		id: assistantId,
		role: 'assistant',
		content: '',
		timestamp: Date.now()
	});

	loading = true;
	error = undefined;
	cancelled = false;

	try {
		const systemPrompt = buildSystemPrompt(context);
		const apiMessages = [
			{ role: 'system', content: terminal
				? systemPrompt + '\n\nIMPORTANT: You have access to a live terminal. When you suggest commands, wrap them in ```bash code blocks and they will be automatically executed. After execution, you will receive the output and can analyze it. Suggest ONE command at a time for clarity.'
				: systemPrompt
			},
			...messages
				.filter((m) => m.id !== assistantId)
				.map((m) => ({ role: m.role, content: m.content }))
		];

		const content = await aiInvoke(apiMessages);

		if (cancelled) return;

		const idx = messages.findIndex((m) => m.id === assistantId);
		if (idx !== -1) {
			messages[idx].content = content;
		}

		// Auto-execute code blocks if we have an active terminal
		if (terminal && !cancelled) {
			let lastResponse = content;
			let rounds = 0;

			while (rounds < 6 && !cancelled) {
				const codeBlocks = extractCodeBlocks(lastResponse);
				if (codeBlocks.length === 0) break;

				const command = codeBlocks[0];
				const output = await runCommandInTerminal(command, terminal);

				if (cancelled) return;

				messages.push({
					id: crypto.randomUUID(),
					role: 'user',
					content: `[Auto-executed] \`${command}\`\n\nOutput:\n\`\`\`\n${output}\n\`\`\`\n\nAnalyze this output. If more investigation is needed, suggest the next command in a \`\`\`bash block. If the task is complete, summarize your findings without code blocks.`,
					timestamp: Date.now()
				});

				const nextId = crypto.randomUUID();
				messages.push({
					id: nextId,
					role: 'assistant',
					content: '',
					timestamp: Date.now()
				});

				const nextMessages = [
					{ role: 'system', content: systemPrompt + '\n\nYou are running commands on a live terminal and receiving their output. Analyze the output. If more commands are needed, suggest them in ```bash blocks (ONE at a time). If the task is complete, summarize your findings without any code blocks.' },
					...messages
						.filter((m) => m.id !== nextId)
						.map((m) => ({ role: m.role, content: m.content }))
				];

				lastResponse = await aiInvoke(nextMessages);

				if (cancelled) return;

				const nIdx = messages.findIndex((m) => m.id === nextId);
				if (nIdx !== -1) {
					messages[nIdx].content = lastResponse;
				}

				rounds++;
			}
		}
	} catch (e) {
		if (cancelled) return;
		error = String(e);
		const idx = messages.findIndex((m) => m.id === assistantId);
		if (idx !== -1 && !messages[idx].content) {
			messages.splice(idx, 1);
		}
	} finally {
		loading = false;
	}
}

export async function generatePlaybook(description: string): Promise<string> {
	return aiInvoke([
		{ role: 'system', content: 'Generate a YAML playbook for the Reach SSH tool. Format: name, description, variables, steps[{name, command, timeout}]. Output ONLY valid YAML, no markdown fences or explanation.' },
		{ role: 'user', content: description }
	]);
}

export async function fixPlaybook(yaml: string): Promise<string> {
	return aiInvoke([
		{ role: 'system', content: 'Fix and improve this YAML playbook for the Reach SSH tool. Correct any syntax errors, improve commands, add missing fields. Output ONLY valid YAML, no markdown fences or explanation.' },
		{ role: 'user', content: yaml }
	]);
}

export async function executeCommand(
	command: string,
	bufferId: string,
	tabType: 'ssh' | 'local',
	connectionId?: string,
	context?: { host?: string; username?: string; os?: string }
): Promise<void> {
	const terminal: TerminalInfo = { bufferId, tabType, connectionId };

	messages.push({
		id: crypto.randomUUID(),
		role: 'user',
		content: `Running command: \`${command}\`\n\nWaiting for output...`,
		timestamp: Date.now()
	});

	loading = true;
	error = undefined;
	cancelled = false;

	try {
		const output = await captureCommandOutput(command, terminal);

		if (cancelled) return;

		const userMsgIdx = messages.length - 1;
		messages[userMsgIdx].content = `Ran command: \`${command}\`\n\nOutput:\n\`\`\`\n${output}\n\`\`\``;

		const assistantId = crypto.randomUUID();
		messages.push({
			id: assistantId,
			role: 'assistant',
			content: '',
			timestamp: Date.now()
		});

		const sysPrompt = buildSystemPrompt(context ? { ...context, terminalOutput: output } : { terminalOutput: output });
		const apiMessages = [
			{ role: 'system', content: sysPrompt },
			...messages
				.filter((m) => m.id !== assistantId)
				.map((m) => ({ role: m.role, content: m.content }))
		];

		const content = await aiInvoke(apiMessages);

		if (cancelled) return;

		const idx = messages.findIndex((m) => m.id === assistantId);
		if (idx !== -1) {
			messages[idx].content = content;
		}

		// Continue auto-executing if the analysis suggests more commands
		let lastResponse = content;
		let rounds = 0;
		while (rounds < 5 && !cancelled) {
			const moreCmds = extractCodeBlocks(lastResponse);
			if (moreCmds.length === 0) break;

			const cmd = moreCmds[0];
			const cmdOutput = await runCommandInTerminal(cmd, terminal);

			if (cancelled) return;

			messages.push({
				id: crypto.randomUUID(),
				role: 'user',
				content: `[Auto-executed] \`${cmd}\`\n\nOutput:\n\`\`\`\n${cmdOutput}\n\`\`\`\n\nAnalyze this output.`,
				timestamp: Date.now()
			});

			const nextId = crypto.randomUUID();
			messages.push({
				id: nextId,
				role: 'assistant',
				content: '',
				timestamp: Date.now()
			});

			const nextMessages = [
				{ role: 'system', content: sysPrompt + '\n\nAnalyze the command output. If more commands are needed, suggest them in ```bash blocks. If done, summarize without code blocks.' },
				...messages
					.filter((m) => m.id !== nextId)
					.map((m) => ({ role: m.role, content: m.content }))
			];

			lastResponse = await aiInvoke(nextMessages);

			if (cancelled) return;

			const nIdx = messages.findIndex((m) => m.id === nextId);
			if (nIdx !== -1) {
				messages[nIdx].content = lastResponse;
			}

			rounds++;
		}
	} catch (e) {
		if (cancelled) return;
		error = String(e);
	} finally {
		loading = false;
	}
}
