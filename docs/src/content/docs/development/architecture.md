---
title: Project Architecture
description: How the codebase is organized.
---

Reach is split into two halves: a **Svelte 5 frontend** and a **Rust backend**, connected by **Tauri v2**.

## Frontend (`src/`)

SvelteKit with SSR disabled. It's a static SPA using `@sveltejs/adapter-static`.

**State management** uses Svelte 5 runes (`$state`, `$derived`, `$effect`) at module scope. No stores. State modules live in `src/lib/state/` and export getter functions and mutation helpers. Components import these directly.

**Styling** is Tailwind CSS v4, with custom design tokens defined in `src/app.css`.

**IPC wrappers** in `src/lib/ipc/` map 1:1 to Rust commands. They call `invoke()` from `@tauri-apps/api/core`.

**Components** are organized by feature:

| Directory | What's in it |
|---|---|
| `terminal/` | Terminal tabs, local and SSH |
| `sessions/` | Session management UI |
| `explorer/` | SFTP file browser |
| `tunnel/` | Port forwarding setup |
| `playbook/` | Playbook editor and runner |
| `vault/` | Encrypted secrets manager |
| `ai/` | AI assistant panel |
| `settings/` | Settings panel tabs |
| `setup/` | First-run setup wizard |
| `shared/` | Reusable UI bits (buttons, modals, etc.) |
| `layout/` | App shell, sidebar, title bar |
| `editor/` | File editor overlay |
| `snippets/` | Code snippet manager |

**i18n** uses a simple `t(key, params?)` function with JSON locale files. 6 languages are supported.

## Backend (`src-tauri/`)

Rust with the **Tokio async runtime**. Everything async.

**SSH** is handled by [russh](https://github.com/warp-tech/russh). Pure Rust, no OpenSSH dependency.

**AppState** (`src-tauri/src/state.rs`) is the central state holder. All managers sit behind `Arc<Mutex>` or `Arc<RwLock>` and are passed around as Tauri managed state.

**IPC commands** live in `src-tauri/src/ipc/`, one file per domain. They're registered in `lib.rs` via `generate_handler![]`.

**Desktop-only features** like PTY, serial ports, autostart, and the updater are gated with `#[cfg(desktop)]`.

**Vault encryption** uses XChaCha20-Poly1305 for encryption, Argon2id for key derivation, and X25519 for key exchange. The database is libsql (SQLite-compatible, with an optional Turso remote sync option).

**Data directory**: Everything is stored in the OS data directory. On Windows that's `%APPDATA%/com.reach.app`, on macOS `~/Library/Application Support/com.reach.app`, and on Linux `~/.local/share/com.reach.app`.

## How the two halves talk

The frontend calls `invoke('command_name', { params })` which hits a Rust handler on the backend. That's it for request/response stuff.

For streaming data (terminal output, file transfer progress, monitoring stats), Tauri **events** are used instead of invoke. The backend emits events and the frontend listens for them.

All commands return `Result<T, String>`. Errors come back as plain strings.
