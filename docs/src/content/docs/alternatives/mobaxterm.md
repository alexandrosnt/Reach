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
| Kubernetes, Docker, Compose, Helm | Yes, over the SSH session | — | — |

Also in Reach, free: [Ansible](/features/ansible/) and [OpenTofu](/features/opentofu/) runs, a [database workspace](/features/databases/) (PostgreSQL, MySQL/MariaDB, SQL Server, SQLite, Redis), [Passbolt](/vault/password-managers/) as a source of logins, [system monitoring](/features/monitoring/), [recipes](/features/recipes/), an [AI assistant](/features/ai-assistant/), an [MCP server](/features/mcp-server/), [plugins](/features/plugins/) and [themes](/features/themes/).

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
