---
title: "A free, open source MobaXterm alternative"
description: "Reach is a free, MIT-licensed alternative to MobaXterm: SSH, SFTP, RDP, VNC, tunnels and a serial console with no session or tunnel limits, on Windows, macOS, Linux and Android."
---

[MobaXterm](https://mobaxterm.mobatek.net) is the Swiss army knife of Windows remote work: a tabbed SSH client, an X server, RDP, VNC and an SFTP browser in one program. Its free Home Edition is limited to 12 saved sessions, 2 SSH tunnels and 4 macros; the Professional Edition removes the limits for $69 / 49€ per user (MobaXterm download page, November 2026). It runs on Windows only.

Reach covers the same daily work with no limits, on every desktop and on Android.

## At a glance

| | Reach | MobaXterm Home | MobaXterm Professional |
| --- | --- | --- | --- |
| Price | Free | Free | $69 / 49€ per user |
| Licence | Open source (MIT) | Proprietary | Proprietary |
| Saved sessions | Unlimited | Max. 12 | Unlimited |
| SSH tunnels | Unlimited | Max. 2 | Unlimited |
| Platforms | Windows, macOS, Linux, Android | Windows | Windows |
| SSH with SFTP browser | Yes, with a [details view](/features/file-explorer/) | Yes | Yes |
| RDP and VNC | [Yes](/features/remote-desktop/) | Yes | Yes |
| Serial console | [Yes](/features/serial/) (desktop) | Yes | Yes |
| X server for X11 forwarding | Yes: on Windows Reach [downloads VcXsrv](/settings/general/#x11-server) once when you turn it on; Linux uses the system's, a Mac needs XQuartz | Built in | Built in |
| Telnet, Mosh, rlogin | No | Yes | Yes |
| Saved passwords | [Encrypted vault](/vault/encryption/): key in the OS keychain, or a master password, or biometrics | Master password | Master password |

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

- **No limits, ever.** Sessions, tunnels and snippets are unlimited, and nothing needs a licence for use at work.
- **The same app everywhere.** Your sessions and secrets [sync](/sync/setup/) between Windows, macOS, Linux and Android, encrypted before they leave the device.
- **Built for teams too.** [Shared vaults](/vault/sharing/) and [live session sharing](/features/session-sharing/), peer to peer.
- **Automation next to the terminal.** [Ansible](/features/ansible/), [OpenTofu](/features/opentofu/), [recipes](/features/recipes/) and [databases](/features/databases/).
- **Open source.** Read, build or audit it on [GitHub](https://github.com/alexandrosnt/Reach).

## Where MobaXterm is stronger

- **Telnet, Mosh and rlogin**, and its bundled Unix tools and network daemons (TFTP, NFS and others).
- **X server in the box.** Its X server is part of the download; Reach fetches VcXsrv the first time you turn X11 on.
- **Portable edition.** MobaXterm runs from a USB stick without installing.

## Moving over

Reach imports:

- **`~/.ssh/config`**, with jump-host chains (**+** → **Import SSH config**);
- **Remote Desktop Manager exports** (`.rdm` or `.json`): SSH (PuTTY entries included), RDP and VNC entries, gateways, and the usernames of linked credentials (passwords are asked for on connect);
- **RDP host lists**: a Termix export or a JSON list of hosts;
- **private keys** (OpenSSH or PEM), pasted or from a file.

There is no MobaXterm session importer yet.

[Download Reach](/download/) — free for Windows, macOS, Linux and Android.
