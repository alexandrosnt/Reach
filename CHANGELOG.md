# Changelog

## v0.3.0

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
