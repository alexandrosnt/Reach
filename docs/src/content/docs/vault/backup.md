---
title: Backup & Restore
description: Export and import encrypted backups of all your data.
---

You can export everything in Reach to an encrypted backup file. Vaults, secrets, sessions, settings, sync config. All of it, in one file.

## Exporting a Backup

1. Go to **Settings > Backup**.
2. Click "Export Backup".
3. Set an export password (8+ characters).
4. Pick where to save the file.

The backup is encrypted with this password. Without it, the file is useless.

## Importing a Backup

1. Click "Import Backup".
2. Select the backup file.
3. Enter the export password.
4. You'll see a preview showing how many vaults and secrets are in the backup.
5. Confirm to import.

This replaces your current data and restarts the app.

## Identity Backup

Separately from full backups, you can export your encryption key. Go to **Settings > Sync > "Export Backup Key"**. This gives you the raw X25519 secret key in base64.

Store this somewhere safe. You'll need it if you lose access to your OS keychain and need to recover your data on a new machine.

## When to Use Backups

- Moving to a new computer
- Before reinstalling your OS
- Regular backups for peace of mind
- Disaster recovery if your keychain gets corrupted

Bottom line: export your backup key right after setting up Reach and keep it somewhere secure. Future you will be grateful.
