---
title: How Encryption Works
description: A look at how Reach protects your data.
---

Everything sensitive in Reach is encrypted at rest. Sessions, passwords, secrets, API keys, playbooks. All of it. Nothing is stored in plaintext.

## The Encryption Stack

Reach uses three well-known cryptographic primitives:

- **XChaCha20-Poly1305** for data encryption (fast, authenticated encryption)
- **Argon2id** for key derivation from passwords (memory-hard, resistant to brute force)
- **X25519** for key exchange when sharing secrets between users

## How It Works

Here's the high-level flow:

1. When you initialize your identity, Reach generates an X25519 keypair. The secret key gets stored in your OS keychain (Windows Credential Manager, macOS Keychain, or Linux Secret Service).

2. A Key Encryption Key (KEK) is derived from your secret key using HKDF-SHA256.

3. Each piece of data gets its own random Data Encryption Key (DEK). The data is encrypted with the DEK, and the DEK is wrapped (encrypted) with the KEK.

4. This is called **envelope encryption**. Even if someone gets the encrypted data, they need the KEK to unwrap the DEKs. And the KEK comes from the secret in your OS keychain.

## Auto-Unlock

Because the secret key lives in your OS keychain, Reach can decrypt everything without asking for a password every time you open the app. The OS protects the keychain with your login credentials. So as long as you're logged into your computer, Reach can access the key.

## Password Fallback

If you want to use Reach on multiple devices, you can set a master password. This derives a separate KEK using Argon2id (256 MB memory, 4 iterations) so you can unlock the vault with a password instead of relying on the keychain.

This is also useful as a recovery option if your keychain ever gets corrupted.

## Local Only

All encryption happens locally. Nothing leaves your machine unless you explicitly set up cloud sync. And even then, only encrypted blobs are transmitted. The keys stay with you.
