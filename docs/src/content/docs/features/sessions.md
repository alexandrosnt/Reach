---
title: Session Manager
description: Save, organize, and quickly connect to your SSH servers.
---

Sessions are saved connections. Each one stores the host, port, username, auth method, and optionally a folder and tags. Instead of typing connection details every time, you save them once and connect with a click.

## Auth Methods

Reach supports three authentication methods:

- **Password** - Just a username and password.
- **Private Key** - Supports OpenSSH and PEM formats. Ed25519, RSA, and ECDSA keys all work. Point at a key file on this machine, or import the key into your vault (below). A key with no passphrase connects without asking for anything.
- **Agent** - SSH agent forwarding. This one is planned but not implemented yet.

### Imported keys

Choosing **Private Key** offers two sources: an **imported key** or a **key file**.

A key file is read from this machine's disk every time you connect, so the session only works where that file is. Import the key instead and the key itself is stored in your vault — encrypted at rest, and carried to every machine you sign in from by the same sync that carries your sessions. That is the point: one import, and the session connects from your desktop, your laptop and anywhere else, with no copying of `~/.ssh` between them.

**Import a key…** takes the key from a file or from a paste. A file is read by Reach itself, so key material never passes through the interface. Reach checks the key before saving it: a public key pasted by mistake, or a passphrase that does not open the key, is refused there and then rather than on your next connection attempt.

The passphrase belongs to the key, not to the session. Save it once with the key and every session using that key connects without asking again; leave it empty and Reach asks when it needs it. An unencrypted key is never asked about at all.

Each key shows its algorithm and SHA-256 fingerprint, so two keys with similar names stay distinguishable. **Forget key** removes one from the vault.

Imported keys are part of your backups. `vault_export_backup` writes every vault it holds open, keys included, sealed with your export password; restoring a backup on a new machine brings the keys back with the sessions that use them.

A session shared with a teammate carries the *reference* to a key, not the key itself — your private key is never handed to anyone. If they don't have that key, Reach says so plainly and they can import their own or point the session at a key file.

## The Top of the Sidebar

Two rows. First the vault switcher (see below). Then a field and a **+**.

The field searches your sessions as you type. Type something that reads as an address instead — `root@10.0.0.7`, `bastion.example.com:2200` — and a **Connect to …** suggestion appears above the list. Press Enter (when no saved session matched) or click it, and Quick Connect opens with the host, port and username already filled; type the password or pick a key and you are in. A session whose name happens to contain a dot still just filters — the suggestion is offered, never assumed.

The **+** holds everything else: **New session** (`Ctrl+N`), **Quick Connect** (`Ctrl+Shift+N`, for the jump-host and proxy cases), **New folder**, and **Import SSH config**. The menu shows each shortcut so you can stop needing the menu.

## Creating a Session

Choose **New session** from the **+** menu, or press `Ctrl+N`. Fill in the host, port, username, and pick your auth method. Hit save. That's it.

If you just need to connect once without saving anything, type the address into the search field, or choose **Quick Connect** from the **+** menu.

## Organizing Sessions

**Folders** let you group sessions together. You can nest folders inside other folders. Drag sessions between folders to reorganize.

**Tags** are comma-separated labels you can add to any session. Use them for filtering when your session list gets long.

## Sessions and Vaults

Every session belongs to a vault — the device's own private vault by default, or any vault you choose when saving it. A vault is the unit of sharing: put a session in a shared vault and everyone in that vault has it.

The sidebar shows sessions from **all vaults at once**. A session that lives outside the device's own vault carries a small chip naming its vault — green with a people icon for a shared vault — so you never have to remember where a host lives before you can connect to it.

The control above the list is the vault switcher. Click it to narrow the sidebar to one vault: its folders appear, and new sessions and folders go into it. Type to filter, use the arrow keys and Enter, or click. Vaults are grouped Private and Shared, each with its secret and member counts; an amber dot marks one that cannot be reached right now, with the reason on hover. "New vault" is at the bottom of the picker. Your choice is remembered per device.

The device's own vault is called **Private (this device)** in the picker, to keep it apart from vaults whose *type* is private.

## Jump Hosts (ProxyJump)

If your servers sit behind a bastion host, you can configure a jump chain. When creating or editing a session, check "Connect via Jump Host" and add one or more hops. Each hop has its own host, port, username, and auth method.

Reach chains the hops using SSH tunneling (`direct-tcpip` channels) — it connects to the first jump host, opens a tunnel to the next, and so on until it reaches the target. This is the same thing OpenSSH does with `ProxyJump`, but built into the app with no external dependencies.

Quick Connect also supports a single jump host for one-off connections through a bastion.

## Importing from SSH Config

If you already have hosts defined in `~/.ssh/config`, you can import them instead of re-entering everything. Choose **Import SSH config** from the **+** menu at the top of the Sessions sidebar. Reach parses your config file and shows all named hosts with their resolved settings.

Select the hosts you want, hit Import, and they'll be saved as sessions in your vault. ProxyJump chains are imported automatically — if host A jumps through host B, the full chain is preserved. IdentityFile paths are resolved too.

This works cross-platform: `~/.ssh/config` on Linux/macOS, `C:\Users\<you>\.ssh\config` on Windows.

## Connecting

Click a session card to connect. A session with a saved password, or with any key at all, connects right away — Reach only asks for a passphrase when the key turns out to be locked and it has none saved. If there is nothing to connect with, you'll get a password prompt. There's a "Remember password" checkbox that stores the password encrypted in the vault for next time.

## Security

Session credentials are encrypted at rest with XChaCha20-Poly1305. They live inside the vault, not in plaintext config files. You won't find passwords sitting in a JSON file somewhere.

## Other Features

**Duplicate** lets you clone a session to quickly create a similar one. Handy when you have a bunch of servers with the same username and auth setup but different hostnames.

**Session sharing** lets you send sessions to teammates securely. It uses X25519 key exchange: you swap public keys, and the session data gets encrypted specifically for the recipient. Nobody else can read it, even if they intercept the payload.
