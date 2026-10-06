import type { Folder, SessionConfig } from '$lib/ipc/sessions';

export interface FolderTreeNode {
	folder: Folder;
	sessions: SessionConfig[];
	children: FolderTreeNode[];
	depth: number;
	totalSessions: number;
}

export interface FolderTree {
	roots: FolderTreeNode[];
	ungrouped: SessionConfig[];
}

type ShowEmpty = (folder: Folder) => boolean;

function hasParentCycle(folderId: string, parentId: string, folders: Map<string, Folder>): boolean {
	const seen = new Set<string>();
	let current: string | null = parentId;
	while (current) {
		if (current === folderId || seen.has(current)) return true;
		seen.add(current);
		current = folders.get(current)?.parent_id ?? null;
	}
	return false;
}

export function buildFolderTree(
	folders: Folder[],
	sessions: SessionConfig[],
	showEmpty: ShowEmpty = () => false,
): FolderTree {
	const folderMap = new Map(folders.map((folder) => [folder.id, folder]));
	const nodes = new Map<string, FolderTreeNode>(
		folders.map((folder) => [folder.id, { folder, sessions: [], children: [], depth: 0, totalSessions: 0 }]),
	);
	const ungrouped: SessionConfig[] = [];

	for (const session of sessions) {
		const node = session.folder_id ? nodes.get(session.folder_id) : undefined;
		if (node) node.sessions.push(session);
		else ungrouped.push(session);
	}

	const roots: FolderTreeNode[] = [];
	for (const folder of folders) {
		const node = nodes.get(folder.id)!;
		const parent = folder.parent_id ? nodes.get(folder.parent_id) : undefined;
		if (parent && !hasParentCycle(folder.id, parent.folder.id, folderMap)) parent.children.push(node);
		else roots.push(node);
	}

	function finish(node: FolderTreeNode, depth: number): FolderTreeNode | null {
		const children = node.children
			.map((child) => finish(child, depth + 1))
			.filter((child): child is FolderTreeNode => child !== null);
		const totalSessions = node.sessions.length + children.reduce((sum, child) => sum + child.totalSessions, 0);
		if (totalSessions === 0 && children.length === 0 && !showEmpty(node.folder)) return null;
		return { ...node, children, depth, totalSessions };
	}

	return {
		roots: roots.map((root) => finish(root, 0)).filter((root): root is FolderTreeNode => root !== null),
		ungrouped,
	};
}

export function folderPath(folderId: string, folders: Folder[], separator = ' / '): string {
	const folderMap = new Map(folders.map((folder) => [folder.id, folder]));
	const names: string[] = [];
	const seen = new Set<string>();
	let current: string | null = folderId;
	while (current && !seen.has(current)) {
		seen.add(current);
		const folder = folderMap.get(current);
		if (!folder) break;
		names.push(folder.name);
		current = folder.parent_id;
	}
	return names.reverse().join(separator);
}

export function canMoveFolder(folderId: string, parentId: string | null, folders: Folder[]): boolean {
	if (!parentId) return true;
	if (folderId === parentId) return false;
	const folderMap = new Map(folders.map((folder) => [folder.id, folder]));
	const folder = folderMap.get(folderId);
	const parent = folderMap.get(parentId);
	if (!folder || !parent || folder.vault_id !== parent.vault_id) return false;
	return !hasParentCycle(folderId, parentId, folderMap);
}
