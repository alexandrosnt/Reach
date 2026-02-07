---
title: What is Turso?
description: Why Reach uses Turso for cloud sync.
---

Turso is a cloud database service built on libSQL (a fork of SQLite). Reach uses it to sync vaults across devices and between team members.

## Why Turso?

Because it gives you a SQLite-compatible database that lives in the cloud. Your encrypted vault data gets stored on Turso's servers, and any device with Reach can connect to it. The data is encrypted before it leaves your machine, so Turso only ever sees encrypted blobs.

## Do I Need It?

No. You don't need Turso to use Reach. Everything works locally without it. Turso is only needed if you want:

- **Cloud sync** - access your sessions and secrets from multiple devices
- **Shared vaults** - collaborate with team members

If you're a solo user on one machine, you can skip this entirely.

## Pricing

Turso has a free tier that's more than enough for personal use. You'll need to create an account at [turso.tech](https://turso.tech) and generate a Platform API Token.

## How Reach Uses It

Reach creates databases on your Turso account automatically. Each shared vault gets its own database. Your personal sync (if enabled) also gets its own database. You don't need to create or manage any databases yourself.
