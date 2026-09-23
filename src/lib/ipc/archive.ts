/**
 * Compressing and extracting files on the machine you are connected to.
 *
 * The work happens over SSH on the remote side, not here — the files are
 * there, and pulling a directory down to compress it and pushing it back would
 * be absurd for anything larger than a few megabytes.
 *
 * Which formats are possible depends on what that machine has installed, so
 * the menu asks first rather than offering something that fails. `.tar.gz`
 * effectively always works; the rest are offered only when they will.
 */

import { invoke } from '@tauri-apps/api/core';

/** Formats a machine may be able to create. No `.rar`: see the Rust module. */
export type ArchiveFormat = 'tarGz' | 'tarBz2' | 'tarXz' | 'tarZst' | 'zip';

/** What each format is called on disk, which is also what the menu shows. */
export const FORMAT_EXTENSION: Record<ArchiveFormat, string> = {
	tarGz: 'tar.gz',
	tarBz2: 'tar.bz2',
	tarXz: 'tar.xz',
	tarZst: 'tar.zst',
	zip: 'zip'
};

export interface ArchiveTools {
	present: string[];
}

export interface ArchiveOutcome {
	/** Which tool actually did it, so the user can be told. */
	tool: string;
	/** The archive that was written, or the directory extracted into. */
	produced: string;
}

export async function archiveTools(connectionId: string): Promise<ArchiveTools> {
	return invoke('sftp_archive_tools', { connectionId });
}

export async function archiveCreate(
	connectionId: string,
	directory: string,
	entries: string[],
	archiveName: string,
	format: ArchiveFormat
): Promise<ArchiveOutcome> {
	return invoke('sftp_archive_create', {
		connectionId,
		directory,
		entries,
		archiveName,
		format
	});
}

export async function archiveExtract(
	connectionId: string,
	directory: string,
	archiveName: string
): Promise<ArchiveOutcome> {
	return invoke('sftp_archive_extract', { connectionId, directory, archiveName });
}

/**
 * Which formats this machine can produce, in the order to show them.
 *
 * Mirrors `creatable_formats` in the Rust module. Duplicated deliberately:
 * the menu needs the answer before any command runs, and asking the backend
 * per right-click would put a round trip in front of a context menu.
 */
export function creatableFormats(tools: ArchiveTools): ArchiveFormat[] {
	const has = (t: string) => tools.present.includes(t);
	const tar = has('tar') || has('bsdtar');
	const out: ArchiveFormat[] = [];
	if (tar && has('gzip')) out.push('tarGz');
	if (tar && has('bzip2')) out.push('tarBz2');
	if (tar && has('xz')) out.push('tarXz');
	if (tar && has('zstd')) out.push('tarZst');
	if (has('zip') || has('bsdtar') || has('7zz') || has('7z') || has('7za') || has('python3')) {
		out.push('zip');
	}
	return out;
}

/** Extensions we can open. Mirrors `kind_of` in the Rust module. */
const EXTRACTABLE = [
	'.tar.gz',
	'.tar.bz2',
	'.tar.xz',
	'.tar.zst',
	'.tgz',
	'.tbz2',
	'.txz',
	'.tzst',
	'.tar',
	'.zip'
];

/** Whether this filename is something the Extract item should appear for. */
export function isExtractable(name: string, tools: ArchiveTools): boolean {
	const lower = name.toLowerCase();
	if (!EXTRACTABLE.some((e) => lower.endsWith(e))) return false;
	const has = (t: string) => tools.present.includes(t);
	if (lower.endsWith('.zip')) {
		return has('unzip') || has('bsdtar') || has('7zz') || has('7z') || has('7za') || has('python3');
	}
	return has('tar') || has('bsdtar');
}
