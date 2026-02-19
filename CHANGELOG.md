# Changelog

## v0.2.3

### Bug Fixes

- **Linux Wayland crash** — Fixed app failing to launch on Wayland with `Error 71 (Protocol error)` by disabling the WebKitGTK DMA-BUF renderer on Linux. Affects KDE Plasma, GNOME, Sway, and other Wayland compositors, especially with NVIDIA proprietary drivers.

---

## v0.2.2

### Bug Fixes

- **Plugin async actions** — Fixed plugin button actions (e.g. Refresh) failing when calling async host API functions like `reach.ssh.exec()`. Actions now properly await async operations.
- **Plugin async hooks** — Fixed plugin hooks (`session:connected`, `session:disconnected`, etc.) not awaiting async Lua calls, causing `reach.ssh.exec()` to silently fail in hook handlers.
- **Plugin auto-loading** — Plugins now automatically discover and load on app startup instead of requiring manual activation via Settings > Plugins.
- **SFTP hook dispatch** — Fixed upload/download completion hooks not awaiting async plugin dispatch.

---

## v0.2.1

### New Features

- **Jump Host / ProxyJump Support** — Connect to servers through bastion hosts using multi-hop SSH tunneling. Supports chaining multiple jump hosts. Works with both saved sessions and Quick Connect.
- **SSH Config Import** — Import hosts from your `~/.ssh/config` file with a single click. Automatically resolves ProxyJump chains, IdentityFile paths, and host aliases. Cross-platform (Linux, macOS, Windows).
- **Lua Plugin System (Beta)** — Extend Reach with Lua plugins. Sandboxed execution environment with host API access for SSH commands, storage, and UI hooks. Manage plugins from the new Plugins tab in Settings.

### Improvements

- Session Editor now supports adding multiple jump hops with per-hop authentication settings
- Quick Connect supports optional single jump host for ad-hoc bastion connections
- New "Import SSH Config" button in the Sessions sidebar with host selection modal
- BETA badges on new features to set expectations

---

## v0.2.0

- Drag-drop upload stacking fix
- AI message copy button
- Various UI improvements

## v0.1.9

- Replace Ansible with native Rust playbook engine
- 8 built-in modules: shell, copy, template, apt, systemd, user, file, lineinfile

## v0.1.8

- Initial public release
