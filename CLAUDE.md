# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
npm install                # once
npm run tauri dev          # run the full app (Vite dev server + Rust backend)
npm run tauri build        # production bundle for the host OS
npm run dev                # frontend only, no Tauri backend (most IPC will fail)
npm run check              # svelte-kit sync && svelte-check — the only frontend typecheck
npm run i18n:check         # verify every locale has exactly en.json's keys (CI-relevant)
```

Rust side (run from `src-tauri/`):

```bash
cargo check                # fastest backend feedback loop
cargo test                 # unit tests live in ssh/keyfile.rs, vault/{crypto,export,kdf}.rs
cargo test kdf::tests::    # single module
cargo test -- --nocapture  # see tracing output
```

Docs site is a separate Astro/Starlight project in `docs/` with its own `package.json` (`cd docs && npm install && npm run dev`).

Android builds go through `npx tauri android init` then `npx tauri android build --apk` (see `.github/workflows/release.yml`).

There is no linter or formatter configured, and no frontend test runner. `npm run check` + `cargo check` are the gate.

## Architecture

Tauri v2 app: Rust backend (`src-tauri/`), Svelte 5 + SvelteKit frontend (`src/`) built as a static SPA via `adapter-static`. All privileged work (SSH, crypto, filesystem, processes) happens in Rust; the webview only renders.

### The three-layer frontend contract

Every feature follows the same path, and new features should too:

1. `src-tauri/src/ipc/*_commands.rs` — `#[tauri::command]` handlers, registered in the `generate_handler![]` list in `src-tauri/src/lib.rs`. **A new command must be added there or it silently does not exist.** `lib.rs` has separate desktop and Android handler lists; desktop-only commands (`pty_*`, `serial_*`) are `#[cfg(desktop)]`-gated in both `lib.rs` and the module tree.
2. `src/lib/ipc/*.ts` — thin typed `invoke()` wrappers. One file per backend domain. Nothing else in the frontend should call `invoke` directly.
3. `src/lib/state/*.svelte.ts` — Svelte 5 runes modules holding app state. Components read state from here, never from IPC directly.

State modules use two styles: older ones export module-level `$state` plus getter functions (`sessions.svelte.ts`); newer ones export a singleton class instance with `$state` fields and `get` derived accessors (`vault.svelte.ts` — `export const vaultState = new VaultState()`). Prefer the class-singleton style for new state. Use `SvelteMap`/`SvelteSet` from `svelte/reactivity` for reactive collections — a plain `Map` in `$state` will not trigger updates on mutation.

### Backend shared state

`src-tauri/src/state.rs` defines `AppState`, an `Arc`-wrapped bundle of per-domain managers (`SshManager`, `VaultManager`, `TunnelManager`, `PtyManager`, `SerialManager`, `MonitoringCollector`, `AnsibleProjectManager`, `TofuProjectManager`, `PluginManager`). It is registered inside `setup()` in `lib.rs`, **not** at builder time — it must be rooted at the Tauri-resolved app data dir.

That data dir is the process-wide `APP_DATA_DIR` `OnceLock` in `lib.rs`; always call `app_data_dir()` for writable paths. Using `dirs::data_dir()` directly breaks Android, where the sandbox is not an XDG dir and every write fails with `os error 30`. Desktop deliberately keeps the legacy `…/com.reach.app` path even though the bundle id is `com.reach.desktop`, so existing vaults are never relocated.

Locking discipline matters: `ssh_connect` deliberately does the handshake/auth/shell setup **without** holding the connection-manager lock and only locks for the final registration. Holding a manager lock across slow I/O freezes every other SSH operation.

### Events (Rust → frontend)

Terminal data is not returned from commands; it streams over per-connection Tauri events named `ssh-data-{connection_id}` and `ssh-exit-{connection_id}` (see `ssh/client.rs`). Fixed-name events include `ssh-hostkey-prompt`, `app-quit-requested`, `editor-open-file`, `menu-copy`/`menu-paste`, `plugin-ui-update`, `plugin-notify`, `toolchain-install-*`.

`ssh-hostkey-prompt` is a request/response pair: the connect future blocks on the handshake until the frontend calls `ssh_hostkey_response`, and fails closed if never answered.

### Vault and crypto

`src-tauri/src/vault/` is the encrypted-storage core and the most security-sensitive area.

- Key hierarchy: an X25519 identity keypair lives in the OS keychain; a KEK is derived from it via HKDF (TLS-style) or from a password via Argon2id (`kdf.rs`: 256 MiB / 4 iterations / 4 parallelism); per-vault master DEKs are wrapped with the KEK. Content encryption is XChaCha20-Poly1305.
- Storage is libsql/SQLite, offline-first, with optional Turso sync for shared vaults (`sync.rs`, `turso_api.rs`).
- Internal vault names are constants exported from `manager.rs`: `SESSIONS_VAULT`, `CREDENTIALS_VAULT`, `FOLDERS_VAULT`, `SETTINGS_VAULT`, `PLAYBOOKS_VAULT`. Sessions, saved passwords and app settings are all rows in these encrypted vaults — not JSON config files.
- Full backup is a sealed binary `.reachbackup`: `[8B magic][2B version][32B salt][24B nonce][4B json_len][ciphertext]` (`export.rs`). Secrets are moved as ciphertext and are never decrypted during export/import.
- Invariants to preserve: credentials wrapped in `secrecy::Secret`, zeroized after use, never logged in plaintext, and the sync server only ever sees encrypted blobs.

`VAULT_IMPLEMENTATION_PLAN.md` tracks which phases of the multi-vault/sharing design are done vs. pending — read it before touching vault sharing or Turso sync.

### SSH

`src-tauri/src/ssh/` uses `russh` natively; there is no OpenSSH dependency. Auth is a single `cascade_authenticate` used by both direct and jump-host paths, trying methods in OpenSSH order (configured key → ssh-agent identities → password) via a flat `AuthParams` struct with optional fields — not exclusive enum variants. Agent support is `SSH_AUTH_SOCK` on Unix and the `\\.\pipe\openssh-ssh-agent` named pipe (Pageant fallback) on Windows. `keyfile.rs` classifies key files by content so a `.pub` file pasted into a private-key field produces an actionable error.

### Plugins

Lua (Luau via `mlua`) sandbox in `src-tauri/src/plugin/`. Plugins declare permissions in `plugin.toml`; grants are stamped with `version_at_grant` and cleared when the manifest version changes, and stored permissions are intersected with what the manifest currently declares. Hook dispatch is fire-and-forget at every call site with a per-plugin `tokio::time::timeout` — never make a plugin hook block an IPC return. The marketplace verifies SHA-256 against the registry, extracts to a staging dir with zip-slip and symlink rejection, then atomic-renames into place.

## Conventions

- **i18n is mandatory.** Every user-facing string goes through `t('key')` from `src/lib/state/i18n.svelte.ts`, and the key must be added to **all** locale files in `src/lib/i18n/locales/` (`en.json` is the base). `npm run i18n:check` fails on any missing or extra key. Locale JSON is flat, dot-separated keys with `{{param}}` interpolation.
- **Tauri capabilities.** Any new frontend use of a Tauri plugin API needs its permission added to `src-tauri/capabilities/default.json` (or `desktop.json` for desktop-only) or the call fails at runtime.
- **CHANGELOG.md** entries are written as explanatory prose — what broke, the root cause, and the fix — not one-line bullets. Match that depth. Each release note ends with the i18n key delta.
- Tailwind CSS v4 via `@tailwindcss/vite` (no `tailwind.config.js`); components also use plain scoped `<style>` blocks.
- Comments in this codebase explain *why* (the Android `os error 30` note, the zip CVE pin, the lock-free connect rationale). Keep that register; don't add comments that restate the code.

## Provenance note

This is the canonical upstream repo (`alexandrosnt/Reach`), not a fork. In 2026 a
contributor's PRs (#1 and #26, identical content merged twice) inserted third-party
promotional material that described the project as "this fork" and pointed the build
instructions at their own fork. That was reverted in `Remove third-party promotional
content`; do not reintroduce it. Clone and issue URLs belong to `alexandrosnt/Reach`.

The same contributor's other merged PRs are legitimate and should be left alone:
#27 (persist the marketplace registry URL in the settings vault) and #28 (fix issue
#25, where the password-encrypted secret key was computed at identity init but
discarded, so password unlock never survived a restart; adds `change_password` and
regression tests).

Known inconsistency: `src-tauri/Cargo.toml` declares `license = "LicenseRef-Reach-SAL"`
while `LICENSE` and `README.md` say MIT.

Pre-existing i18n gap: `zh.json` is missing the six `marketplace.registry_*` keys, so
`npm run i18n:check` fails until those are translated.
