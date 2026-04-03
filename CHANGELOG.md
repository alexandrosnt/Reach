# Changelog

All notable changes to Reach are documented here.

## v0.3.9
- **SSH reconnect button** — When an SSH connection drops, a "Reconnect" overlay appears on the terminal with a single click to re-establish the connection using the same credentials. Terminal buffer is preserved so you can see what happened before the disconnect.
- **Host key auto-accept on server reinstall** — If a server's host key changes (e.g. VPS reformatted), Reach automatically updates the stored fingerprint and connects. No more manual editing of `known_hosts.json`.
- **Local Only setup option** — The welcome wizard now lets you choose between "Local Only" (data on this device) and "Turso Cloud Sync". Users who don't need cloud sync can skip Turso setup entirely.
- **Russian locale** — Added 5 new reconnection keys, 6 new storage mode keys across all 7 locales.

## v0.3.8
- **Command snippets** — Save frequently used commands in the new Snippets sidebar panel. Add, edit, delete, search, and organize with tags. Click to paste or run in any terminal. Stored encrypted in the vault, persists across restarts.
- **Terminal autocomplete with ghost text** — Type in the terminal and saved snippets appear as dimmed placeholder text at the cursor position (fish shell style). Press Tab to accept. Uses a Trie (prefix tree) for O(m) lookup. Adaptive echo detection polls cursor movement instead of fixed delay — works on fast LAN and slow proxy connections alike.
- **Network monitoring** — Upload/download speed displayed in the monitoring bar alongside CPU, RAM, and disk. Reads `/proc/net/dev` with delta calculation across all non-loopback interfaces. Auto-formats to B/s, KB/s, MB/s, GB/s.
- **Hidden CMD windows** — All Ansible, OpenTofu, and toolchain commands on Windows now use `CREATE_NO_WINDOW` flag. No more CMD popups flashing on screen.
- **Folder delete simplified** — Single-click delete instead of double-click confirmation that broke on macOS WebKit.
- **Status bar cleaned up** — Removed hardcoded version text from the bottom bar.
- **Proxy save fix** — `sessionCreate` now accepts proxy config directly. No more fragile create-then-update.
- **Proxy in Quick Connect** — Full SOCKS5/SOCKS4/HTTP proxy support added to Quick Connect modal.
- **macOS folder delete fix** — Simplified folder deletion to work reliably on macOS WebKit.
- **Russian locale fixed** — Synced ru.json to match all 910 keys from en.json.
- **Snippets vault mapping** — `__snippets__` added to unified vault mapping for persistence with personal sync.

## v0.3.7
- **Tab labels show server name + OS icon** — SSH tabs display the saved session name (e.g. "MiniPC") instead of `root@IP`. Detected OS distro icon (Ubuntu, Debian, Rocky, etc.) replaces the generic SSH icon. Hover for the full `user@host` tooltip.
- **Click-to-copy terminal selection** — Click selected text in the terminal to copy it to clipboard automatically. *(Contributed by [@alien-ye](https://github.com/alien-ye))*
- **Proxy fix: saves on first create** — Proxy config is now passed directly in `sessionCreate` instead of a fragile create-then-update. Proxy checkbox and fields save reliably on new sessions.
- **Proxy in Quick Connect** — Full proxy support (SOCKS5/SOCKS4/HTTP) added to Quick Connect, not just the session editor.
- **macOS entitlements** — Added `Entitlements.plist` with `network.client` and `network.server` for signed/notarized macOS builds. Proxy and SSH connections work in production on Mac.
- **Proxy checkbox visible on macOS** — Explicit checkbox sizing and `accent-color` styling for WebKit/Safari rendering.

## v0.3.6
- **Ctrl + Mouse Wheel zoom** — Change terminal font size with Ctrl + Scroll in any terminal tab. Saves automatically, persists across restarts.
- **Removed font size slider** — No separate "console text size" setting. Terminal sizing is Ctrl+Wheel only, keeping UI and terminal scaling independent.
- **Font family via Settings only** — Google Fonts picker stays in Appearance for choosing terminal font family, font size is controlled naturally in the terminal itself.

## v0.3.5
- **Proxy support (SOCKS5/SOCKS4/HTTP)** — Connect to SSH servers through proxies. Supports Tor (SOCKS5 on 127.0.0.1:9050), corporate HTTP CONNECT proxies, and any SOCKS4/5 proxy with optional authentication. Configured per session in the session editor.
- **Vault-scoped folders** — Folders now belong to the vault where they were created. A folder in "DevOps Team" vault won't appear in "Private" or other vaults.
- **Clean toolchain check** — Ansible and OpenTofu panels show a spinner while checking installation status instead of raw terminal commands.
- **Zero svelte-check warnings** — Fixed all 17 accessibility warnings (form labels, ARIA roles, unused CSS) across Ansible, Tofu, and Vault components.

## v0.3.4
- **Google Fonts integration** — Searchable font picker in Settings → Appearance with 24+ monospace fonts from Google Fonts, each rendered in its own typeface.
- **Live font preview** — Real-time preview in Appearance settings shows selected font and size with terminal-style output.
- **Live terminal font & size updates** — Changing font family or size in settings updates all open terminals instantly.
- **Font persistence** — Selected Google Font loads on app startup so terminals use the saved font immediately.
- **Fix session/folder data collision** — Sessions and folders in unified vaults no longer cross-contaminate (SecretCategory filtering).
- **Folder delete confirmation** — Double-click to delete: first click shows "Confirm", second click deletes.
- **Drag & drop sessions into folders** — Grab the grip handle to drag sessions between folders (pointer events, works on Windows WebView2).
- **Folder collapse persistence** — Expanded/collapsed state saved to localStorage across view switches and restarts.
- **Right-click improvements** — No browser default context menu, full-height clickable area, folder creation on empty vaults.

## v0.3.3
- **Connection folders** — Organize sessions into folders via right-click → Move to Folder. Create, collapse, and delete folders. Deleting a folder safely unassigns sessions back to ungrouped.
- **Connection search** — Real-time search bar filters sessions by name, host, username, or tags.
- **File explorer search** — Filter files and folders by name in the current directory.
- **File preview** — Right-click a file → Preview to view contents in a read-only overlay.
- **CD here** — Right-click a folder in the file explorer → CD here to send `cd /path` to the SSH terminal.
- **Copy path / filename** — Right-click any file or folder to copy its full path or filename to clipboard.
- **One-click download** — Download button appears on hover for each file in the explorer.
- **Direct path navigation** — Double-click the breadcrumb bar to type a path directly.
- **Folder selector in session editor** — Assign sessions to folders when creating or editing.
- **SSH host key verification (TOFU)** — Saves host key fingerprints on first connection, rejects changed keys to prevent MITM attacks, stores `known_hosts.json` under the app data directory.

## v0.3.2
- **Fix editor white screen on reopen** — `WebviewWindow.as_ref()` resolved to `&Webview` instead of `&Window`, causing `hide()` to blank the webview content while leaving the OS window frame visible. All window operations now call `WebviewWindow` methods directly.
- **Fix editor window not hiding on close** — Added `on_window_event` handler to intercept `CloseRequested` for editor windows with `api.prevent_close()` + `hide()`, preventing WebView2 destruction.
- **Force WebView2 repaint** — Added 1px size nudge after `show()` to force WebView2 rendering pipeline restart (workaround for Microsoft WebView2Feedback #1077).
- Editor tabs now persist across hide/show cycles instead of being cleared on close.

## v0.3.1
- **Fix editor window reopen** — Editor now hides instead of closing to avoid WebView2 crash on Windows; reopens instantly when editing another file.
- **Fix editor initial load** — Files are delivered via Rust backend message queue instead of unreliable cross-window events.
- **Editor tabs** — Multiple files open as tabs in a single editor window with dirty-state indicators.
- **Close tab shortcut** — Changed from Ctrl+W to Ctrl+Shift+W to avoid browser conflicts.
- Fixed corrupted i18n locale files, added `editor.opening` key to all 6 locales.

## v0.3.0
- **Ansible integration** — Full Ansible UI with project management, playbook execution, inventory editor, roles/collections management, ad-hoc commands, and vault encrypt/decrypt.
- **OpenTofu integration** — Infrastructure-as-Code workspace with project management, plan/apply/destroy, state inspection, and provider/module management.
- **WSL auto-detection** — On Windows, Ansible commands automatically route through WSL with two-step status checks (WSL available + Ansible installed).
- **Toolchain installer** — One-click install for Ansible (via pip/pipx, or through WSL on Windows) and OpenTofu (direct binary download).
- Streaming command output with color-coded stdout/stderr for both Ansible and OpenTofu operations.
- Vault-backed project storage — all IaC projects are encrypted at rest alongside sessions and credentials.

## v0.2.3
- Fixed app failing to launch on Linux Wayland (Error 71 Protocol error) by disabling the WebKitGTK DMA-BUF renderer. Affects KDE Plasma, GNOME, Sway, especially with NVIDIA drivers.

## v0.2.2
- Fixed plugin button actions (e.g. Refresh) failing when calling async host API functions like `reach.ssh.exec()`.
- Fixed plugin hooks not awaiting async Lua calls, causing `reach.ssh.exec()` to silently return errors.
- Plugins now auto-load on app startup instead of requiring manual activation via Settings > Plugins.
- Fixed SFTP upload/download completion hooks not awaiting async plugin dispatch.

## v0.2.1
- **Jump host (ProxyJump) support** — Connect through bastion servers with multi-hop SSH tunneling via russh direct-tcpip channels.
- **SSH config import** — Parse and import hosts from `~/.ssh/config` with automatic ProxyJump chain resolution (cross-platform).
- **Lua plugin system (beta)** — Sandboxed Lua VMs with host API for SSH commands, storage, and UI hooks.
- Session editor and Quick Connect now support jump host configuration with per-hop auth settings.
- New Plugins tab in Settings for managing Lua plugins.
- 19 new i18n keys across all 6 locales.

## v0.2.0
- Fixed drag-and-drop file uploads stacking and freezing — uploads now run sequentially instead of flooding the SSH connection.
- Fixed silent upload failures — errors now show a toast notification instead of being swallowed.
- Added feedback when dragging files from browsers or apps that don't provide file paths.
- Added copy button to every AI chat message (appears on hover).

## v0.1.9
- Updated app preloader to use the actual app icon.

## v0.1.8
- Added Bulgarian language.
- 6 languages supported now: English, German, French, Greek, Italian, Bulgarian.

## v0.1.7
- Single-instance mode, reopening the app focuses the existing window instead of opening a new one.
- Fixed the auto-updater so it actually generates update artifacts and signatures.
- Fixed release workflow for all platforms.

## v0.1.6
- Welcome wizard on first run with language selection and optional Turso cloud sync setup.
- Added German, French, Greek and Italian translations.
- Language selector in Settings > General.
- Language picker with flag icons during setup.
