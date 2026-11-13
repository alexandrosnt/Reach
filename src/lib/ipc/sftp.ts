import { invoke } from '@tauri-apps/api/core';

export interface FileEntry {
  name: string;
  path: string;
  isDirectory: boolean;
  size: number;
  modified: number;
  permissions: string;
}

export async function sftpListDir(connectionId: string, path: string): Promise<FileEntry[]> {
  return invoke<FileEntry[]>('sftp_list_dir', { connectionId, path });
}

/** `transferId` lets the caller listen for the transfer's events first. */
export async function sftpUpload(connectionId: string, localPath: string, remotePath: string, transferId?: string): Promise<string> {
  return invoke<string>('sftp_upload', { connectionId, localPath, remotePath, transferId });
}

export async function sftpDownload(connectionId: string, remotePath: string, localPath: string, transferId?: string): Promise<string> {
  return invoke<string>('sftp_download', { connectionId, remotePath, localPath, transferId });
}

/** Stop an upload or download; its partial file is removed. */
export async function sftpTransferCancel(transferId: string): Promise<void> {
  return invoke('sftp_transfer_cancel', { transferId });
}

export async function sftpDelete(connectionId: string, path: string): Promise<void> {
  return invoke('sftp_delete', { connectionId, path });
}

export async function sftpRename(connectionId: string, oldPath: string, newPath: string): Promise<void> {
  return invoke('sftp_rename', { connectionId, oldPath, newPath });
}

export async function sftpMkdir(connectionId: string, path: string): Promise<void> {
  return invoke('sftp_mkdir', { connectionId, path });
}

export async function sftpTouch(connectionId: string, path: string): Promise<void> {
  return invoke('sftp_touch', { connectionId, path });
}

export async function sftpReadFile(connectionId: string, path: string): Promise<string> {
  return invoke<string>('sftp_read_file', { connectionId, path });
}

export async function sftpWriteFile(connectionId: string, path: string, content: string): Promise<void> {
  return invoke('sftp_write_file', { connectionId, path, content });
}

/** The folder the connection's terminal is in, or null when the server cannot say (#162). */
export async function sftpTerminalCwd(connectionId: string): Promise<string | null> {
  return invoke<string | null>('sftp_terminal_cwd', { connectionId });
}
