---
title: "A modern PuTTY alternative"
description: "Reach is a free, open source alternative to PuTTY with tabs, a built-in SFTP file browser, saved credentials in an encrypted vault, tunnels, RDP and VNC, on Windows, macOS, Linux and Android."
---

[PuTTY](https://www.chiark.greenend.org.uk/~sgtatham/putty/) has been the default SSH client on Windows for over twenty-five years: small, free, open source (MIT) and very well proven. It is also deliberately minimal. Each session is its own window, files move through the command-line tools PSCP and PSFTP, and by design it never remembers passwords. Its team has no macOS or mobile version and does not expect to finish one (PuTTY FAQ A.3.2 and A.3.3).

Reach keeps the free and open source part and adds what PuTTY leaves out.

## At a glance

| | Reach | PuTTY |
| --- | --- | --- |
| Price and licence | Free, MIT | Free, MIT |
| Platforms | Windows, macOS, Linux, Android | Windows, Unix |
| Tabs and split panes | [Yes](/features/ssh-terminal/) | No, one window per session |
| File transfer | [SFTP browser](/features/file-explorer/) with drag and drop | PSCP and PSFTP on the command line |
| Saved passwords and keys | In an [encrypted vault](/vault/encryption/) | Not stored, by design; keys through Pageant |
| Port forwarding | [Yes](/features/tunnels/) | Yes |
| Jump hosts and ssh_config | [Yes](/features/jump-hosts/) | Proxy settings per session |
| Serial console | [Yes](/features/serial/) (desktop) | Yes |
| X11 forwarding | Yes, and on Windows Reach [sets up the X server](/settings/general/#x11-server) itself | Yes, with a separate X server on Windows |
| RDP and VNC | [Yes](/features/remote-desktop/) | No |
| Sync between machines | Free, through your own [Turso](/sync/what-is-turso/) database | No (sessions live in the Windows registry) |
| Telnet, rlogin | No | Yes |

## Also in Reach

- **Logins and host keys as OpenSSH does them:** FIDO2 security keys (`sk-ssh-ed25519`), smart cards and tokens (PKCS#11), Kerberos (GSSAPI), certificates and host-based login; host keys checked against DNS (SSHFP, with DNSSEC validated), connection sharing (ControlMaster) and `ssh -w` tunnels, all from your [ssh_config](/features/jump-hosts/).
- **A vault that guards itself:** unlock with a YubiKey or other FIDO2 key, with Windows Hello, Touch ID or an Android fingerprint, or a master password. It locks when your computer locks. While it is open, its keys stay encrypted in memory and are never swapped to disk, and other programs are kept from reading Reach's memory.
- **Around the terminal:** tab groups and split panes, a command palette, session logs to a file, local terminals and WSL distributions, a built-in editor, compress and extract on the server from the [file browser](/features/file-explorer/), and log files inside containers and pods.
- **Getting there:** SOCKS4, SOCKS5 and HTTP CONNECT proxies (Tor included), jump-host chains and ProxyCommand.
- **In your language:** English, Bulgarian, Chinese, Czech, French, German, Greek, Italian, Russian and Spanish.
- **Containers and Kubernetes:** Docker, rootless Docker and Podman on any server you can SSH to, with Compose stacks, a shell, logs and inspect for each container; Kubernetes clusters from a kubeconfig, with namespaces, pods, live edits and Helm. Mark a host or cluster Production and it shows in red everywhere and asks you to type its name before a change, or make it read-only.
- **Automation and data:** [Ansible](/features/ansible/) playbooks, inventories, roles, collections, ad hoc commands and Ansible Vault, run here, through WSL, in a container or on the server, so it works from Windows too; [OpenTofu](/features/opentofu/) plan and apply with drift detection, providers and environments, installed for you; a [database workspace](/features/databases/) for PostgreSQL, MySQL/MariaDB, SQL Server, SQLite and Redis, with a table designer, backups to `.sql` and the AI to explain a query.
- **Safety nets:** multi-line pastes ask before they run, Reach locks itself after a while idle, host keys are checked before you log in, [recipes](/features/recipes/) are rated benign to destructive before they run, [plugins](/features/plugins/) run sandboxed and reach only what they were granted, and [backups](/vault/backup/) are encrypted with a password of yours.
- **And:** [Passbolt](/vault/password-managers/) as a source of logins, [system monitoring](/features/monitoring/), [snippets](/features/snippets/) and [recipes](/features/recipes/), [live terminal sharing](/features/session-sharing/), [shared vaults](/vault/sharing/), an [AI assistant](/features/ai-assistant/), an [MCP server](/features/mcp-server/), [plugins](/features/plugins/) and [themes](/features/themes/).

## Where Reach is different

- **Everything in one window.** Tabs, split panes, a file browser beside the terminal and [live system stats](/features/monitoring/) for the host.
- **Log in once.** Passwords and keys live in an encrypted vault (XChaCha20-Poly1305, key in the OS keychain), so a saved session connects with one click.
- **Your setup follows you.** Sessions and secrets sync between Windows, macOS, Linux and Android.
- **Reads your existing config.** Import `~/.ssh/config`, including jump hosts.

## Where PuTTY is stronger

- **Tiny and everywhere.** A few megabytes, one executable, nothing to install.
- **Scripting with Plink**, and decades of compatibility with old servers and network equipment.
- **Telnet and rlogin**, which Reach does not speak.

## Moving over

Reach imports:

- **`~/.ssh/config`**, with jump-host chains (**+** → **Import SSH config**);
- **Remote Desktop Manager exports** (`.rdm` or `.json`): SSH (PuTTY entries included), RDP and VNC entries, gateways and credentials;
- **RDP host lists**: a Termix export or a JSON list of hosts;
- **private keys** (OpenSSH or PEM), pasted or from a file.

PuTTY itself keeps sessions in the Windows registry, and Reach does not read them there. If your PuTTY sessions are in Remote Desktop Manager, or you also keep an `~/.ssh/config`, import that; otherwise add hosts by hand. PuTTY `.ppk` keys can be converted to OpenSSH format with PuTTYgen (**Conversions** → **Export OpenSSH key**) and imported into the vault.

[Download Reach](/download/) — free for Windows, macOS, Linux and Android.
