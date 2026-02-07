---
title: Setting Up Cloud Sync
description: Step-by-step guide to connecting Reach with Turso.
---

## Step 1: Create a Turso Account

Head to [turso.tech](https://turso.tech) and sign up if you don't have an account yet.

## Step 2: Get Your Credentials

- Go to your Turso dashboard.
- Note your **organization name**. It's in the URL, like `turso.tech/my-org-name`.
- Go to settings and create a **Platform API Token**.

## Step 3: Enter Credentials in Reach

You can do this in two places:

- During the **first-run wizard** (Step 2 of setup)
- Or later in **Settings > Sync**

Enter your organization name and API token, then click "Save & Setup".

## Step 4: Done

Reach creates a personal database on your Turso account. This stores your encrypted vault data in the cloud.

That's it. Once set up, your data syncs automatically. New sessions, updated secrets, everything stays in sync.

## Setting Up a Second Device

1. Install Reach on the new device.
2. Go through the setup wizard.
3. Enter the same Turso credentials (org name + API token).
4. Import your identity. Either use a backup file or paste your backup key.
5. Your vaults will sync down from Turso.

The identity import step is important. Without your encryption keys, the synced data is just encrypted blobs that can't be read.

## A Note About Security

The Turso credentials are stored locally in your app settings. The API token gives access to create databases on your account, so keep it safe. Don't share it with anyone you don't trust with your Turso account.
