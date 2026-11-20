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
| X server for X11 forwarding | Yes: on Windows Reach [downloads VcXsrv](/settings/general/#x11-server) once when you turn it on; Linux and macOS use the system's | Built in | Built in |
| Telnet, Mosh, rlogin | No | Yes | Yes |
| Saved passwords | [Encrypted vault](/vault/encryption/): key in the OS keychain, or a master password, or biometrics | Master password | Master password |

## Also in Reach

- **Logins and host keys as OpenSSH does them:** FIDO2 security keys (`sk-ssh-ed25519`), smart cards and tokens (PKCS#11), Kerberos (GSSAPI), certificates and host-based login; host keys checked against DNS (SSHFP, with DNSSEC validated), connection sharing (ControlMaster) and `ssh -w` tunnels, all from your [ssh_config](/features/jump-hosts/).
- **A vault that guards itself:** unlock with a YubiKey or other FIDO2 key, with Windows Hello, Touch ID or an Android fingerprint, or a master password. It locks when your computer locks. While it is open, its keys stay encrypted in memory and are never swapped to disk, and other programs are kept from reading Reach's memory.
- **Around the terminal:** tab groups and split panes, a command palette, session logs to a file, local terminals and WSL distributions, a built-in editor, compress and extract on the server from the [file browser](/features/file-explorer/), and log files inside containers and pods.
- **Getting there:** SOCKS4, SOCKS5 and HTTP CONNECT proxies (Tor included), jump-host chains and ProxyCommand.
- **In your language:** English, Bulgarian, Chinese, Czech, French, German, Greek, Italian, Russian and Spanish.
- **DevOps in the same window:** Kubernetes, Docker, Compose and Helm over the SSH session (mark a cluster or host Production and it shows in red everywhere and asks you to type its name before a change, or make it read-only), [Ansible](/features/ansible/) and [OpenTofu](/features/opentofu/), and a [database workspace](/features/databases/) (PostgreSQL, MySQL/MariaDB, SQL Server, SQLite, Redis).
- **And:** [Passbolt](/vault/password-managers/) as a source of logins, [system monitoring](/features/monitoring/), [snippets](/features/snippets/) and [recipes](/features/recipes/), [live terminal sharing](/features/session-sharing/), [shared vaults](/vault/sharing/), an [AI assistant](/features/ai-assistant/), an [MCP server](/features/mcp-server/), [plugins](/features/plugins/) and [themes](/features/themes/).

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
- **Remote Desktop Manager exports** (`.rdm` or `.json`): SSH (PuTTY entries included), RDP and VNC entries, gateways and credentials;
- **RDP host lists**: a Termix export or a JSON list of hosts;
- **private keys** (OpenSSH or PEM), pasted or from a file.

There is no MobaXterm session importer yet.

[Download Reach](/download/) — free for Windows, macOS, Linux and Android.
