---
title: Jump Hosts & SSH Config
description: Connect through bastion servers and import hosts from your SSH config file.
---

Many production environments sit behind bastion (jump) hosts. Instead of manually SSH-ing into a bastion and then hopping to your target, Reach handles the entire chain automatically.

## How it works

Reach uses SSH tunneling (`direct-tcpip` channels) to chain connections. It connects to the first jump host, opens a tunnel to the next hop, establishes SSH over that tunnel, and repeats until it reaches the target server. This is the same thing OpenSSH does with `ProxyJump`, but built into the app with no external dependencies.

All intermediate connections stay alive in the background. When you disconnect, everything is cleaned up in reverse order.

## Adding a jump host to a saved session

1. Open the Session Editor (create new or edit existing)
2. Check **Connect via Jump Host** (in the jump host section)
3. Click **Add Hop** to add a bastion server
4. Fill in the hop's host, port, username, and auth method
5. Add more hops if needed — they're chained in order (first hop = outermost bastion)
6. Save the session

Each hop has its own authentication settings. You can mix password and key-based auth across hops.

## Quick Connect with a jump host

Quick Connect also supports a single jump host for ad-hoc connections. Check the "Connect via Jump Host" checkbox below the main auth fields, fill in the bastion details, and connect.

## Multi-hop chains

You can chain as many hops as you need. For example:

```
You → Bastion A → Bastion B → Target Server
```

In the session editor, you'd add two hops:
- **Hop 1**: Bastion A (the outermost server you connect to first)
- **Hop 2**: Bastion B (reached through Bastion A)

The target server is your main session host.

## Importing from SSH Config

If you already have hosts defined in `~/.ssh/config`, you can import them instead of entering everything manually.

### How to import

1. Click the **Import SSH Config** button in the Sessions sidebar (it has a download icon)
2. Reach parses your config file and shows all named hosts
3. Each host shows its resolved hostname, port, user, and any proxy chain
4. Select the hosts you want with checkboxes (or use Select All)
5. Click **Import Selected**
6. Imported hosts appear as saved sessions in your vault

### What gets imported

Everything. Reach reads the file the way OpenSSH does (`Host` and `Match` blocks, `Include`, tokens, first value wins) and keeps every keyword that applies to the host, so an imported host connects exactly as `ssh` would:

- **HostName**, **Port**, **User** — resolved from the config (including wildcard matches)
- **IdentityFile** — imported as key-based auth with the resolved path
- **ProxyJump chains** — if host A jumps through host B, and B jumps through C, the full chain `[C, B]` is preserved
- **ProxyCommand**, algorithms, forwards and every other keyword — kept as the session's SSH options (see below)
- Hosts that are already imported (matching host + port + user) show as grayed out

A line Reach cannot apply is shown to you, never dropped.

## ProxyCommand

`ProxyCommand` runs a local program and uses its input and output as the connection, for example a corporate proxy, `cloudflared access ssh`, or an older bastion setup:

```
ProxyCommand ssh -W %h:%p bastion.example.com
```

It comes in with an imported host, or you can add it to any session yourself: open the Session Editor, go to **SSH options (ssh_config)** and write it on a line of its own, just as in `~/.ssh/config`. The tokens `%h`, `%p`, `%r`, `%n` and `%k` are expanded as OpenSSH does, and `ProxyUseFdpass` works on Linux and macOS.

- The command runs through your shell (`cmd` on Windows), as `ssh` runs it.
- Because it runs a program on your computer, Reach asks you to approve it once for that session. A session someone else changes in a shared vault needs your approval again.
- Desktop only: a phone cannot run local programs, so a session with `ProxyCommand` says so on Android. Use `ProxyJump` there instead.

### Cross-platform paths

SSH config is read from:
- **Linux / macOS**: `~/.ssh/config`
- **Windows**: `C:\Users\<you>\.ssh\config`

Reach resolves `~` in IdentityFile paths to your home directory on all platforms.

### Limitations

- Wildcard-only hosts (like `Host *`) are skipped during import since they don't represent specific servers
