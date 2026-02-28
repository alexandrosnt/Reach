---
title: SSH Terminal
description: Full interactive SSH terminal with tabs, splits, and WebGL rendering.
---

The terminal in Reach is built on xterm.js with WebGL rendering. It handles 256 colors and truecolor, sets `TERM=xterm-256color` and `COLORTERM=truecolor`, so tools like `htop`, `vim`, and everything else that expects a modern terminal will render correctly. If your GPU doesn't support WebGL, it falls back to canvas automatically.

The font is JetBrains Mono by default, with SF Mono and Cascadia Code as fallbacks. You can change this in Settings > Appearance.

## Tabs

Each terminal session gets its own tab. You can have as many open as you want — local shells and SSH connections mixed together. Local tabs show a terminal icon, SSH tabs show a globe.

Keyboard shortcuts for tab management:

- **Ctrl+T** — open a new local terminal tab
- **Ctrl+W** — close the current tab
- **Ctrl+Tab** — switch to the next tab
- **Ctrl+Shift+Tab** — switch to the previous tab

On Mac, use Cmd instead of Ctrl.

Tab titles update automatically based on the shell prompt. For SSH tabs, it picks up `user@host` from the terminal title escape sequence. If you `sudo su -` to another user, the tab title updates to reflect the new username.

## Clipboard

**Ctrl+C** does two things depending on context. If you have text selected in the terminal, it copies. If nothing is selected, it sends the interrupt signal (SIGINT) like you'd expect. No weird mode switching needed.

**Ctrl+V** pastes. Right-click also pastes.

## Split Panes

You can split the terminal area horizontally or vertically. The divider between panes is draggable — grab it and slide to resize. Neither pane can shrink below 100px.

## OS Detection

When you connect to a server over SSH, Reach runs a quick check to figure out what OS it's running. It reads `/etc/os-release` to identify the distribution (Ubuntu, Debian, Alpine, Rocky, etc.), or falls back to `uname -s` for macOS and BSD. The distro icon shows up on the tab so you can tell your servers apart at a glance. It recognizes 50+ Linux distributions.

## Color Initialization

SSH connections get a color setup script injected automatically on connect. This sets up `ls --color=auto`, colored grep, and a PS1 prompt with colors (green for regular users, red for root). It runs silently and clears the screen, so you get a clean terminal with colors working out of the box.

## Resize and Scrollback

The terminal resizes automatically when you drag the window, toggle the sidebar, or resize split panes. A 50ms debounce prevents excessive resize calls. The backend gets notified too (via `SIGWINCH` for PTY, `window_change` for SSH) so the remote shell knows about the new dimensions.

Scrollback is set to 10,000 lines. Scroll up with the mouse wheel or `Shift+PageUp`.

## Monitoring Bar

When you're connected to a server via SSH, a bar appears at the bottom of the terminal showing live system stats: CPU, RAM, disk usage, and logged-in users. Stats are color-coded — green under 60%, yellow from 60-84%, red at 85% and above.

The monitoring polls every 3 seconds over the existing SSH connection. No agents to install. If your username shows up in the logged-in users list, it gets tagged with "(you)". Hover over the users section to see the full list with TTY info.

Each SSH connection has its own monitoring data. Switch tabs and the bar updates to show the right server's stats.

## Local Terminals (PTY)

Local tabs spawn a shell on your machine using a real pseudo-terminal. On Windows it defaults to PowerShell (with `-NoLogo`), on Linux/macOS it uses your `$SHELL` or falls back to `/bin/bash`.

Each PTY gets its own OS thread for reading output, so one busy terminal doesn't block the others. Data comes through in 4KB chunks, which keeps things responsive even with heavy output.

## SSH Connections

SSH is implemented in pure Rust via russh — no OpenSSH dependency. Connections go through:

1. TCP connect (15-second timeout)
2. SSH handshake
3. Authentication (password or private key)
4. PTY allocation
5. Shell request
6. Color initialization

Data flows through an async event loop using `tokio::select!` to handle both incoming data from the server and outgoing keystrokes from you simultaneously. Stderr is merged with stdout so you see everything in one stream.

## Jump Host Chains

If your session has a ProxyJump chain configured, Reach connects through each hop using SSH `direct-tcpip` channels. The chain is ordered outermost-first — connect to bastion A, tunnel through to bastion B, then reach the target. All intermediate connections stay alive for the duration. See the [Jump Hosts](/Reach/features/jump-hosts/) page for details.

## Buffer Access

The terminal buffer is accessible to internal features and plugins. The last 50 lines can be read programmatically, which is how the AI assistant gets context about what you're doing. The full scrollback buffer line count is also available.
