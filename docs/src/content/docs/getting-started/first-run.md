---
title: First Run & Setup
description: What happens the first time you open Reach.
---

When you launch Reach for the first time, a short setup wizard walks you through the basics. It's 3 steps and takes about 30 seconds.

## Step 1: Language

Pick your language. Right now Reach supports English, German (Deutsch), French (Francais), Greek (Ellinika), Italian (Italiano), and Bulgarian (Balgarski). More languages will come over time.

You can always change the language later in Settings > General.

## Step 2: Cloud Sync

This step is optional. If you have a [Turso](https://turso.tech) account, you can enter your organization name and API token here. Turso is a hosted SQLite database that Reach uses to sync sessions and shared vaults across your devices.

Don't have a Turso account? No problem. Just leave the fields blank and click Next. You can set this up later in Settings whenever you want. Reach works fine without it, you just won't get cross-device sync.

## Step 3: All done

The last screen shows a summary of your choices and a "Get Started" button. Click it and you're in.

## The main interface

After the wizard, you'll land on the main app screen. Here's the layout:

- **Title bar** at the top with the app name, settings gear, and AI assistant button.
- **Sidebar** on the left. This is where you find your sessions list, file explorer, playbooks, tunnels, and snippets. Click the icons to switch between panels.
- **Terminal area** in the center. This is where your SSH sessions and local terminals live. You can open multiple tabs and split them horizontally or vertically.

## Your first connection

The quickest way to connect to something is the **Quick Connect** button at the top of the Sessions panel. Type in a hostname, username, and port, and you're connected. Quick Connect sessions aren't saved, they're for one-off use.

To create a saved session, click the **+** button in the Sessions panel. Fill in the host details, pick an authentication method (password, key file, or agent), and save. The session shows up in your list and you can double-click it to connect anytime.

## Identity initialization

The first time you try to save a session or interact with the vault, Reach will ask you to **initialize your identity**. This is a one-time thing.

What happens behind the scenes: Reach generates an X25519 encryption keypair and stores the private key in your operating system's credential store (Windows Credential Store, macOS Keychain, or the Secret Service API on Linux). This keypair is what encrypts and decrypts your saved passwords, SSH keys, and vault secrets.

Because the key lives in your OS keychain, you don't need to type a master password every time you open the app. Your system handles the authentication. If your OS account is locked, your keys are locked too.

This is the same general approach that TLS/SSH use for key management: the private key stays protected, and you authenticate through the system rather than through a separate password. It's more convenient than a master password and just as secure, assuming you keep your OS account locked when you step away.

After initialization, sessions and vault entries are encrypted automatically. You won't need to think about it again unless you move to a new machine, in which case you can export and import your identity from the settings.
