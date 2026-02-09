---
title: Session Manager
description: Save, organize, and quickly connect to your SSH servers.
---

Sessions are saved connections. Each one stores the host, port, username, auth method, and optionally a folder and tags. Instead of typing connection details every time, you save them once and connect with a click.

## Auth Methods

Reach supports three authentication methods:

- **Password** - Just a username and password.
- **Private Key** - Supports OpenSSH and PEM formats. Ed25519, RSA, and ECDSA keys all work. If your key has a passphrase, you'll be prompted for it.
- **Agent** - SSH agent forwarding. This one is planned but not implemented yet.

## Creating a Session

Click "New Session" in the sidebar. Fill in the host, port, username, and pick your auth method. Hit save. That's it.

If you just need to connect once without saving anything, use **Quick Connect** instead. Type the host, port, and username, and you're in.

## Organizing Sessions

**Folders** let you group sessions together. You can nest folders inside other folders. Drag sessions between folders to reorganize.

**Tags** are comma-separated labels you can add to any session. Use them for filtering when your session list gets long.

## Jump Hosts (ProxyJump)

If your servers sit behind a bastion host, you can configure a jump chain. When creating or editing a session, check "Connect via Jump Host" and add one or more hops. Each hop has its own host, port, username, and auth method.

Reach chains the hops using SSH tunneling (`direct-tcpip` channels) — it connects to the first jump host, opens a tunnel to the next, and so on until it reaches the target. This is the same thing OpenSSH does with `ProxyJump`, but built into the app with no external dependencies.

Quick Connect also supports a single jump host for one-off connections through a bastion.

## Importing from SSH Config

If you already have hosts defined in `~/.ssh/config`, you can import them instead of re-entering everything. Click the "Import SSH Config" button in the Sessions sidebar. Reach parses your config file and shows all named hosts with their resolved settings.

Select the hosts you want, hit Import, and they'll be saved as sessions in your vault. ProxyJump chains are imported automatically — if host A jumps through host B, the full chain is preserved. IdentityFile paths are resolved too.

This works cross-platform: `~/.ssh/config` on Linux/macOS, `C:\Users\<you>\.ssh\config` on Windows.

## Connecting

Click a session card to connect. If you have a saved password or key, it connects right away. If not, you'll get a password prompt. There's a "Remember password" checkbox that stores the password encrypted in the vault for next time.

## Security

Session credentials are encrypted at rest with XChaCha20-Poly1305. They live inside the vault, not in plaintext config files. You won't find passwords sitting in a JSON file somewhere.

## Other Features

**Duplicate** lets you clone a session to quickly create a similar one. Handy when you have a bunch of servers with the same username and auth setup but different hostnames.

**Session sharing** lets you send sessions to teammates securely. It uses X25519 key exchange: you swap public keys, and the session data gets encrypted specifically for the recipient. Nobody else can read it, even if they intercept the payload.
