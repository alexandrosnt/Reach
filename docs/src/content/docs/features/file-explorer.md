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

The row you right-clicked is highlighted for as long as the menu is open, so
there is never a doubt about which file an action is about to apply to.

**On a file:**
- **Edit** — open in the inline editor (text files only, max 5 MB)
- **Download** — save to your local machine
- **Archive** — compress it, or extract it if it already is an archive
- **Rename** — inline rename field appears
- **Delete** — confirmation bar appears before deleting

**On a directory:**
- **Archive** — compress the folder and everything in it
- **Rename** — inline rename
- **Delete** — recursive delete with confirmation

Select several files first and the menu applies to all of them, which is how
you get a single archive out of a multiple selection.

**On empty space:**
- **New File** — creates an empty file (inline naming)
- **New Folder** — creates a directory (inline naming)
- **Refresh** — reloads the listing

## Archives

Right-click and choose **Archive** to compress or extract without opening a
terminal. The work happens on the remote machine, so nothing is transferred to
your computer and back just to be packed.

### Compressing

Pick a format and Reach builds the archive in the current directory:

| Format | Needs |
| --- | --- |
| `.tar.gz`, `.tgz` | `tar` with gzip — present almost everywhere |
| `.tar.xz`, `.txz` | `xz` |
| `.tar.bz2`, `.tbz2` | `bzip2` |
| `.tar.zst`, `.tzst` | `zstd` |
| `.tar` | `tar` alone |
| `.zip` | `zip`, or `7z`, or Python |

Only the formats the server can actually produce are offered. Reach probes for
`tar`, `gzip`, `bzip2`, `xz`, `zstd`, `zip`, `unzip`, `bsdtar`, `7z` and
`python3` when the menu opens, and hides the rest rather than letting you pick
something that will fail. Where a tool is missing but installable, it offers to
install it with the machine's own package manager.

`.zip` has several fallbacks because it is the format people want and the one
least likely to be installed: `zip` if present, then `7z`, then a small Python
script. That last one matters on stripped containers — an Alpine image
typically has Python and no `zip` at all.

### Extracting

Right-click an archive and the same menu offers **Extract**. The contents go
into a folder named after the archive, so `backup.tar.gz` unpacks into
`backup/` rather than scattering files across the directory you were in.

### Names never collide

If `backup.tar.gz` already exists, the new one becomes `backup-1.tar.gz`. The
number goes before the extension, not after it, so the file is still recognised
as a `.tar.gz` by everything that looks at names.

### Progress, and changing your mind

Compressing a large directory shows a real percentage rather than a spinner,
counting entries as they are processed. The panel stays in view while it runs —
you do not have to scroll to find it.

Reach asks the tool to run under a pseudo-terminal so it reports progress line
by line. Without that, most archivers detect they are writing to a pipe and
buffer their output in large blocks, which is why a long compression can
otherwise sit at zero and then jump straight to done.

**Cancel** stops the operation and removes the half-written archive, so a
cancelled compression leaves the directory exactly as it was.

## Drag a File Out to Your Desktop

Drag a file from the explorer onto your desktop or any folder, and it downloads
to wherever you dropped it.

Nothing is transferred while you drag. The download starts when you release the
button, and only then — so dragging a 2 GB file across your screen and changing
your mind costs nothing.

On Windows this uses delayed rendering, which is the mechanism Explorer itself
uses: Reach hands over a promise of a file rather than the file, and the bytes
are streamed straight to the destination you chose when Explorer asks for them.
The progress bar you see is Explorer's own.

:::note
Dragging out is currently a Windows feature. On macOS and Linux, use
**Download** from the context menu.
:::

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
