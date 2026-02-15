<p align="center">
  <img src="src-tauri/icons/128x128.png" alt="Reach" width="80" />
</p>

<h1 align="center">Reach</h1>

<p align="center">
  A modern, cross-platform SSH client and remote management tool.<br>
  Built for engineers who got tired of PuTTY and wanted something that just works.
</p>

<p align="center">
  <img src="https://img.shields.io/github/v/release/alexandrosnt/Reach?style=flat-square&color=0a84ff" alt="Release" />
  <img src="https://img.shields.io/badge/platforms-Windows%20%7C%20macOS%20%7C%20Linux%20%7C%20Android-333?style=flat-square" alt="Platforms" />
  <img src="https://img.shields.io/github/license/alexandrosnt/Reach?style=flat-square?cacheSeconds=60" alt="License" />
</p>

<p align="center">
  <a href="https://alexandrosnt.github.io/Reach/"><strong>Documentation</strong></a> · <a href="https://github.com/alexandrosnt/Reach/releases">Download</a> · <a href="https://github.com/alexandrosnt/Reach/issues">Report a Bug</a>
</p>

---

<p align="center">
  <img src="assets/preview.png" alt="Reach Preview" width="900" />
</p>

---

## Why Reach?

Most SSH tools feel like they were designed in 2005, because they were. MobaXterm is Windows-only and bloated, PuTTY hasn't changed in decades, and Termius wants a subscription for basic features.

Reach is what happens when you build an SSH client from scratch with a native UI, proper encryption, and the kind of workflow you'd actually want to use every day. No Electron. No monthly fee. Just a fast, clean tool that runs everywhere.

## What's inside

### Core

- **SSH Terminal** · Full interactive shell with WebGL rendering. Tabs, split views, and resize that actually works.
- **SFTP File Explorer** · Browse remote filesystems, drag-and-drop transfers, inline editing. Feels like a local file manager.
- **Session Manager** · Save connections with folders and tags. Credentials are encrypted at rest, not stored in plaintext configs.
- **Jump Host (ProxyJump)** · Connect through bastion servers with multi-hop SSH tunneling. Import hosts directly from `~/.ssh/config`.

### Productivity

- **Playbooks** · Write YAML scripts to automate deployments and maintenance across multiple servers. Think Ansible-lite, built in.
- **Port Tunneling** · Local, remote, and dynamic SOCKS forwarding. Set it up once, save it with the session.
- **Multi-Exec** · Broadcast the same command to 10 servers at once. Handy for fleet updates.
- **System Monitoring** · Live CPU, memory, and disk stats from connected hosts without installing agents.

### Extras

- **Serial Console** · Talk to routers, switches, and embedded devices over COM/TTY.
- **AI Assistant** · Optional AI integration for command suggestions and troubleshooting (bring your own API key).
- **Encrypted Vault** · Store secrets, credentials, and SSH keys in an encrypted vault with cloud sync support.
- **Lua Plugins** · Extend Reach with sandboxed Lua scripts. Access SSH, storage, and UI hooks through the host API.
- **Auto-Updates** · The app checks for updates on startup and periodically while running. No manual downloads.

## Tech

Reach is a [Tauri v2](https://v2.tauri.app) app with a Rust backend and Svelte 5 frontend. The entire SSH stack runs natively in Rust through [russh](https://github.com/warp-tech/russh), with no OpenSSH dependency. The UI is rendered in a system webview (not bundled Chromium), so the final binary is small and memory usage stays low.

| | |
|---|---|
| **Backend** | Rust, Tokio, russh |
| **Frontend** | Svelte 5, SvelteKit, TypeScript |
| **Styling** | Tailwind CSS v4 |
| **Terminal** | xterm.js with WebGL addon |
| **Crypto** | XChaCha20-Poly1305, Argon2id, X25519 |
| **Platforms** | Windows, macOS, Linux, Android |

## Getting started

Grab the latest release from the [Releases page](https://github.com/alexandrosnt/Reach/releases). Installers are available for Windows (NSIS), macOS (.dmg), Linux (.deb, .AppImage, .rpm), and Android (.apk).

## Building from source

You'll need [Rust](https://rustup.rs), [Node.js 22+](https://nodejs.org), and the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your OS.

```bash
git clone https://github.com/alexandrosnt/Reach.git
cd Reach
npm install
npm run tauri dev
```

For a production build:

```bash
npm run tauri build
```

## Project structure

```mermaid
graph LR
  root["🗂 Reach"]

  root --> src["📁 src · Svelte frontend"]
  root --> tauri["📁 src-tauri · Rust backend"]
  root --> gh["📁 .github/workflows · CI/CD"]

  src --> routes["📄 routes"]
  src --> lib["📁 lib"]

  lib --> components["📁 components"]
  lib --> state["📄 state · Reactive .svelte.ts modules"]
  lib --> ipc["📄 ipc · Tauri command wrappers"]
  lib --> i18n["📄 i18n · Internationalization"]

  components --> layout["📄 layout · AppShell, TitleBar, Sidebar"]
  components --> terminal["📄 terminal · SSH terminal, multi-exec"]
  components --> explorer["📄 explorer · SFTP file browser"]
  components --> sessions["📄 sessions · Connection manager"]
  components --> tunnel["📄 tunnel · Port forwarding UI"]
  components --> playbook["📄 playbook · Editor and runner"]
  components --> vault["📄 vault · Encrypted secrets"]
  components --> ai["📄 ai · AI assistant panel"]
  components --> settings["📄 settings · App preferences"]
  components --> shared["📄 shared · Button, Modal, Toast"]

  tauri --> taurisrc["📁 src"]
  taurisrc --> ssh["📄 ssh · SSH client via russh"]
  taurisrc --> sftp["📄 sftp · File transfers"]
  taurisrc --> tvault["📄 vault · Encrypted storage, crypto"]
  taurisrc --> ttunnel["📄 tunnel · Port forwarding engine"]
  taurisrc --> pty["📄 pty · Local terminal (desktop)"]
  taurisrc --> serial["📄 serial · Serial port (desktop)"]
  taurisrc --> monitoring["📄 monitoring · Remote system stats"]
  taurisrc --> tplaybook["📄 playbook · Execution engine"]
  taurisrc --> tipc["📄 ipc · Tauri command handlers"]
```

## Changelog

### v0.2.2
- Fixed plugin button actions (e.g. Refresh) failing when calling async host API functions like `reach.ssh.exec()`
- Fixed plugin hooks not awaiting async Lua calls, causing `reach.ssh.exec()` to silently return errors
- Plugins now auto-load on app startup instead of requiring manual activation via Settings > Plugins
- Fixed SFTP upload/download completion hooks not awaiting async plugin dispatch

### v0.2.1
- Jump host (ProxyJump) support — connect through bastion servers with multi-hop SSH tunneling via russh direct-tcpip channels
- SSH config import — parse and import hosts from `~/.ssh/config` with automatic ProxyJump chain resolution (cross-platform)
- Lua plugin system (beta) — sandboxed Lua VMs with host API for SSH commands, storage, and UI hooks
- Session editor and Quick Connect now support jump host configuration with per-hop auth settings
- New Plugins tab in Settings for managing Lua plugins
- 19 new i18n keys across all 6 locales

### v0.2.0
- Fixed drag-and-drop file uploads stacking and freezing — uploads now run sequentially instead of flooding the SSH connection
- Fixed silent upload failures — errors now show a toast notification instead of being swallowed
- Added feedback when dragging files from browsers or apps that don't provide file paths
- Added copy button to every AI chat message (appears on hover)
- Updated docs for the native playbook engine and removed snippets page

### v0.1.9
- Replaced Ansible integration with a native Rust playbook engine — no Python or external tools required
- Playbooks use Ansible-compatible YAML and execute directly over SSH from any platform including Windows
- Supports 8 modules: shell, command, copy, file, apt, systemd/service, lineinfile, template
- Streaming output, cancellation, variable interpolation, conditional execution (when), and register support
- Save and load playbook projects from the encrypted vault
- Updated app preloader to use the actual app icon

### v0.1.8
- Added Bulgarian language
- 6 languages supported now: English, German, French, Greek, Italian, Bulgarian

### v0.1.7
- Single-instance mode, reopening the app focuses the existing window instead of opening a new one
- Fixed the auto-updater so it actually generates update artifacts and signatures
- Fixed release workflow for all platforms

### v0.1.6
- Welcome wizard on first run with language selection and optional Turso cloud sync setup
- Added German, French, Greek and Italian translations
- Language selector in Settings > General
- Language picker with flag icons during setup

## Contributing

Contributions are welcome. Bug reports, feature ideas, and pull requests all help. If you're picking up a larger feature, open an issue first so we can talk about the approach.

## License
Licensed under the MIT License.
This project is free software: you are allowed to use, modify, and redistribute it for personal, academic, or commercial purposes under the terms of the MIT license. See the [LICENSE](LICENSE) file for full details.
