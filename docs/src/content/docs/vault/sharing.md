---
title: Sharing & Collaboration
description: Share vaults and secrets with your team.
---

Reach supports two kinds of sharing: sharing an entire vault with team members, or sharing individual secrets as a one-off.

## Vault Sharing

This is the main way to collaborate. Here's how it works:

1. Create a shared vault (it will set up a Turso database for syncing).
2. Go to "Manage Members" on the vault.
3. You'll see your UUID and public key. Send these to the person you want to invite.
4. They send you their UUID and public key.
5. Enter their info, pick a role (Admin, Member, or Read Only), and send the invite.
6. The recipient enters the sync URL and token to accept the invite.
7. Done. You both have access to the same vault now. All secrets sync through Turso.

### Roles

| Role | View Secrets | Add Secrets | Manage Members |
|------|-------------|-------------|----------------|
| **Owner** | Yes | Yes | Yes |
| **Admin** | Yes | Yes | Yes (can invite) |
| **Member** | Yes | Yes | No |
| **Read Only** | Yes | No | No |

## Secret Sharing (One-Off)

Sometimes you just need to send someone a single secret without giving them access to a whole vault.

1. From any vault, click "Share" on a specific secret.
2. Enter the recipient's UUID and public key.
3. Optionally set an expiration (in hours).
4. The recipient can accept the share into any of their vaults.

## Security

All sharing uses X25519 key exchange. The secret is re-encrypted for the recipient's public key. At no point does the plaintext travel over the network. Turso only ever sees encrypted data.
