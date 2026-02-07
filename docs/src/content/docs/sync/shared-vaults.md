---
title: Syncing Shared Vaults
description: How shared vaults work with cloud sync.
---

Shared vaults need Turso to work. When you create a shared vault, Reach automatically creates a Turso database for it.

## The Flow

1. Make sure you have Turso set up (**Settings > Sync**).
2. Create a new vault and pick "Shared" as the type.
3. Reach creates a Turso database with a name like `vault-myname-timestamp`.
4. Invite members (see [Sharing & Collaboration](/vault/sharing)).
5. When they accept the invite, their Reach connects to the same Turso database.
6. Any secrets added to the vault now sync to all members.

## End-to-End Encryption

All data is encrypted end-to-end. Turso stores encrypted blobs. Members decrypt locally with their own keys. Nobody in between can read your secrets. Not Turso, not your ISP, nobody.

## Who Pays for the Database?

The vault owner's Turso account hosts the database. Members connect using a token generated during the invite process. The owner's API token is never shared with members.

This means the owner's Turso free tier usage goes up with each shared vault. For most teams this isn't an issue, but keep it in mind if you're creating a lot of shared vaults.

## Deleting a Shared Vault

If you delete a shared vault, the Turso database is deleted too. All members lose access immediately. Make sure everyone knows before you pull the plug.
