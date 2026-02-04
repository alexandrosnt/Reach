export interface Session {
	id: string;
	name: string;
	host: string;
	port: number;
	username: string;
	auth_method: { type: 'Password' } | { type: 'Key'; path: string } | { type: 'Agent' };
	folder_id: string | null;
	tags: string[];
	detected_os?: string | null;
}

export interface Folder {
	id: string;
	name: string;
	parent_id: string | null;
}

const sessions = $state<Map<string, Session>>(new Map());
const folders = $state<Map<string, Folder>>(new Map());

const sessionList = $derived<Session[]>(Array.from(sessions.values()));

export function getSessions(): Map<string, Session> {
	return sessions;
}

export function getFolders(): Map<string, Folder> {
	return folders;
}

export function getSessionList(): Session[] {
	return sessionList;
}

export function addSession(session: Omit<Session, 'id'>): Session {
	const id = crypto.randomUUID();
	const newSession: Session = { ...session, id };
	sessions.set(id, newSession);
	return newSession;
}

export function updateSession(id: string, updates: Partial<Omit<Session, 'id'>>): void {
	const existing = sessions.get(id);
	if (!existing) return;

	sessions.set(id, { ...existing, ...updates });
}

export function deleteSession(id: string): void {
	sessions.delete(id);
}

export function getSessionsByFolder(folderId: string | null): Session[] {
	return sessionList.filter((s) => s.folder_id === folderId);
}

export function addFolder(name: string, parentId: string | null = null): Folder {
	const id = crypto.randomUUID();
	const folder: Folder = { id, name, parent_id: parentId };
	folders.set(id, folder);
	return folder;
}

export function deleteFolder(id: string): void {
	// Remove the folder
	folders.delete(id);

	// Remove all child folders recursively
	const childFolderIds = Array.from(folders.values())
		.filter((f) => f.parent_id === id)
		.map((f) => f.id);

	for (const childId of childFolderIds) {
		deleteFolder(childId);
	}

	// Unassign sessions that belonged to this folder
	for (const [sessionId, session] of sessions) {
		if (session.folder_id === id) {
			sessions.set(sessionId, { ...session, folder_id: null });
		}
	}
}
