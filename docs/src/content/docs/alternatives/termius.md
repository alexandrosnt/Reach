---
title: "A free, open source Termius alternative"
description: "Reach is a free, MIT-licensed alternative to Termius for Windows, macOS, Linux and Android: SSH, SFTP, tunnels, RDP and VNC, containers and Kubernetes, and encrypted sync through your own database. No subscription, no account."
---

[Termius](https://termius.com) is a polished, cross-platform SSH client. Its free Starter plan covers SSH, SFTP and port forwarding on one device; syncing hosts and keys between devices starts with Pro at $10 a month billed yearly, and Team and Business are $20 and $30 per seat a month (Termius pricing page, November 2026).

Reach does that everyday work for free, with everything included and the source open, and goes well past SSH.

## At a glance

| | Reach | Termius |
| --- | --- | --- |
| Price | Free, everything included | Starter free; Pro $10/month; Team $20 and Business $30 per seat/month |
| Licence | Open source (MIT) | Proprietary |
| Account | None | Needed for sync |
| Sync between devices | Free, through your own [Turso](/sync/what-is-turso/) database, encrypted before it leaves the device | Pro and above, through Termius's cloud |
| Platforms | Windows, macOS, Linux, Android | Windows, macOS, Linux, Android, iOS |
| SSH, SFTP, port forwarding | Yes | Yes |
| Jump hosts | [Yes](/features/jump-hosts/), and every ssh_config keyword | Yes |
| Remote desktop | [RDP and VNC](/features/remote-desktop/) in a tab | Requested on Termius's roadmap |
| X11 forwarding | Yes; on Windows Reach [sets up the X server](/settings/general/#x11-server) | Requested on Termius's roadmap |
| Kubernetes | Yes, with Docker, Compose and Helm, over the SSH session | Requested on Termius's roadmap |
| Serial console | [Yes](/features/serial/) (desktop) | Yes |
| Shared vaults for a team | [Yes](/vault/sharing/), free | Paid plans |
| Live terminal sharing | [Yes](/features/session-sharing/), free, peer to peer, end-to-end encrypted | Multiplayer, paid plans |
| Telemetry | [None](/privacy/) | See Termius's privacy policy |

## Also in Reach

- **Logins and host keys as OpenSSH does them:** FIDO2 security keys (`sk-ssh-ed25519`), smart cards and tokens (PKCS#11), Kerberos (GSSAPI), certificates and host-based login; host keys checked against DNS (SSHFP, with DNSSEC validated), connection sharing (ControlMaster) and `ssh -w` tunnels, all from your [ssh_config](/features/jump-hosts/).
- **A vault that guards itself:** unlock with a YubiKey or other FIDO2 key, with Windows Hello, Touch ID or an Android fingerprint, or a master password. It locks when your computer locks. While it is open, its keys stay encrypted in memory and are never swapped to disk, and other programs are kept from reading Reach's memory.
- **Around the terminal:** tab groups and split panes, a command palette, session logs to a file, local terminals and WSL distributions, a built-in editor, compress and extract on the server from the [file browser](/features/file-explorer/), and log files inside containers and pods.
- **Getting there:** SOCKS4, SOCKS5 and HTTP CONNECT proxies (Tor included), jump-host chains and ProxyCommand.
- **In your language:** English, Bulgarian, Chinese, Czech, French, German, Greek, Italian, Russian and Spanish.
- **DevOps in the same window:** Kubernetes, Docker, Compose and Helm over the SSH session (mark a cluster or host Production and it shows in red everywhere and asks you to type its name before a change, or make it read-only), [Ansible](/features/ansible/) and [OpenTofu](/features/opentofu/), and a [database workspace](/features/databases/) (PostgreSQL, MySQL/MariaDB, SQL Server, SQLite, Redis).
- **And:** [Passbolt](/vault/password-managers/) as a source of logins, [system monitoring](/features/monitoring/), [snippets](/features/snippets/) and [recipes](/features/recipes/), [live terminal sharing](/features/session-sharing/), [shared vaults](/vault/sharing/), an [AI assistant](/features/ai-assistant/), an [MCP server](/features/mcp-server/), [plugins](/features/plugins/) and [themes](/features/themes/).

## Where Reach is different

- **Nothing is behind a plan.** Sync, shared vaults, terminal sharing and every other feature are in the one free app.
- **Your data stays yours.** Secrets are encrypted on your device with XChaCha20-Poly1305, with the key in the OS keychain (or behind a master password, or biometrics). Sync goes to a database on *your* Turso account, which only ever stores encrypted data. There is no Reach server and no account.
- **Your whole server toolbox.** RDP and VNC desktops, containers and Kubernetes, [Ansible](/features/ansible/) and [OpenTofu](/features/opentofu/) runs, [databases](/features/databases/), [system monitoring](/features/monitoring/), [recipes](/features/recipes/), an [AI assistant](/features/ai-assistant/) and an [MCP server](/features/mcp-server/) sit next to your terminals, on desktop and Android alike.
- **You can read the code.** Every line is on [GitHub](https://github.com/alexandrosnt/Reach).

## Where Termius is stronger

- **iOS.** Reach has no iPhone or iPad app.
- **Zero-setup sync.** Termius syncs once you sign in; with Reach you first [create a free Turso database](/sync/setup/).
- **Mosh and Telnet.** Termius speaks both; Reach does not.

## Moving over

Reach imports:

- **`~/.ssh/config`**, with jump-host chains (**+** → **Import SSH config**);
- **Remote Desktop Manager exports** (`.rdm` or `.json`): SSH, RDP and VNC entries, gateways and credentials;
- **RDP host lists**: a Termix export or a JSON list of hosts;
- **private keys** (OpenSSH or PEM), pasted or from a file.

There is no Termius importer; if your hosts live only in Termius, put them in an SSH config file first, or add them by hand.

[Download Reach](/download/) — free for Windows, macOS, Linux and Android.
