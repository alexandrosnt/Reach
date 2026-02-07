---
title: SSH Terminal
description: Full interactive SSH terminal with tabs, splits, and WebGL rendering.
---

The terminal in Reach is built on xterm.js. It uses WebGL rendering by default for smooth performance, and falls back to canvas if your system doesn't support it.

You get full color support: 256 colors and truecolor both work. The TERM variable is set to `xterm-256color`, so tools like `htop`, `vim`, and anything else that expects a modern terminal will render correctly.

## Tabs

Open multiple terminals at once. Press `Ctrl+T` to open a new local tab (a shell on your machine). Each tab can be either local or SSH (a remote server).

To manage tabs:

- `Ctrl+W` closes the current tab
- `Ctrl+Tab` and `Ctrl+Shift+Tab` switch between tabs
- Right-click a tab for a context menu with options to close it, close all others, or close everything to the right

## Clipboard

`Ctrl+C` has two jobs. If you have text selected, it copies. If nothing is selected, it sends the interrupt signal like you'd expect. `Ctrl+V` pastes.

## OS Detection

When you connect to a remote server over SSH, Reach figures out what OS it's running and shows the distro icon on the tab. It recognizes 50+ Linux distributions, plus macOS and the BSDs. This is a small touch, but it makes it easy to tell your tabs apart at a glance.

## Terminal Title

The tab title updates automatically based on the shell prompt. It usually shows `user@host`, so you always know where you are.

## Resize and Scrollback

The terminal resizes automatically when you drag the window or toggle the sidebar. No manual adjustment needed.

Scrollback history is set to 10,000 lines. Scroll up with the mouse wheel or `Shift+PageUp` to see older output.

## Monitoring Bar

When you're connected to a server via SSH, a small bar appears at the bottom of the terminal showing live system stats: CPU usage, RAM usage, disk usage, and who's logged in. This polls every 3 seconds. It's a quick way to keep an eye on the server without running `htop` in a separate window.
