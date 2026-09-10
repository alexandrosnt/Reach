# Changelog

All notable changes to Reach are documented here.

## v0.5.3
- **A recipe's declared risk now counts** — The registry card showed the level an author declared; the installed card showed the level the pattern scan found; and for `harden-ssh` those disagreed, because the config path sits in a variable and `sed -i "$CONFIG"` looks harmless to a scanner that cannot see through it. The recipe read as Sensitive before install and Benign after — and ran without the typed confirmation its author asked for. The level Reach shows and gates on is now the worse of the two. The scan still reports what it saw on its own, an author who understated the risk is still called out, and when the author rated a recipe higher than any line the scanner could see, the run dialog says that is where the rating came from rather than putting "Nothing risky found" under a Sensitive badge.
- 1 new i18n key across all 9 locales.

## v0.5.2
- **Session sharing** — Share a terminal with another person, peer to peer, with no server of ours anywhere in the path. Left pane is always yours; theirs appears beside it while they share one, sized to *their* terminal rather than to the pane, because a mirror with a different column count renders every wrapped line wrong. Each side chooses what the other may do to its own terminal — read only, or read and write — and that choice is enforced on the machine that owns the terminal, never on the claim the other side makes about it. The handshake is two pastes: a code out over any chat you already trust, and a code back. The transport is the webview's own WebRTC, so there is no Rust WebRTC stack to carry. On top of DTLS, every frame is sealed with AES-256-GCM under a key derived from the share token, with direction-tagged nonces so two peers on one key can never collide, and a reply sealed with that key is the proof the guest actually had the token. Off by default, and off is absent rather than disabled: the control does not render, and the production build confirms the sharing code sits in a chunk that no entry bundle references. Turning it on loads nothing either — the first packet leaves when you click Share. Connection servers are yours: STUN defaults from two public operators, no TURN because no free public one is worth trusting, bring your own with credentials kept in the vault, or clear the list for sharing that never contacts anything outside your network. Honest limit: roughly 15–30% of NAT pairs cannot hole-punch, and those get told so, and what fixes it, instead of a silent fallback through a stranger's relay.
- **MCP server** — Expose a session you choose to an AI client over the Model Context Protocol (Streamable HTTP, JSON-RPC 2.0), bound to `127.0.0.1` only and off until you turn it on. Four agents — Linux Administrator, DevOps Engineer, DevSecOps Engineer, and a read-only Architect — set which tools a client gets, and a per-agent policy can only tighten the compiled-in guard, never loosen it; a test asserts the policy struct's field count so a permissive knob cannot be added quietly. The guard enforces rather than advises: read before write, describe before act, a lockout while terminal echo is off because someone is typing a password, a mandatory rationale, a rate limit, and a refusal for any command that carries a secret. A refusal is a tool result, never a protocol error, and a stale-view refusal hands back the output the client missed — the single largest latency win in the design. Modes — Ask, Auto (safe), Auto, and Dangerous, which runs everything and says so — are chosen from the bottom bar, per session if you like, and every setting including the token now persists in the vault rather than being regenerated on each launch. A setup wizard gives the exact connect command for eleven clients with their brand marks; each spells its config differently, and the differences were recorded from their documentation rather than guessed. Commands the AI types are echoed into your terminal under a dim banner, so nothing happens off-screen.
- **Recipes** — Bash scripts that carry their metadata in leading `#` comments, so a recipe stays an ordinary script: `bash recipe.sh` runs it and shellcheck reads it. That matters because a recipe is arbitrary code aimed at a production server, and a format only Reach can execute is a format nobody can audit. Parameters arrive as single-quoted environment variables — the entire boundary between a value and a command. Execution is one heredoc through `bash -s`, not line-by-line typing: `set -euo pipefail` works and the exit status means something, at the cost that a recipe cannot prompt, so it takes parameters instead. Risk analysis borrows the MCP guard's classifier rather than growing a second list of destructive patterns that would drift; the run dialog names the target machine, quotes the offending lines, flags anything that downloads and executes code (which nothing can see inside), and points out when an author declared less risk than the script shows. Above "mutating", the confirmation is typed, not clicked. A registry follows the plugin pattern with a SHA-256 pin and ships with three recipes; the test that parses them caught one understating its own risk before it shipped.
- **You are told what is new, once** — The gear, the Recipes rail icon and new settings tabs take the accent colour while they hold something you have not been shown, with a NEW badge where a word fits and a dot where it does not. Nothing is dismissible: an indicator clears when you have been shown the thing, because a dismiss button is one more click before the feature it is pointing at. A first-ever launch marks everything seen — all of it is new to you, and highlighting three arbitrary corners is noise rather than news.
- **Community** — A Discord for Reach, with a permanent invite baked into the app. The nudge to join holds itself back until the third launch, "Not now" defers it three weeks, the checkbox ends it for good, and Settings → General keeps a permanent link so ending it loses nothing. Repo tooling under `scripts/discord/` scaffolds the server — roles, channels, permissions, and the GitHub release and activity feeds — from a plan file, idempotently, with a mocked suite that caught its own test run overwriting the live webhook file.
- **MCP settings you can read** — Shared sessions were listed by UUID; they now show `root@10.144.144.2`, on a line of their own so the agent and mode dropdowns cannot truncate the name. Regenerating the token asks first, since it locks out every configured client, and the hint no longer claims the token is new on every start.
- 160 new i18n keys across all 9 locales, 1168 keys per locale, `npm run i18n:check` clean.

## v0.5.1
- **Spanish** — Reach now ships in Spanish, contributed by [CarlosTohe](https://github.com/CarlosTohe) in #41. Nine locales, 1008 keys each, `npm run i18n:check` clean.
- **A vault that cannot be reached no longer empties the vault panel** — refreshing failed outright with `Failed to refresh: Database error: Hrana `ap`, and every vault vanished from the list, including local ones that were perfectly healthy. A shared vault is a remote-only Turso connection, so the `SELECT COUNT(*)` used to render "N secrets" is a network round-trip; one expired token or paused database failed the whole listing. An unreachable vault is now reported as unreachable and everything else lists normally. Opening one behaved the same way, which was worse — it left a vault you hold a valid key for permanently unopenable.
- **Errors you can actually read** — libsql renders a rejected request as ``Hrana: `api error: `<http body>```, and the toast clipped to one line inside a 360px pill, so the reason was cut off after ten characters with no way to see the rest. Error and warning toasts now wrap to four lines, and Turso's response is unwrapped into a plain sentence with a hint for the two failures that actually happen: a token no longer accepted, and a database that is gone.
- **The setup wizard is a wizard again, not a wall of flags** — the first screen a new user saw was a grid of flag cards: fine at four languages, 400px tall at eight, and taller with every translation contributed. Language is now one dropdown row — flag, endonym and English name — with type-to-search across all three, so "Deutsch", "german" and "de" all find German, and matching folds accents so "francais" finds Français. Costs the same height at nine locales as at ninety.
- **Theme and plugins during setup** — the wizard now picks a theme, shown as swatches painted in each theme's own colours rather than a list of names, because that is how anyone actually chooses one. Marketplace themes appear in their own section below the built-ins and install in a single click. A plugins step offers the plugin marketplace; the registry is empty today, and since an empty registry and an unreachable one look identical to a first-time user, neither is treated as an error — both get the same calm empty state and the wizard never blocks on the network.
- **Nothing in setup grows without bound** — both registries are public and have no size limit. Against a synthetic 2,002-theme registry the personalize step measured 972px inside an 882px window, with the primary button below the fold. Marketplace themes are now a single horizontally scrolling strip — one card tall whether the registry holds two themes or two thousand — and the card is capped at the window height with the step body scrolling and the actions pinned. Verified from 820px down to 320px tall. Settings → Appearance, which rendered the entire registry one row at a time, now searches and pages.
- **Installed themes apply on launch** — `theme_list_installed` had exactly one caller: Settings → Appearance. Until you opened that tab the engine held only the two built-ins, so a theme you had installed did not apply at startup and appeared in no picker. It is now loaded with the rest of the app state.
- **Scrollbars follow the theme** — the last of the hardcoded `rgba(255,255,255,x)` overlays the v0.5.0 migration replaced everywhere else. An installed theme restyled the whole app and then got Reach's own scrollbar, which is exactly the seam a theme exists to remove. Firefox, which has no `::-webkit` pseudo-elements, had no scrollbar styling at all and now does.
- **Android builds again** — every Android release build was failing on `aarch64-linux-android-ranlib: not found`. `openssl-sys` was scoped to `cfg(all(unix, not(target_os = "macos")))` under a comment claiming it was Linux-only; Android is also unix and also not macOS, so it matched and forced a vendored OpenSSL build against the NDK, which ships `llvm-ranlib` rather than the prefixed binutils. Now scoped to `target_os = "linux"`, with the Linux need and the Windows fix from v0.5.0 both intact.
## v0.5.0
- **Themes** — Reach now has a real theme system, and a marketplace to install more. A theme is data rather than code: 16 UI tokens plus the terminal's 22 colours, in a single JSON file. Nothing executes, so unlike plugins there is no sandbox, no permissions and no code review burden — the worst a bad theme can do is look wrong. Installs verify the SHA-256 before anything touches disk, cap the file at 256 KB, and require every one of the 38 keys: a theme missing one would leave part of the app unstyled, which reads as an app bug rather than a theme bug, so it is refused outright. Installed themes appear in Settings → Appearance alongside the built-in Reach Dark and Reach Light, each previewed in its own colours.
- **Themes actually reach the whole app** — Making the above real meant fixing why it could not work. Around 350 colour values across 70 components bypassed the design tokens entirely (202 `rgba()` and 45 hex literals), so any theme would have repainted roughly two thirds of the app and left the rest dark. Those are now migrated: backgrounds, borders, overlays and shadows all resolve through tokens, and a theme reaches the title bar, tab bar, sidebar, panels, buttons, search, session rows, status bar and the terminal. Deliberately *not* themed: the national flag colours in the language picker, distro brand colours, and OpenTofu graph series — those are meant to be fixed.
- **The terminal follows the theme** — its 22 colours were hardcoded in JavaScript, set once at construction with no reactivity. As a result light mode left the terminal at `#0a0a0a`: a black rectangle filling most of an otherwise white app. The palette now comes from the active theme and is pushed into every live terminal when it changes.
- **A design system to build against** — there wasn't one. A palette, three radii and two shadows, with every component inventing its own spacing; the app measured 12 distinct font sizes and 8+ padding values, with 11px body text and 9px badges. Added a type scale (the floor moves from 9px to 11px, body from 11px to 13px), a spacing scale on a 4px grid, four surface levels and a third text level, all inverted for light mode.
- **The session list is readable again** — session names rendered as `prod…` and hosts as `root@…` at *every* window size, including a 1280px laptop. Of a 209px row the name got 38px; the rest was chrome, and the three action buttons were most of it. They were already meant to be hover-only, but were hidden with `opacity: 0`, which does not give their width back — so they reserved 76px of every row, permanently and invisibly. They are now taken out of flow and fade in over the row's right edge, and stay in place under `pointer: coarse` where there is no hover.
- **Navigation no longer competes with content** — the sidebar stacked its five nav buttons above the active panel in one column, so they fought for the same height. A 601px sidebar spent 415px on chrome and left 186px for connections, about four rows, and every tool added would have taken another slice. Navigation is now a 48px rail beside the panel: it costs zero vertical space, the panel gets the sidebar's full height, and the rail keeps showing the active tool even when collapsed, which the old collapsed state could not.
- **Usable on a phone** — a 240px sidebar beside the terminal left the terminal 149px of a 390px screen, 62% of the display spent on navigation. Below 700px the sidebar now overlays as a drawer and the terminal gets the full width. Terminal tabs compress rather than overflowing, the shell uses `100dvh` so the status bar is not pushed under the on-screen keyboard, fixed chrome trims on short screens, and touch targets grow under `pointer: coarse`.
- **Smaller UI fixes** — "Import SSH Config" wrapped onto three lines and made all three session actions 61px tall; they are now uniform single-line rows with Quick Connect as a clear primary. The terminal's empty state had a grey heading on a grey ground and only *described* the action; it now has a real "New Tab" button and a legible title.
- **Retired the BETA and NEW badges** — jump hosts, SSH-config import, snippets and plugins have shipped across several releases. A badge that never comes off stops meaning "this is new" and starts reading as "this is unfinished". The Marketplace tab keeps a NEW badge, since it genuinely is.
- 8 new i18n keys for the theme marketplace, plus the marketplace registry strings that had only ever landed in three locales. All 8 locales now carry all keys and `npm run i18n:check` passes, which it previously did not.

## v0.4.9
- **SSH connects no longer block each other or the rest of the app** — `ssh_connect` used to hold the global connection-manager lock across the entire handshake, authentication and shell setup, so a slow or hanging connect froze every other SSH action (send, resize, disconnect, list) and forced connects to run one at a time. Connecting is now lock-free: the slow work happens without any lock and the manager is locked only for the final registration, so connects run concurrently and never stall the UI.
- **Host-key verification with a man-in-the-middle warning** — Reach now asks before trusting a server's key instead of silently accepting it. The first time you reach a host, a "Verify host key" dialog shows its fingerprint and key type to confirm; if a previously-trusted host suddenly presents a *different* key you get a prominent "host key changed — possible MITM" warning with the old and new fingerprints side by side. The connection waits safely on the handshake until you accept or reject (and fails closed if left unanswered). This replaces the previous accept-on-change behavior.
- **No more lost login banners or first prompt** — The session now captures server output from the instant the channel opens and replays it once the terminal is attached, so the MOTD, login banner and first shell prompt are never dropped on fast connections.
- **Configurable, shell-aware terminal colors** — The post-login color setup is now an "Auto shell colors" toggle (on by default) and is hardened: it detects GNU vs BSD `ls`, guards `dircolors`, and stays quiet on shells that don't support it — no stray output or errors on unusual shells.
- **Terminal clipboard overhaul** — Selecting text no longer auto-copies and clobbers your clipboard. Copy and paste now use the OS clipboard through the system clipboard manager (no web popup): right-click copies the selection, or pastes when nothing is selected, and pasting multiple lines asks for confirmation first so you can't accidentally fire off a block of commands. Terminal links open in your default system browser.
- **Window close behavior, done right** — Closing the window (X / Alt+F4 / Cmd+Q) now reliably minimizes to the tray when that option is enabled instead of sometimes quitting, and the tray "Quit" now also warns you when SSH sessions are still active. The close-vs-quit decision lives in a single place so the two paths can't disagree.
- **System fonts with a CJK-aware preview** — The terminal font picker now lists installed system fonts (plus a custom-font field) instead of bundled web fonts, and the live preview includes CJK samples so you can see fullwidth/halfwidth rendering before committing.
- **Multi-cursor and column selection in the editor** — The file editor now supports multiple cursors (Alt-click, Ctrl-D to add the next occurrence) and rectangular/column selection (Alt-drag), aligned correctly across fullwidth CJK characters.
- **Smarter right-click menus** — Context menus in the session list and file manager now flip up or to the left near a screen edge so they're never clipped, the "Move to folder" list scrolls when you have a lot of folders (about five visible at a time), and clicking anywhere dismisses the menu without that click also selecting or connecting whatever was beneath it.
- **Collapsing the sidebar keeps your sessions loaded** — Collapsing and reopening the sidebar no longer re-reads every session from the encrypted database; the list stays in memory.
- **Faster, draggable startup splash** — The splash screen can now be dragged (so you can move the window while it loads) and disappears the moment the app is actually ready rather than after a fixed delay.
- 18 new i18n keys added across all 7 locales. 975 keys per locale.

## v0.4.8
- **Ctrl+Shift+Tab now selects the _previous_ tab** — Ctrl+Tab (next) / Ctrl+Shift+Tab (previous) is the universal tabbed-app convention, but Ctrl+Shift+Tab was navigating forward like Ctrl+Tab. The shortcut matcher had a `key !== 'Tab'` special-case that let the no-Shift "Next tab" binding greedily match Shift+Tab too, and since it is first in the list the loop returned before reaching "Previous tab" (which was therefore dead code). Removed the exception so the Shift guard applies uniformly to every shortcut.
- **Quick Connect catches wrong key files and suggests the right one** — Pasting an OpenSSH _public_ key (`id_ed25519.pub`) where a private key was expected produced a cryptic "Key load error" mentioning spaces (russh chokes on the public key's whitespace), which read like a "no spaces in filenames" bug. A new content-based classifier (`ssh/keyfile.rs`) recognizes public keys, private keys (PEM/PKCS#8/OpenSSH), and non-keys. The key-path field now inspects the file as you type and, for a public key, shows a clear warning plus clickable chips of the actual private keys found in the same folder (the matching de-`.pub` sibling first). Connect-time errors are likewise classified ("…is an OpenSSH public key, not a private key. Use … instead"), including "not found" and "passphrase-protected?" cases. Applies to the main and jump-host key fields.
- **Per-session login shell** — A new optional "Login shell" field (Quick Connect + saved sessions) runs a specific shell as a login shell instead of the remote default — e.g. `fish` (run as `fish -l`) or `fish -l` verbatim. This also fixes a long-standing fish bug: Reach injected a bash/POSIX color-setup snippet (`export`, `$(…)`, `if…then…fi`) after login that fish can't parse, throwing a syntax error on every connect. The post-login init is now shell-aware — fish gets a fish-native init, bash/zsh/sh/dash/etc. get the POSIX one, unknown shells get nothing. Persists encrypted on saved sessions; carries through jump hosts and reconnects.
- **Close confirmation for active SSH sessions** — Closing the window (X / Alt+F4 / Cmd+Q) or an auto-update restart no longer silently kills live connections. When SSH sessions are active you get a confirmation ("You have N active connection(s). Closing now will terminate them…"); updates additionally offer **Postpone** (the staged update applies on next launch, surfaced by a "restart to apply" banner). With no active sessions, closing/updating stays silent. The updater no longer self-relaunches — it hands off to a confirm step that counts live connections first. (Tray "Quit" and backup-restore restart remain deliberate, un-prompted exits.)
- **Android: "Initialize" no longer fails with "Read-only file system (os error 30)"** — The vault resolved its storage directory from `dirs::data_dir()`, which on Android points at a non-writable path (the OS sandbox isn't an XDG/known dir), so writing `vault_identity.json` failed. All storage (vault, SSH `known_hosts`, tools dir) now routes through one app-data-dir resolved from Tauri's path API on mobile, while **desktop keeps its existing `…/com.reach.app` location unchanged** (the bundle id is `com.reach.desktop`, so existing desktop vaults are never relocated). Note: Android still lacks an OS keychain backend, so vault auto-unlock across launches remains a separate follow-up.
- **Android: setup wizard is reachable on small screens** — On phones the first-run wizard centered its content with no scrolling, so on the language step the 7 language cards pushed the "Next" button off-screen (and the top slid under the status bar) — you couldn't get past it. The overlay now scrolls when taller than the viewport and respects safe-area insets; added `viewport-fit=cover` and safe-area padding to the main app shell so the title/status bars clear the device status & navigation bars.
- 17 new i18n keys added across all 7 locales (6 key-file, 2 login-shell, 9 close/update-confirm). 957 keys per locale.

## v0.4.2
- **Plugin hooks no longer hang SSH connect** — Loading a plugin that subscribed to `session:connected` and did slow work (e.g. `reach.http.get` to a sluggish endpoint) used to stall the entire `ssh_connect` IPC return until the plugin finished. Hook dispatch is now fire-and-forget at every call site (SSH connect/disconnect, tunnel start/stop) — IPC returns instantly, plugins run in the background. Each plugin's hook is wrapped in `tokio::time::timeout` (default 5 s, manifest-overridable via `hook_timeout_ms`, hard-capped at 30 s); on timeout or error the plugin flips to `Error` status and a toast surfaces which plugin misbehaved.
- **Luau interrupt cap for runaway plugin code** — A `while true do end` in a plugin no longer starves the runtime. The sandbox now installs a Luau interrupt callback that aborts the VM if execution exceeds 10M interrupt ticks per call, with a backup wall-clock timeout for awaits.
- **Plugin HTTP client now has a 30 s request timeout** — `reach.http.get/post` previously used `reqwest::get()` with no timeout; a stuck server could hang a plugin indefinitely. Replaced with a shared `OnceLock<reqwest::Client>` built with `.timeout(30s)`.
- **Plugin permissions re-prompt on version upgrade** — `PluginConfig` now stamps `version_at_grant` when permissions are granted; on load, if the manifest version differs, granted permissions are cleared so the user must re-confirm what the new version is allowed to do. Stored permissions are also intersected with what the manifest currently declares (stale permissions auto-dropped).
- **Plugin marketplace** — New "Marketplace" tab in the Plugins panel. Fetches a registry JSON (default `alexandrosnt/reach-plugins-registry`, override-able via `marketplace_set_url` IPC), lists available plugins with description / author / declared permissions, and installs in one click. Each install: downloads the release zip (60 s timeout, 16 MB hard cap), verifies SHA-256 against the registry entry, extracts to a staging dir with zip-slip + symlink protections, validates `plugin.toml` is at the root, then atomic-renames into `~/.reach/plugins/{id}/` and loads the plugin disabled. Trust model: the user owns the registry repo and merges PRs to admit plugins; that merge is the gatekeeping action.
  - `zip = "2.3"` pinned to avoid CVE-2025-29787 (symlink-based path traversal in older zip crate releases).
  - Defense-in-depth in `extract_zip_safely`: rejects any `is_symlink()` entry outright, validates each path via `enclosed_name()`, then canonicalizes the parent and confirms it `starts_with(staging_root)`.
- **Shared `<Button>` defaults to `type="button"`** — Bare `<button>` elements inside a `<form>` default to `type="submit"`. Previously the shared Button component had no `type` set, so any `<Button onclick={...}>` placed inside a form (e.g. the SSH key Browse picker added in v0.4.1) would also submit the surrounding form on click. Defaulted to `"button"`; submit behavior is now opt-in via the new `type` prop.
- 13 new i18n keys added across all 7 locales (`plugin.hook_failed`, `plugin.tab_installed`, `plugin.tab_marketplace`, plus 10 `marketplace.*` keys). 940 keys per locale.

## v0.4.1
- **Terminal width fix on tab switch** — Fixes a bug where switching away from a tab with active output and back left the terminal rendering in a ~2-character-wide stripe. Root cause: `FitAddon.fit()` clamps to its 2×1 minimum when called against a `display:none` container; the ResizeObserver would fire on the hidden tab and resize the remote PTY to 2 columns, baking CRLF-wrapped 2-char lines into the buffer that couldn't be reflowed back. Fixed by routing all fit call sites (initial mount, ResizeObserver, Ctrl+Wheel zoom, font-family change, tab activation) through a `safeFitAndResize()` helper that bails when `clientWidth === 0` and only sends `sendResize()` when cols/rows actually changed. Particularly noticeable with TUIs that use full terminal width — though TUIs like htop self-heal on SIGWINCH and previously masked the bug; plain stream commands (`yes`, `tail -f`) showed the damage clearly.
- **File picker for SSH private key path** — Session editor now has a "Browse" button next to the private key path input. Opens the OS-native file dialog with filters for common SSH key file types (`pem`, `key`, `ppk`, `rsa`, `ed25519`, `ecdsa`, `dsa`) plus an "All Files" fallback for unextensioned keys. Available for both the main session and every jump-host hop. Implementation routes through `@tauri-apps/plugin-dialog`'s `open()`. Mirrors the existing path-picker pattern from the Ansible project editor.
- 3 new i18n keys added across all 7 locales (`session.browse_key`, `session.select_key_file`, `session.ssh_private_key_filter`).

## v0.4.0
- **OpenSSH-style cascading auth** — SSH connect now tries methods in OpenSSH order: configured key → ssh-agent identities → password. The first method the server accepts wins. Replaces the previous one-method-only model where you had to pick the right auth upfront and re-edit the session if it failed.
- **SSH agent integration** — Reach now talks to the local SSH agent and tries every loaded identity. On Windows, connects to OpenSSH's `\\.\pipe\openssh-ssh-agent` named pipe with Pageant as fallback; on Unix uses `SSH_AUTH_SOCK`. If the configured key gets rejected, every agent identity is offered to the server before giving up — same behavior as `ssh user@host` from a terminal.
- **Password fallback prompt** — when the configured key and every agent identity get rejected, the connect modal stays open and shows a password input ("Key was rejected by the server. Enter your password to try again.") instead of failing outright. Type the password, click "Try with password", connection retries with password auth.
- **Tab keyboard focus on switch** — switching SSH tabs via Ctrl+Tab or mouse click now auto-focuses the terminal so you can type immediately. Previously required a second click inside the terminal area. Focus runs at three timings (sync, next frame, microtask after) to defeat races with click handlers and WebView layout. Cross-platform: same behavior on Windows WebView2, macOS WebKit, Linux WebKitGTK.
- **`~` path expansion for SSH keys** — `~/.ssh/id_ed25519` in the key path field now resolves to the user's home directory on Windows / macOS / Linux. Whitespace is also trimmed so trailing spaces don't break key loads.
- **AuthParams refactored to a flat struct** — replaces the three exclusive `Password | Key | Agent` enum variants with optional fields (`key`, `password`, `allow_agent`). One `cascade_authenticate` function used by both direct and jump-host paths — no duplicated auth logic.
- **Better SSH error messages** — "Authentication failed" now reads "Authentication rejected — server did not accept the public key (not in authorized_keys?) or password is wrong", and detailed `tracing::info!` lines for every auth step so debugging a connect failure shows exactly which method was tried and what the server said.
- **Quick Connect tabs are now reconnectable** — Quick Connect now stores `sshConnectParams` on the tab so the reconnect overlay works for ad-hoc connections too, not only saved sessions.
- 3 new i18n keys added across all 7 locales (`session.fallback_to_password`, `session.try_with_password`, `session.password_required`).

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
