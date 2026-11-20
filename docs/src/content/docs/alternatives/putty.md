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
- **Remote Desktop Manager exports** (`.rdm` or `.json`): SSH (PuTTY entries included), RDP and VNC entries, gateways, and the usernames of linked credentials (passwords are asked for on connect);
- **RDP host lists**: a Termix export or a JSON list of hosts;
- **private keys** (OpenSSH or PEM), pasted or from a file.

PuTTY itself keeps sessions in the Windows registry, and Reach does not read them there. If your PuTTY sessions are in Remote Desktop Manager, or you also keep an `~/.ssh/config`, import that; otherwise add hosts by hand. PuTTY `.ppk` keys can be converted to OpenSSH format with PuTTYgen (**Conversions** → **Export OpenSSH key**) and imported into the vault.

[Download Reach](/download/) — free for Windows, macOS, Linux and Android.
