---
title: General Settings
description: Configure language, shell, appearance, security, AI, sync, backups, and plugins.
---

Open Settings with **Ctrl+,** (or **Cmd+,** on Mac). The settings panel has a left sidebar with seven tabs and the content on the right.

## General

Basic app preferences.

### Language

Pick your language from the dropdown. Six options: English, Deutsch, Fran&ccedil;ais, Ελληνικά, Italiano, Български. Changes apply immediately.

### Default Shell

The shell used for local terminal tabs. Options: Bash, Zsh, PowerShell, CMD. On Linux/macOS this defaults to your `$SHELL`. On Windows it defaults to PowerShell.

### Startup Behavior

Toggle "Open last session" to restore your tabs from the previous session when the app starts.

### Minimize to Tray

When enabled, clicking the close button minimizes the app to the system tray instead of quitting. The app stays running in the background.

### Start with System

Launch Reach automatically when you log in. Uses the OS autostart mechanism (Startup folder on Windows, Launch Agents on macOS, autostart entries on Linux).

## Appearance

Visual customization.

### Theme

Three cards to pick from:

- **Dark** — dark background (#1c1c1e). The default.
- **Light** — light background (#f5f5f7).
- **System** — follows your OS dark/light mode setting.

Click a card and the theme applies immediately.

### Font Size

A slider from 10px to 24px (default 14px). Changes the base font size across the app. The current value shows on the right side of the slider.

### Terminal Font

The font used in terminal tabs. Options: JetBrains Mono, SF Mono, Cascadia Code, Fira Code, monospace. Pick whatever you have installed and like reading.

## Security

Master password and vault lock management.

### Master Password

Shows whether a master password is set (green "Set" badge or red "Not set"). If you have one, you can change it. If you don't, you can set one.

Setting a password opens a form with two fields: new password and confirmation. Minimum 8 characters. The password is used to encrypt/decrypt your vault.

### Lock Status

Shows whether the vault is currently locked or unlocked. When unlocked, a **Lock now** button appears to lock it immediately. Locking clears decrypted secrets from memory.

## AI

Configure the AI assistant.

### Enable Toggle

Turn AI features on or off. When disabled, the rest of the AI settings are grayed out.

### API Key

Enter your OpenRouter API key (starts with `sk-or-`). It's a password field so the key stays hidden. Click **Validate** to test the key — Reach hits the OpenRouter API and reports how many models are available.

The API key is stored encrypted in the vault.

### Model Browser

After validating your key, a searchable list of available models appears. Filter by name or model ID. Each row shows:

- Model name and context window size
- Pricing per million tokens (prompt and completion)

Click a model to select it. The selected model shows below the list with a green checkmark.

## Sync

Cloud synchronization via Turso.

### Identity

Your cryptographic identity for vault sharing:

- **User UUID** — your unique identifier, with a copy button
- **Public key** — your X25519 public key, with a copy button
- **Export backup key** — reveals your secret key for recovery. A warning tells you to keep it safe.

### Cloud Sync Setup

Connect to Turso for cross-device sync:

- **Organization** — your Turso org name
- **Platform API Token** — your Turso API token (password field)
- **Group** — database group name (default: "default")
- **Personal database** — shows the URL once created, or "Will be created"
- **Sync status** — green "Enabled" or red "Disabled" badge

Click **Save setup** to connect. Reach creates a personal database on Turso and enables sync.

### Accept Invite

Join a shared vault someone invited you to:

- **Sync URL** — the libsql URL from the invite
- **Token** — the auth token from the invite

Click **Accept** to join.

## Backup

Export and import encrypted backups.

### Export

Create an encrypted backup of your vault and settings:

1. Enter an export password (minimum 8 characters)
2. Confirm the password
3. Click **Export backup**
4. Pick where to save the `.reachbackup` file

### Import

Restore from a backup:

1. Click **Select file** and pick a `.reachbackup` file
2. Enter the password used during export
3. Click **Verify** — Reach reads the file and shows a preview with:
   - Export date
   - Number of vaults and secrets
   - Whether sync config is included
4. Optionally enter your master password
5. Click **Import backup**

A warning tells you that importing overwrites existing data.

## Plugins

Manage Lua plugins.

### Plugins Directory

Shows the path where Reach looks for `.lua` plugin files. Click **Browse** to change it.

### Discovery and Reload

- **Discover** — scans the plugins directory, auto-grants permissions, and loads any new plugins found. Shows a toast with the count.
- **Reload all** — unloads and reloads every enabled plugin.

### Installed Plugins

Each plugin shows:

- **Name** and **version** badge
- **Status** — Running (green), Loaded (blue), Error (red), or Disabled (gray)
- **Description** and **author** (if provided)
- **Toggle switch** — enable or disable the plugin

See the [Plugins](/Reach/features/plugins/) page for details on writing plugins.
