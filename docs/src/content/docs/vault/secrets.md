---
title: Managing Secrets
description: Store passwords, API keys, SSH keys, and notes securely.
---

The vault stores secrets in categories: **Password**, **SSH Key**, **API Token**, **Certificate**, **Note**, or **Custom**.

## Getting Started

Open the **Vault** section from the rail on the left — it sits just under Sessions. If this is your first time, you'll need to initialize your identity first. This creates your encryption keypair and stores the secret key in your OS keychain.

## Finding Your Way

The Vault section lists your vaults grouped **Private** and **Shared**, sorted by name, with each one's secret count and — for shared vaults — member count. An amber dot marks a vault that cannot be reached right now; hover it for the reason. There is a search box at the top and **New vault** at the bottom. Internal vaults that Reach keeps for itself (session passwords, playbooks) are not shown.

Open a vault and its secrets are listed the same way: category glyph, name, category and when it was last changed. The search box matches names and categories, so "key" finds every SSH key whatever it was called. The vault's name, type and member count sit in the header, with the back arrow and, for a shared vault, the invite button.

## Creating a Secret

Open a vault, click "Add Secret", give it a name, pick a category, and enter the value. For passwords, there's a "Generate" button that creates a random strong password for you.

## Reading Secrets

Secrets are stored encrypted. Click "Show" to decrypt and reveal the value. Click "Copy" to copy it to your clipboard.

## Vault Types

- **Private**: Only you can access it. Data stays local on your machine.
- **Shared**: Team members can be invited to access the vault. Requires Turso cloud sync for the shared database.

## Multiple Vaults

You can create as many vaults as you want. This is useful for separating personal credentials from team or project credentials. Keep your AWS keys in one vault, your homelab SSH passwords in another. Whatever makes sense for you.

## Session Passwords

When you check "Remember password" on an SSH connection, the password gets stored in an internal vault. You don't have to manage these manually. Reach handles it behind the scenes.

## Playbooks

Playbooks are also stored encrypted in an internal vault. Same encryption, same protection.
