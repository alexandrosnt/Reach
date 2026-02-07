---
title: Setting Up Cloud Sync
description: Step-by-step guide to connecting Reach with Turso.
---

## Step 1: Create a Turso Account

Head to [turso.tech](https://turso.tech) and sign up if you don't have an account yet. The free tier is more than enough for personal use.

You can also check out the [Turso quickstart guide](https://docs.turso.tech/quickstart) if you want to get familiar with how Turso works before setting it up in Reach.

## Step 2: Install the Turso CLI (optional but helpful)

The Turso CLI makes it easy to grab your credentials. You don't strictly need it (you can do everything from the dashboard), but it's handy.

**macOS / Linux:**
```bash
curl -sSfL https://get.tur.so/install.sh | bash
```

**Windows (PowerShell):**
```powershell
irm https://get.tur.so/install.ps1 | iex
```

After installing, log in:
```bash
turso auth login
```

This opens your browser to authenticate. Once done, you're good to go.

For more details, see the [Turso CLI reference](https://docs.turso.tech/cli/introduction).

## Step 3: Get Your Organization Name

Your org name is basically your account identifier. You can find it in a few ways:

**From the dashboard:** Log into [turso.tech](https://turso.tech). The org name is in the URL: `turso.tech/your-org-name`.

**From the CLI:**
```bash
turso org list
```

This prints your organizations. Use the one you want Reach to create databases in.

## Step 4: Create a Platform API Token

Reach needs a Platform API Token to create and manage databases on your Turso account.

**From the dashboard:** Go to your org settings, find the API Tokens section, and create a new token.

**From the CLI:**
```bash
turso auth api-tokens mint reach-app
```

This creates a token called "reach-app". Copy it somewhere safe, you'll need it in the next step. The token won't be shown again.

For more on API tokens, see [Turso Platform API docs](https://docs.turso.tech/platform/api).

## Step 5: Enter Credentials in Reach

You can do this in two places:

- During the **first-run wizard** (Step 2 of setup)
- Or later in **Settings > Sync**

Enter your organization name and the API token you just created, then click "Save & Setup".

## Step 6: Done

Reach creates a personal database on your Turso account automatically. This stores your encrypted vault data in the cloud.

That's it. Once set up, your data syncs automatically. New sessions, updated secrets, everything stays in sync.

You can verify the database was created:
```bash
turso db list
```

You should see a database with a name like `reach-personal-xxxxx`.

## Setting Up a Second Device

1. Install Reach on the new device.
2. Go through the setup wizard.
3. Enter the same Turso credentials (org name + API token).
4. Import your identity. Either use a backup file or paste your backup key (you can export it from Settings > Sync > "Export Backup Key" on your original device).
5. Your vaults will sync down from Turso.

The identity import step is important. Without your encryption keys, the synced data is just encrypted blobs that can't be read.

## A Note About Security

The Turso credentials are stored locally in your app settings. The API token gives access to create databases on your account, so keep it safe. Don't share it with anyone you don't trust with your Turso account.

## Useful Links

- [Turso quickstart](https://docs.turso.tech/quickstart)
- [Turso CLI docs](https://docs.turso.tech/cli/introduction)
- [Platform API reference](https://docs.turso.tech/platform/api)
- [Turso pricing](https://turso.tech/pricing) (free tier covers personal use)
