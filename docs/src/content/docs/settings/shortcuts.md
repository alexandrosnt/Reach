---
title: Keyboard Shortcuts
description: All the keyboard shortcuts in Reach.
---

Reach has global keyboard shortcuts that work anywhere in the app. On Mac, replace Ctrl with Cmd.

## Tab Management

| Shortcut | Action |
|----------|--------|
| **Ctrl+T** | Open a new local terminal tab |
| **Ctrl+W** | Close the current tab |
| **Ctrl+Tab** | Switch to the next tab (wraps around) |
| **Ctrl+Shift+Tab** | Switch to the previous tab (wraps around) |

## App Controls

| Shortcut | Action |
|----------|--------|
| **Ctrl+,** | Open Settings |
| **Ctrl+Shift+A** | Toggle AI assistant panel |
| **Ctrl+Shift+P** | Command palette (placeholder, not yet active) |
| **Escape** | Close modal or dialog |

## Terminal

These work inside the terminal area:

| Shortcut | Action |
|----------|--------|
| **Ctrl+C** (with selection) | Copy selected text to clipboard |
| **Ctrl+C** (no selection) | Send interrupt signal (SIGINT) |
| **Ctrl+V** | Paste from clipboard |
| **Right-click** | Paste from clipboard |
| **Ctrl+S** | Save file (in the file editor overlay) |
| **Shift+PageUp** | Scroll up through terminal history |
| **Shift+PageDown** | Scroll down through terminal history |

## How Shortcuts Work

Shortcuts are registered globally when the app starts. The handler checks the key combination, prevents the default browser behavior, and runs the action.

Mac detection uses `navigator.platform` to decide whether to match Ctrl or Cmd as the modifier. Shift+Tab is special-cased to avoid interfering with Tab key handling in forms and inputs.

:::note
Shortcuts are not customizable yet — the bindings are hardcoded. When the terminal is focused, most key input goes straight to the shell. The Ctrl-modified shortcuts above are designed to not conflict with normal terminal input.
:::
