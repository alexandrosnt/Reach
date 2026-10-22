Security fix: passwords and keys no longer reach the log file.

- Up to 0.8.0, creating or editing a saved session wrote its password,
  key passphrase, embedded key and proxy password into reach.log, and
  opening a synced vault its token. 0.8.1 logs none of them and deletes
  the old log files on first start.
- If you ever shared a Reach log file, change the passwords of the
  sessions you created or edited before this update.

Reported by @qwerin.

Full details: https://github.com/alexandrosnt/Reach/blob/main/CHANGELOG.md
