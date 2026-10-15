---
title: Password Managers (Passbolt)
description: Let a session take its password from Passbolt instead of keeping a copy in Reach's vault.
---

If your team already keeps its server passwords in a password manager, a session in Reach can point at the entry there instead of holding a copy of the password. Reach fetches the password when you connect, uses it for the login, and forgets it. Change the password in the manager and every session that points at it uses the new one the next time it connects.

The first password manager Reach can read from is **[Passbolt](https://www.passbolt.com/)**, through Passbolt's own command-line client, **go-passbolt-cli**. The integration is built so that others (Bitwarden, 1Password, KeePassXC) can follow the same way.

## What you need

- A Passbolt server, version 5 (the current release, with encrypted metadata) or 4.
- **go-passbolt-cli 0.4.0 or newer** on the computer running Reach. Its program is called `passbolt`.
  - **macOS:** `brew install go-passbolt-cli`
  - **Linux:** the `.deb` or `.rpm` from the [releases page](https://github.com/passbolt/go-passbolt-cli/releases), or `passbolt-cli` from the AUR
  - **Windows:** the `windows_amd64.zip` from the [releases page](https://github.com/passbolt/go-passbolt-cli/releases). Put `passbolt.exe` in a folder on your PATH, or tell Reach where it is.
- Your Passbolt **private key** (the account recovery kit, `passbolt_private.txt` / `.asc`) and its **passphrase** (which Reach can ask for when needed instead of storing it). The key file must be on this computer (not on a network share) and at most 32,000 bytes; a real key is a few kilobytes.

Password managers are a desktop feature. On Android the setting is shown but the provider reports that it is not available.

## Setting it up

Open Settings with **Ctrl+,** (**Cmd+,** on Mac) and choose **Password managers**.

1. Switch **Password managers** on. It is off on a fresh install, and while it is off Reach refuses every password-manager command, not only the page.
2. In the **Passbolt** card, check that Reach found the CLI and its version. If it did not, install it (above) or enter the path to `passbolt` / `passbolt.exe`. The path is kept **on this device only**: it is not synced with your vault, so each computer finds or is told its own CLI. Reach runs only a program actually named `passbolt` or `passbolt.exe` (no `.cmd`, `.bat` or other scripts), only from this computer (not from a `\\server\share` path), and on macOS and Linux only if the file and its folder belong to you or to root and cannot be changed by other users. The same rules apply to a `passbolt` found on PATH.
3. Choose where the login comes from:
   - **Stored by Reach** (the default): enter the server address (`https://…`) and choose your private key file (or paste it). The key is kept in Reach's encrypted vault, in the settings vault, and is never shown again: the page only says whether one is saved, and offers to remove it.
   - **My go-passbolt-cli configuration**: Reach runs the CLI with the configuration you made with `passbolt configure`, and stores nothing itself.
4. With **Stored by Reach**, choose what happens to the key's passphrase:
   - **Ask when needed** (the default for a new setup): Reach never saves the passphrase. When it next signs in to Passbolt (a connect, the entry search, *Test connection*) it asks for it in a small dialog that says what it is for, for example *To connect to "web-01"*. Choose under **Remember for** how long Reach keeps what you typed: **Ask every time**, 5 minutes, 15 minutes, 1 hour or 4 hours. The time counts from when you typed it; using the passphrase does not extend it. **Forget now** drops it at once. With *Ask every time*, one connect still asks only once, even when it needs several entries (a jump host and the target, say). If Passbolt rejects the passphrase, Reach forgets it and asks again, up to three times; *Cancel* stops the connect. A question left unanswered for three minutes is given up.
   - **Store in Reach's vault**: type the passphrase once; it is kept in the settings vault next to the key, never shown again, and used without asking.

   Switching to *Ask when needed* and saving removes a stored passphrase: Reach never keeps both. A Reach that had a passphrase stored before this choice existed keeps using it (it shows *Store in Reach's vault*); one without asks.
5. If your Passbolt account uses two-step verification, choose **TOTP** and enter the TOTP secret (the base32 text behind the QR code), so Reach can answer the challenge itself.

   :::caution
   A TOTP secret stored here is **not a second factor** any more. It sits next to the private key (and the passphrase, if you store it) in your vault, and it syncs with the vault to every device that opens it: whoever can open your Reach vault can sign in to Passbolt as you, codes included. Store it only if that is acceptable for your account, or use your own go-passbolt-cli configuration instead.
   :::
6. Press **Test connection**. Reach logs in (asking for the passphrase if it is not stored) and lists your entries, and says how many it can see — or why it could not.

## Using a Passbolt entry in a session

In the session editor, every password field has a key button, **From password manager**. It opens a search over your Passbolt entries (name, username and URL); pick one and the field becomes a chip, *Passbolt · name of the entry*. Remove the chip with its × to type a password again.

It works for:

- SSH passwords and key passphrases
- every jump host's password or key passphrase
- the proxy password
- RDP and VNC passwords
- database connections, both the database password and a session the connection goes through

A session that points at an entry connects without asking for a password. Reconnecting fetches the password again, so a password changed in Passbolt in the meantime is picked up.

## What Reach does and does not do

- **The password never reaches the window.** The session stores only *which* entry it uses (Passbolt's id for it, and its name for display). The password is fetched by Reach's backend at connect time and handed to the SSH, RDP, VNC or database login. It is not stored anywhere and never written to disk, to the session, to the vault or to the log. For SSH, RDP and VNC it is kept in memory for the login only; for a database it stays in memory as long as the connection is open, since the connection opens further sessions to the server with it, and is gone once you disconnect.
- **The list shows names, not passwords.** Searching your entries asks Passbolt for names, usernames and URLs only; no password is decrypted to fill the list. The list is kept in memory for five minutes so the search is quick, filed under the account it came from, and cleared when you change the settings or lock the vault.
- **Your key goes to the CLI privately.** Reach passes the server, private key and passphrase to go-passbolt-cli in its environment, never on its command line, where other programs could read them in the process list. The CLI starts from an almost empty environment: only the program path, the system, temporary and home folders and your proxy settings are passed on from Reach's, so variables such as `GODEBUG` or `SSL_CERT_FILE` do not reach it (a private certificate authority has to be in the system's certificate store). Debug output and skipping the TLS check are always switched off, even if your environment or your own go-passbolt-cli configuration file turns them on. When Reach stores the login, the CLI is pointed away from your own `passbolt configure` file, so the two cannot mix.
- **A passphrase you type is kept encrypted, in memory only.** With *Ask when needed*, what you type goes straight from the dialog to Reach's backend and is never written to disk, to the vault or to the log, nor sent back to the window. While Reach remembers it, it is held encrypted with the same in-memory shield as the vault's own keys: sealed under a random key made when Reach starts, which is locked into RAM where the system allows it. It is opened only for the moment it is handed to go-passbolt-cli; from then on it is in the CLI's environment, like a stored passphrase, for as long as that run lasts (a few seconds). It is forgotten when its time is up, when you press *Forget now*, save the Password managers settings, lock the vault, quit Reach, or when Passbolt rejects it. It is filed under the account it was typed for, so changing the server or the key never hands it to another account.
- **Every fetch is a full Passbolt login.** go-passbolt-cli keeps no session between runs, so a connect takes a moment longer than with a stored password (usually a second or two, more with two-step verification). Each one shows up in Passbolt's activity log as a login by you. Reach runs at most two of them at once and makes the entry search share one, so a jump chain or a reconnect does not flood the server. On Windows, a CLI still running when Reach quits is stopped with it.
- **Passbolt decides who sees what.** Reach can only read what your Passbolt account can read. Sharing a session through a shared vault shares *which* entry it uses, not the password: a teammate connects only if Passbolt has shared that entry with them too.

## References you did not pick

A session in a shared vault can be edited by everyone who can write to that vault. Without a check, someone could keep the reference to *your* Passbolt entry and change the host to a server of theirs: your Reach would fetch your password and log in there with it.

So a reference counts only for the person who picked it, and only for where it was picked for. When you pick an entry, Reach signs the choice with a key only you have, together with the session's host, port, user, jump hosts and proxy. A reference someone else picked, or one whose session has since been moved to another host or route, is not used until you confirm it. Reach asks before connecting:

> 'web-01' wants to send your Passbolt entry 'Production DB' to admin@db.example.com:22. Only allow this if you trust whoever last edited this session.

The entry's name in that question is looked up in your own Passbolt by its id, never taken from the session, so it cannot be dressed up as something harmless. In the session editor such a reference shows *not confirmed*. Renaming a session, moving it to another folder or changing its tags keeps your confirmation; changing where it connects does not. The question also lists the session's ssh_config lines that change where it connects (`HostName`, `ProxyJump`, `ProxyCommand`), since those win over the host shown.

The same goes for a session reached through another one:

- **A VNC session that goes through an SSH session** is confirmed for that SSH session as it is, too. If someone changes the SSH session's host, port, user, jump hosts, proxy or ssh_config settings, or deletes it, the VNC session asks again before it sends your password, and the question names the SSH session and where it goes.
- **A database connection that goes through a saved session** is confirmed when you save it: saving signs the session as it is then. If that session is later pointed somewhere else, the connection is refused with *The session this connection goes through has changed since you saved it*; open the connection in the database editor and save it again to confirm. *Test connection* in the editor, and connecting without saving, use the session as it is at that moment, since you are choosing it right then.

## Several devices, several Reach versions

The reference to the entry is stored with the session and syncs with it. Reach versions from before this feature do not know it: they still open the session, but **saving it in an older Reach drops the reference**, and the session is left without a password. Update Reach on every device that edits these sessions, or edit them only where Reach is up to date.

## Troubleshooting

| Message | What to do |
|---|---|
| *go-passbolt-cli was not found* | Install it, or enter the path to `passbolt` in the Passbolt card. |
| *Wrong passphrase or private key* | Enter the passphrase again, or choose the key file again. With *Ask when needed*, Reach asks again by itself, up to three times. |
| *Passbolt: cancelled* / *Passbolt: passphrase not entered* | The passphrase dialog was cancelled, or left open for three minutes. Connect again. |
| *Passbolt asks for two-step verification* | Choose TOTP and enter the TOTP secret, or use your own go-passbolt-cli configuration with `mfaMode` set. |
| *The entry was not found* | The entry was deleted in Passbolt, or is no longer shared with you. Pick another one in the session editor. |
| *The session this connection goes through has changed since you saved it* | Someone changed where that session connects. Check it, then open the database connection in the database editor and save it again. |
| *Password managers are turned off* | Switch them on in Settings → Password managers. |
