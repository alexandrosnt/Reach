Failed uploads say why and can be cancelled; new tab actions; the explorer can follow the terminal.

- File explorer: uploading to a folder you can't write (such as /opt)
  hung forever at "0.0 MB" or partway. It now fails at once with the
  server's message. Uploads and downloads can be cancelled, and the
  transfer list clears on disconnect. Reported by @elizabevil (#161).
- Tabs: right-click a tab to Duplicate it, or close the others, the
  ones to its left or the ones to its right. The command palette has
  the same. Requested by @elizabevil (#160).
- File explorer: a button opens the terminal's current folder, and a
  Follow switch keeps it in step as you `cd` (Linux servers).
  Requested by @elizabevil (#162).

Full details: https://github.com/alexandrosnt/Reach/blob/main/CHANGELOG.md
