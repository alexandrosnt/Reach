# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Reach is a cross-platform SSH client and remote management tool built with **Tauri v2** (Rust backend) and **Svelte 5** (SvelteKit frontend). It provides SSH terminals, SFTP file browsing, port tunneling, playbook automation, system monitoring, serial console access, an AI assistant, and an encrypted vault for secrets.

## Development Commands

```bash
npm install                  # Install frontend dependencies
npm run tauri dev            # Run app in development mode (starts Vite + Tauri)
npm run tauri build          # Production build (outputs platform installers)
npm run dev                  # Vite dev server only (no Tauri shell)
npm run build                # Vite build only (outputs to build/)
npm run check                # SvelteKit sync + svelte-check type checking
npm run i18n:check           # Validate locale files have all keys from en.json
```

Rust backend builds are handled by Tauri CLI (`npm run tauri dev` / `npm run tauri build`). To work on Rust code independently: `cd src-tauri && cargo check` or `cargo build`.

Prerequisites: Rust (rustup), Node.js 22+, and [Tauri v2 prerequisites](https://v2.tauri.app/start/prerequisites/) for your OS.

## Architecture

### Frontend (src/)

- **Single-page SPA** — SSR is disabled (`+layout.ts` exports `ssr = false, prerender = true`), uses `@sveltejs/adapter-static` with `fallback: 'index.html'`
- **Svelte 5 runes** — all state management uses `$state`, `$derived`, `$effect` (no stores)
- **Tailwind CSS v4** — imported via `@tailwindcss/vite` plugin; design tokens defined in `src/app.css` under `@theme`
- **Global `APP_VERSION`** — injected via `vite.config.ts` `define`, declared in `src/app.d.ts`

**Key directories under `src/lib/`:**

| Directory | Purpose |
|-----------|---------|
| `components/` | UI components organized by feature (terminal, explorer, sessions, tunnel, playbook, vault, ai, settings, shared, layout, editor, snippets) |
| `state/` | Reactive state modules (`.svelte.ts` files using runes). Each module exports getter functions and mutation helpers — no Svelte stores. |
| `ipc/` | TypeScript wrappers around `invoke()` from `@tauri-apps/api/core`. Each file maps 1:1 to a Rust command module. |
| `i18n/` | Translation system using `t(key, params?)` with `{{param}}` interpolation. Locale JSON files in `i18n/locales/`. English is the base locale. |
| `data/` | Static data (e.g. distro icon mappings) |
| `utils/` | Shared utilities (formatters, virtual scroll) |

**Layout flow:** `+layout.svelte` loads settings/AI config/shortcuts/updater, wraps everything in `AppShell`. The single `+page.svelte` manages terminal tabs (local PTY and SSH), monitoring polling, and the file editor overlay.

### Backend (src-tauri/)

- **Rust with Tokio async runtime** — all SSH, SFTP, tunnel, monitoring operations are async
- **SSH via russh** (no OpenSSH dependency) — managed by `SshManager`
- **Centralized `AppState`** (`src-tauri/src/state.rs`) — holds all managers behind `Arc<Mutex/RwLock>`, passed as Tauri managed state
- **IPC commands** (`src-tauri/src/ipc/`) — one file per domain, registered in `lib.rs` via `tauri::generate_handler![]`. Desktop and mobile have separate handler registrations (desktop includes PTY and serial commands).
- **Desktop-only features** gated with `#[cfg(desktop)]`: PTY (portable-pty), serial ports (serialport), autostart, updater
- **Vault encryption** — XChaCha20-Poly1305 + Argon2id + X25519 key exchange. Sessions, credentials, and settings are stored encrypted in SQLite via libsql.
- **Data directory** — `dirs::data_dir()/com.reach.app`

### Frontend ↔ Backend Communication

All communication goes through Tauri's `invoke()`. The pattern is:
1. Frontend calls a typed wrapper in `src/lib/ipc/*.ts`
2. Wrapper calls `invoke('command_name', { params })`
3. Rust handler in `src-tauri/src/ipc/*_commands.rs` receives it, accesses `AppState`, returns result
4. SSH terminal data flows via Tauri events (not invoke) for streaming output

### State Management Pattern

State modules in `src/lib/state/*.svelte.ts` use Svelte 5 runes at module scope:
```typescript
let items = $state<Item[]>([]);           // reactive state
let active = $derived(items.find(...));    // derived values
export function getItems() { return items; }  // getter
export function addItem(item: Item) { ... }   // mutation
```
Components import these functions directly — no context providers or store subscriptions needed.

## Conventions

- **Styling:** Tailwind utility classes + component-scoped `<style>` blocks. Use CSS custom properties from `app.css` `@theme` for colors/radii/shadows.
- **i18n:** All user-facing strings should use `t('key')`. Add new keys to `src/lib/i18n/locales/en.json` and run `npm run i18n:check` to validate.
- **IPC naming:** Rust commands use `snake_case` (e.g. `ssh_connect`), TypeScript wrappers use `camelCase` (e.g. `sshConnect`). Both must be registered in `lib.rs` `generate_handler![]` for both desktop and mobile blocks.
- **Platform gating:** Use `#[cfg(desktop)]` for desktop-only Rust modules. Some Cargo dependencies are under `[target.'cfg(not(target_os = "android"))'.dependencies]`.
