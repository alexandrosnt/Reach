import { snippetList, snippetCreate, snippetUpdate, snippetDelete, type Snippet } from '$lib/ipc/snippets';

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
