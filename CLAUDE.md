# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
npm install                  # once
npm run tauri dev            # run the app (starts vite on :1420, then the Rust shell)
npm run dev                  # frontend only, in a browser — no Tauri IPC, most of the app will not work
npm run tauri build          # production bundle

npm run check                # svelte-check — the only frontend lint/typecheck there is
npm run i18n:check           # every locale must have exactly en.json's keys

cd src-tauri && cargo test --lib          # 44 tests, all in-crate #[cfg(test)] modules
cd src-tauri && cargo test --lib kdf::    # a single module
cd src-tauri && cargo test --lib -- --nocapture derives_a_kek   # a single test
```

`cargo` may not be on `PATH` in a fresh shell; it lives at `~/.cargo/bin`.

Before claiming a change is done, run all three: `cargo test`, `npm run check`, `npm run i18n:check`.
Commit messages in this repo record their results (`cargo test 39 passed, svelte-check 0 errors,
i18n-check valid`) and explain *why* a change was made, not just what — match that.

There is no CI for tests. `.github/workflows/` only builds releases on a `v*` tag and deploys
`docs/` (a separate Astro/Starlight site with its own `package.json`) to Pages.

## Architecture

Tauri v2. Rust backend (`src-tauri/`), Svelte 5 + SvelteKit frontend (`src/`), talking over Tauri IPC.
SvelteKit is in SPA mode (`adapter-static`, `ssr = false`, one route) — it is a bundler and a router,
not a web framework here. The SSH stack is native Rust via `russh`; there is no OpenSSH dependency.

### The IPC seam

Every backend capability is a `#[tauri::command]` in `src-tauri/src/ipc/<area>_commands.rs`, wrapped
one-to-one by a typed async function in `src/lib/ipc/<area>.ts`. Frontend code calls those wrappers,
never `invoke()` directly.

**Commands are registered in two separate `generate_handler!` lists in `lib.rs`** — one under
`#[cfg(desktop)]`, one under `#[cfg(not(desktop))]` for Android. A new command added to only one of
them compiles fine and then fails at runtime on the other platform. Add it to both.

Most commands use `#[tauri::command(rename_all = "snake_case")]`, so the TS wrapper passes snake_case
argument names; return types are `#[serde(rename_all = "camelCase")]`, so the TS interface is
camelCase. Check the specific command rather than assuming.

Streaming (terminal output) does not go through return values. The backend emits per-connection
events named `ssh-data-<connection_id>` / `ssh-exit-<id>` (`ssh/client.rs`) and `pty-data-<id>`, which
`Terminal.svelte` subscribes to. Because listeners attach after the connection opens, the backend
buffers early output (motd/banner) and flushes it when the frontend calls `ssh_ready`.

### State

Frontend state is Svelte 5 runes in `src/lib/state/*.svelte.ts` — plain modules exporting `$state`
and getter functions. There are no `svelte/store` writables anywhere; do not introduce them.

Backend state is a single `AppState` (`state.rs`) holding the per-subsystem managers (`SshManager`,
`VaultManager`, `TunnelManager`, `PluginManager`, …). It is registered inside `setup()`, not on the
builder, so it can be rooted at the Tauri-resolved app data dir — `dirs::data_dir()` returns a
non-writable path on Android. Use `app_data_dir()` from `lib.rs` for any path, never `dirs`.

### Vault — the part most likely to surprise you

Nearly all persistence is the vault, not files. Sessions, folders, credentials, snippets, settings,
Ansible/OpenTofu projects and playbooks are all encrypted secrets inside *internal* vaults with
reserved `__name__` identifiers (`SESSIONS_VAULT`, `SETTINGS_VAULT`, … in `vault/manager.rs`).
`list_vaults` deliberately hides anything whose name starts with `__`. Only UI preferences that must
survive a locked vault (theme, font size, locale) live in `localStorage`.

Crypto: Argon2id (256 MiB, t=4, p=4) derives a KEK from the master password; the KEK wraps a
per-vault DEK; secrets are sealed with XChaCha20-Poly1305; X25519 identity keypairs drive vault
sharing between users. `vault/crypto.rs`, `kdf.rs`, `sharing.rs`.

Storage is libsql. **A private vault is a local SQLite file; a shared vault is a *remote-only* Turso
connection** (`Builder::new_remote` in `vault/sync.rs`, chosen to dodge a Windows embedded-replica
bug). That means every read on a shared vault is a network round-trip that can fail with a
`Hrana` error — an expired token, a paused database, no network. Code that queries a vault must not
assume the query is cheap or infallible; degrade the affected vault rather than failing the operation
(`list_vaults` and `open_vault` show the pattern, and `error::describe_db_error` turns libsql's
nested-backtick Hrana blob into something a user can read).

### Theming

A theme is data, never code: 16 UI tokens plus a 22-colour terminal palette (`state/theme.svelte.ts`,
`src-tauri/src/theme/`). Applying one writes CSS custom properties onto `:root` and pushes the
palette into xterm separately, since xterm keeps colours in JS.

**Style components with `var(--color-*)` tokens only.** A hardcoded colour or a bare
`rgba(255,255,255,.06)` overlay silently breaks light mode and every installed theme — undoing that
migration was the bulk of v0.5.0. Tokens are defined in `src/app.css` (`@theme`), including type,
spacing (4px grid), radius and shadow scales.

### Plugins and themes are installed from registries

Both fetch a PR-gated index JSON from a GitHub repo. Plugins are sandboxed Luau (`mlua`) with
dangerous globals deleted, a 64 MB memory cap and an interrupt-based instruction limit
(`plugin/sandbox.rs`), and subscribe to hook events like `session:connected` (`plugin/hooks.rs`).
Themes carry no code, so installation only verifies a SHA-256 and validates that every token is
present before anything reaches disk.

### Platform splits

`#[cfg(desktop)]` gates PTY, serial, tray, autostart, single-instance and the updater; Android has
none of them. On Windows, Ansible and OpenTofu are transparently re-invoked through `wsl.exe` when
the binary is not native, with path translation via `toolchain::detect::windows_to_wsl_path`.

## Conventions

- **i18n is not optional.** User-visible strings go through `t('key')` from `state/i18n.svelte.ts`,
  and the key must be added to **all 8** locale files (`en` is the base; `i18n:check` fails on a
  missing or extra key in any other). Insert keys surgically — do not reformat a locale JSON.
- Frontend uses tabs; Rust uses rustfmt defaults (4 spaces).
- Comments here explain the reason a thing is the way it is, especially where the obvious approach
  was tried and failed. Preserve those when editing nearby, and write new ones in the same register.
- `VAULT_IMPLEMENTATION_PLAN.md` is a design document for the sharing/identity model, not a record of
  what is built. Verify against `vault/` before trusting it.
