---
title: What is Reach?
description: A quick intro to Reach and what it can do.
---

Reach is a cross-platform SSH client and remote management tool. It runs on Windows, macOS, Linux, and Android. You can use it to connect to servers over SSH, browse files with SFTP, manage credentials in an encrypted vault, set up port tunnels, run automated playbooks and recipes, monitor system resources, talk to a serial console, share a live terminal with a colleague, hand a session to an AI client on your terms, and chat with an AI assistant about your infrastructure.

## Why another SSH client?

PuTTY has been around since 1999 and looks like it. MobaXterm is decent but Windows-only and packed with stuff most people never use. Termius looks nice but wants you to pay a subscription to sync your sessions. SecureCRT costs a fortune.

Reach tries to be the SSH client you actually want to use. It's free, it's fast, and it works on every platform. Sessions sync across devices if you want them to. Your passwords are encrypted locally with real cryptography, not just "password protected" config files.

## What's inside

Here's what you get:

**SSH Terminals** with tabs and split panes. You can have multiple sessions open at once, search through output, and resize panes however you like. Shell history is saved per session. Jump host (ProxyJump) support lets you connect through bastion servers with multi-hop chains.

**SFTP File Explorer** that lets you browse remote file systems, upload and download files, edit text files in a built-in editor, and manage permissions. It works like a regular file manager.

**Encrypted Vault** for storing passwords, SSH keys, and other secrets. Uses XChaCha20-Poly1305 for encryption and Argon2id for key derivation. Your encryption key lives in your OS keychain (Windows Credential Store, macOS Keychain, or Linux Secret Service), so you don't need a master password for day-to-day use.

**Port Tunneling** for local and remote forwards. Set them up once and they persist with your session config.

**Playbooks** let you automate server tasks using Ansible-compatible YAML. The playbook engine runs natively in Rust over SSH — no Python or Ansible installation needed. Supports 8 modules including shell, copy, apt, systemd, and more.

**System Monitoring** shows CPU, memory, disk, and network stats for connected hosts. The data updates in real time while you're connected.

**Serial Console** for talking to devices over a serial port. Handy if you work with networking gear, embedded systems, or anything that speaks RS-232.

**AI Assistant** is a built-in chat that can help you with commands, troubleshoot issues, or explain what's happening on your servers. You bring your own API key.

**MCP Server** exposes a terminal session you choose to an AI client — Claude Code, Cursor, Gemini CLI and others — over the Model Context Protocol. It listens on loopback only and is off by default. A guard enforces the rules: the AI must read before it writes, cannot type while you are entering a password, and never sees your secrets in the output. You pick an agent that decides which tools it gets, and a mode that decides how much you are asked.

**Recipes** are reusable bash scripts with parameters and a declared risk level, run against the terminal you have open. Reach analyses each one before it runs, quotes the risky lines, and asks you to type the recipe's name to confirm anything that changes system configuration. Share them through a registry where every script is pinned by hash.

**Session Sharing** puts your terminal and another person's side by side, peer to peer, encrypted end to end, with no server in between. Each of you decides whether the other may only watch or also type — enforced on your own machine. Off by default, and off means the code is not even loaded.

## How it's built

The backend is written in Rust and runs as a Tauri v2 app. SSH connections are handled by [russh](https://github.com/warp-tech/russh), a pure Rust SSH implementation. There's no dependency on OpenSSH or libssh.

The frontend is Svelte 5 with SvelteKit, using Tailwind CSS for styling. It runs inside the system webview (WebView2 on Windows, WebKit on macOS/Linux), not a bundled Chromium. That's what makes it fast to start and light on memory compared to Electron apps.

All communication between the frontend and backend goes through Tauri's IPC system. SSH terminal data is streamed via events for low latency.

## Platforms

Reach runs on:

- **Windows** 10/11 (x64)
- **macOS** 12+ (Intel and Apple Silicon)
- **Linux** (x64, most distros with WebKitGTK 4.1)
- **Android** (ARM64)

Desktop builds include the full feature set. The Android version covers SSH terminals, SFTP, and vault, but skips desktop-only things like serial console and local PTY.
