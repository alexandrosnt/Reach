---
title: Managing Secrets
description: Store passwords, API keys, SSH keys, and notes securely.
---

The vault stores secrets in categories: **Password**, **SSH Key**, **API Token**, **Certificate**, **Note**, or **Custom**.

## Getting Started

To use the vault, go to the Vault section in the app. If this is your first time, you'll need to initialize your identity first. This creates your encryption keypair and stores the secret key in your OS keychain.

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
