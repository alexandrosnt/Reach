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

PuTTY keeps sessions in the Windows registry, and Reach has no PuTTY importer yet. If you also use OpenSSH, import `~/.ssh/config` (**+** → **Import SSH config**); otherwise add hosts by hand. PuTTY `.ppk` keys can be converted to OpenSSH format with PuTTYgen (**Conversions** → **Export OpenSSH key**) and imported into the vault.

[Download Reach](/download/) — free for Windows, macOS, Linux and Android.
