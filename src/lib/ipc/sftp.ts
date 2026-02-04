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

export async function sftpUpload(connectionId: string, localPath: string, remotePath: string): Promise<string> {
  return invoke<string>('sftp_upload', { connectionId, localPath, remotePath });
}

export async function sftpDownload(connectionId: string, remotePath: string, localPath: string): Promise<string> {
  return invoke<string>('sftp_download', { connectionId, remotePath, localPath });
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
