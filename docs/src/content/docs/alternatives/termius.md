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
| Account | None for Reach; sync uses your own free Turso account | Needed for sync |
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

Everything below is in the free app. Most of it works on Android too; where it doesn't, it says so.

- **Remote desktops, properly:** RDP with the remote's sound, a two-way clipboard for text and files, and a local folder shared in as a drive (desktop); RDP to Windows or to Linux (xrdp), with domain logins; VNC that can travel inside a saved SSH session, so its port stays closed to the network. Both run on Android.
- **SSH that tells you the truth:** a warning when a server does not use a post-quantum key exchange; ssh_config settings that weaken a connection or run commands wait for your approval; one-click presets for old servers (legacy algorithms), keep-alive and compression; keyboard-interactive and one-time codes; known_hosts read as OpenSSH reads it, hashed names, revoked keys and UpdateHostKeys included.
- **Logins as OpenSSH does them:** FIDO2 security keys (`sk-ssh-ed25519`) and Kerberos on the desktop, smart cards and tokens (PKCS#11), certificates, host-based login, Pageant and the OpenSSH agent; host keys checked against DNS (SSHFP, with DNSSEC validated), connection sharing (ControlMaster) and `ssh -w` tunnels, all from your [ssh_config](/features/jump-hosts/). A key you import into the vault goes to every device with your sessions, phone included.
- **A vault for every secret:** passwords, API tokens, certificates and notes, with a password generator; [shared vaults](/vault/sharing/) with Owner, Admin, Member and Read-only roles. Unlock it with a YubiKey or other FIDO2 key (desktop), Windows Hello, an Android fingerprint, Touch ID (beta) or a master password. It locks when your computer locks or after a while idle; while it is open, its keys stay encrypted in memory and are never swapped to disk, and other programs are kept from reading Reach's memory.
- **The terminal:** tab groups, split panes (drag a tab to split), a command palette, [snippets](/features/snippets/) suggested as you type, Quick Connect from the search box (type `user@host`), reconnect after a drop, clickable links, session logs to a file the way PuTTY writes them (file names from host and date), local terminals and WSL distributions (desktop), and an extra-keys bar on a phone.
- **The file browser:** follows the terminal's folder and can send it a `cd`; a details view with owner, group and permissions; drag files in and out; a transfer queue you can cancel; compress and extract on the server; a [built-in editor](/features/file-explorer/) with syntax highlighting (desktop).
- **Getting there:** SOCKS4, SOCKS5 and HTTP CONNECT proxies (Tor included), jump-host chains and ProxyCommand.
- **Containers and Kubernetes:** Docker, rootless Docker and Podman on any server you can SSH to (and Docker or Podman Desktop on this computer), with Compose stacks, images, volumes and networks, a shell, logs and inspect for each container, and error help that offers to run the fix. Kubernetes clusters from a kubeconfig, also through an SSH bastion: a health overview in plain words, scale and rollout restart, Helm releases and rollback, and YAML edits that refuse to overwrite someone else's change. Mark a host or cluster Production and it shows in red everywhere and asks you to type its name before a change, or make it read-only.
- **Databases:** PostgreSQL, MySQL/MariaDB, SQL Server, SQLite and Redis, reached through a saved SSH session or the open terminal, and found for you on your servers; a table designer, export to CSV, JSON or SQL, backup and restore with `.sql` files, query history, EXPLAIN, a server monitor, a Redis console, and the AI to explain or speed up a query. See [databases](/features/databases/).
- **Automation:** [Ansible](/features/ansible/) playbooks with a dry run (check and diff), inventories built from your saved sessions, Galaxy roles and collections, ad hoc commands and Ansible Vault, run here, through WSL, in a container or on the server, so it works from Windows (on Android, on the server). [OpenTofu](/features/opentofu/) on the desktop: it installs itself, applies exactly the plan you reviewed, finds drift, encrypts state and plans, handles workspaces and state moves, and asks you to type the project's name before a destroy.
- **AI, on your terms:** an [AI assistant](/features/ai-assistant/) with any model through your OpenRouter key; an [MCP server](/features/mcp-server/) where you choose which terminal tab an AI client sees, how much it may do (Ask, Auto, Dangerous), what it must confirm, and secrets are taken out of what it reads. It listens on this computer only.
- **Safety nets:** multi-line pastes ask before they run, host keys are checked before you log in, [recipes](/features/recipes/) are rated benign to destructive before they run, [plugins](/features/plugins/) run sandboxed and reach only what they were granted, and [backups](/vault/backup/) are encrypted with a password of yours.
- **And:** [Passbolt](/vault/password-managers/) as a source of logins (desktop), with entries set by someone else used only once you allow them; [system monitoring](/features/monitoring/) of CPU, memory, disk, network and logged-in users; [live terminal sharing](/features/session-sharing/), read-only or read-write, with your own STUN and TURN servers if you have them; [plugins](/features/plugins/) and [themes](/features/themes/); "Report a problem" with a log cleaned of your hosts and names; English, Bulgarian, Chinese, Czech, French, German, Greek, Italian, Russian and Spanish; installs from apt, dnf, winget and Scoop.

## Where Reach is different

- **Nothing is behind a plan.** Sync, shared vaults, terminal sharing and every other feature are in the one free app.
- **Your data stays yours.** Secrets are encrypted on your device with XChaCha20-Poly1305, with the key in the OS keychain (or behind a master password, or biometrics). Sync goes to a database on *your* Turso account, which only ever stores encrypted data. There is no Reach server and no account.
- **Your whole server toolbox.** RDP and VNC desktops, containers and Kubernetes, [Ansible](/features/ansible/) and [OpenTofu](/features/opentofu/) runs, [databases](/features/databases/), [system monitoring](/features/monitoring/), [recipes](/features/recipes/), an [AI assistant](/features/ai-assistant/) and an [MCP server](/features/mcp-server/) sit next to your terminals, most of them on Android too.
- **You can read the code.** Every line is on [GitHub](https://github.com/alexandrosnt/Reach).

## Where Termius is stronger

- **iOS.** Reach has no iPhone or iPad app.
- **Zero-setup sync.** Termius syncs once you sign in; with Reach you first [create a free Turso database](/sync/setup/).
- **Mosh and Telnet.** Termius speaks both; Reach does not.

## Moving over

Reach imports:

- **`~/.ssh/config`**, with jump-host chains (**+** → **Import SSH config**);
- **Remote Desktop Manager exports** (`.rdm` or `.json`): SSH (PuTTY entries included), RDP and VNC entries, gateways, and the usernames of linked credentials (passwords are asked for on connect);
- **RDP host lists**: a Termix export or a JSON list of hosts;
- **private keys** (OpenSSH or PEM), pasted or from a file.

There is no Termius importer; if your hosts live only in Termius, put them in an SSH config file first, or add them by hand.

[Download Reach](/download/) — free for Windows, macOS, Linux and Android.
