---
title: File Explorer (SFTP)
description: Browse and manage remote files like a local file manager.
---

When you connect to a server via SSH, a file explorer shows up in the sidebar. It lets you browse the remote filesystem, move files around, and edit them without leaving Reach.

## Navigating

The explorer has breadcrumb navigation at the top. Click any path segment to jump directly to that directory. It works the way you'd expect from any file manager.

## File Operations

You can upload, download, delete, rename, and create new files and folders. Right-click to get a context menu with the relevant options depending on what you clicked:

**On a file:** Download, Edit, Delete, Rename.

**On a folder:** New File, New Folder, Delete, Rename.

**On empty space:** New File, New Folder, Refresh.

## Inline Editing

Right-click a file and pick "Edit" to open it in a code editor overlay. The editor has syntax highlighting for 30+ languages, detected automatically by file extension. Make your changes and press `Ctrl+S` to save back to the server. Or close the editor to discard.

There's a 5 MB size limit for inline editing. For anything bigger, download it, edit locally, and re-upload.

## Drag and Drop Upload

Drag files from your desktop onto the file explorer panel to upload them to the current directory. Nothing fancy to configure.

## Transfer Queue

When you upload or download files, a transfer queue shows progress bars for each active transfer. You can cancel any transfer that's in progress.

## How It Works

Under the hood, all file operations run over the existing SSH connection using shell commands (`ls`, `mv`, `rm`, `mkdir`, etc.). File transfers use SCP-style streaming. There's no separate SFTP subsystem connection needed.
