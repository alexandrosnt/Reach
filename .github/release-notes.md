Passwords from Passbolt, folders that sync, and a terminal that fits a phone.

- A session or database connection can take its password from Passbolt
  instead of keeping a copy in Reach's vault: Settings → Password
  managers. The password is fetched when you connect and never stored,
  shown or logged. Off by default, desktop only. Contributed by @qwerin.
- A shared vault's folders now sync to your other devices and to the
  vault's members. Existing folders move into their vault on their own.
  Reported by @swjpilot.
- The terminal on a phone: a real monospace font on every phone, pinch
  or A−/A+ to change the text size, the extra-keys bar stays above the
  keyboard, and a status bar that fits. Reported by @mival.
- Changing the terminal's text size (Ctrl+wheel too) no longer empties
  it.
- Security: passwords Reach 0.1.0 saved in plain text in its web
  storage, unused since 0.1.1, are deleted on first start.
- The generic settings and vault commands can no longer open the vaults
  Reach keeps for its own data.

Full details: https://github.com/alexandrosnt/Reach/blob/main/CHANGELOG.md
