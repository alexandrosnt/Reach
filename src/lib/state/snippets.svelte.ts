import { snippetList, snippetCreate, snippetUpdate, snippetDelete, type Snippet } from '$lib/ipc/snippets';
import { getActiveTab } from '$lib/state/tabs.svelte';
import { sshSend } from '$lib/ipc/ssh';
import { ptyWrite } from '$lib/ipc/pty';

let snippets = $state<Snippet[]>([]);

// --- Trie (Prefix Tree) for O(m) command lookup ---
interface TrieNode {
	children: Map<string, TrieNode>;
	snippet: Snippet | null;
}

function createNode(): TrieNode {
	return { children: new Map(), snippet: null };
}

let trieRoot: TrieNode = createNode();

function rebuildTrie(): void {
	trieRoot = createNode();
	for (const s of snippets) {
		let node = trieRoot;
		for (const ch of s.command.toLowerCase()) {
			if (!node.children.has(ch)) {
				node.children.set(ch, createNode());
			}
			node = node.children.get(ch)!;
		}
		node.snippet = s;
	}
}

/** O(m) prefix lookup — returns the first snippet whose command starts with input */
export function trieMatch(input: string): Snippet | null {
	if (!input || input.length < 2) return null;
	let node = trieRoot;
	for (const ch of input.toLowerCase()) {
		const next = node.children.get(ch);
		if (!next) return null;
		node = next;
	}
	// Walk down to find the first complete command
	return findFirst(node);
}

function findFirst(node: TrieNode): Snippet | null {
	if (node.snippet) return node.snippet;
	for (const child of node.children.values()) {
		const found = findFirst(child);
		if (found) return found;
	}
	return null;
}

// --- Public API ---

/** The active terminal's channel: an SSH tab's connection, or a local tab's
    pty, which is keyed by the tab's own id (it has no connectionId — the
    reason a snippet's Paste did nothing in a local terminal). */
function terminalTarget(): { ssh: boolean; id: string } | null {
	const tab = getActiveTab();
	if (tab?.type === 'ssh') return tab.connectionId ? { ssh: true, id: tab.connectionId } : null;
	if (tab?.type === 'local') return { ssh: false, id: tab.id };
	return null;
}

/** Whether there is a terminal for a snippet to go into. */
export function canPasteSnippet(): boolean {
	return terminalTarget() !== null;
}

/** Types `command` into the active terminal, without pressing Enter: the
    snippet panel's Paste and the command palette both use it. False when
    there is no terminal to type into. */
export function pasteSnippet(command: string): boolean {
	const target = terminalTarget();
	if (!target) return false;
	const data = Array.from(new TextEncoder().encode(command));
	if (target.ssh) {
		sshSend(target.id, data);
	} else {
		ptyWrite(target.id, data);
	}
	return true;
}

export function getSnippets(): Snippet[] {
	return snippets;
}

export async function loadSnippets(): Promise<void> {
	try {
		snippets = await snippetList();
		rebuildTrie();
	} catch (err) {
		console.error('Failed to load snippets:', err);
	}
}

export async function addSnippet(name: string, command: string, description?: string, tags: string[] = []): Promise<Snippet> {
	const snippet = await snippetCreate({ name, command, description, tags });
	snippets = [...snippets, snippet];
	rebuildTrie();
	return snippet;
}

export async function editSnippet(snippet: Snippet): Promise<void> {
	const updated = await snippetUpdate(snippet);
	snippets = snippets.map(s => s.id === updated.id ? updated : s);
	rebuildTrie();
}

export async function removeSnippet(id: string): Promise<void> {
	await snippetDelete(id);
	snippets = snippets.filter(s => s.id !== id);
	rebuildTrie();
}
