---
title: File Explorer (SFTP)
description: Browse and manage remote files like a local file manager.
---

When you connect to a server via SSH, a file explorer shows up in the sidebar. It lets you browse the remote filesystem, move files around, and edit them without leaving Reach.

## Navigating

The explorer has breadcrumb navigation at the top. Click any path segment to jump directly to that directory. There's also an **Up** button (left arrow) to go to the parent directory — it's disabled when you're at the root.

A **Refresh** button reloads the current directory listing.

## File List

Each entry shows:

- **Icon** — folder or file icon
- **Name** — truncated with ellipsis if too long
- **Size** — formatted in B, KB, MB, or GB (hidden for directories)
- **Permissions** — Unix-style string like `drwxr-xr-x`

Files are sorted with directories first, then alphabetically by name. Click a directory to open it. The listing comes from running `ls -lA` over the SSH connection, so there's nothing extra to install on the server.

## Context Menu

Right-click to get actions. What shows up depends on what you clicked:

**On a file:**
- **Edit** — open in the inline editor (text files only, max 5 MB)
- **Download** — save to your local machine
- **Rename** — inline rename field appears
- **Delete** — confirmation bar appears before deleting

**On a directory:**
- **Rename** — inline rename
- **Delete** — recursive delete with confirmation

**On empty space:**
- **New File** — creates an empty file (inline naming)
- **New Folder** — creates a directory (inline naming)
- **Refresh** — reloads the listing

## Inline Editing

Right-click a file and pick Edit. A code editor overlay opens with syntax highlighting for 30+ languages, auto-detected by file extension. Make your changes and press **Ctrl+S** to save back to the server. Close the editor to discard.

There's a 5 MB limit. Larger files need to be downloaded, edited locally, and re-uploaded. Under the hood, file content is transferred via base64 encoding so binary-safe reads work correctly.

## Renaming

When you rename a file or folder, the name turns into an editable text field right in the list. Press **Enter** to confirm, **Escape** to cancel. Clicking outside the field also confirms the rename. The backend runs `mv` over SSH.

## Creating Files and Folders

Same inline approach as renaming — a text field appears with a placeholder. Type the name, press Enter, and it's created. New files use `touch`, new folders use `mkdir -p`.

## Deleting

Delete triggers a confirmation bar that appears inline: "Are you sure you want to delete [filename]?" with Cancel and Delete buttons. Directories are deleted recursively with `rm -rf`.

## Drag and Drop Upload

Drag files from your desktop onto the file explorer panel. A blue dashed outline appears with "Drop files here to upload" as you hover. Drop the files and they start uploading to the current directory.

Uploads are streamed in 48KB base64 chunks so you can see progress in real time. Multiple files upload sequentially — not in parallel — to avoid flooding the SSH connection.

## Transfer Queue

Active uploads and downloads show a progress display:

- **Filename** and transfer direction icon
- **Progress bar** with percentage
- **Size** — bytes transferred / total (e.g., "2.4 MB / 10.1 MB")

Completed transfers show a green checkmark. Failed transfers show a red error message. Both have a dismiss button (X). There's a **Clear finished** button to remove all completed and errored transfers at once.

## Plugin Hooks

Plugins get notified when transfers complete:

- `sftp_upload_complete` — fires after a successful upload
- `sftp_download_complete` — fires after a successful download

## How It Works

All file operations run over the existing SSH connection using shell commands. There's no separate SFTP subsystem — it's all `ls`, `mv`, `rm`, `mkdir`, `touch`, `cat`, and `base64` piped through the SSH channel. File transfers use base64 streaming for binary safety.

Errors are caught and shown in a red bar below the breadcrumb with a **Retry** button.
