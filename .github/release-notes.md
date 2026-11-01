Personal sync deletes and edits work again; unreadable files show an error.

- Personal sync: deleting or editing a session, folder or snippet did
  nothing after sync was turned on. Fixed, and data synced before is
  repaired on first start. Reported and partly fixed by @ahhexia
  (#148, #149).
- File explorer: downloading, previewing or dragging a file you can't
  read gave an empty file with no error. It now shows the server's
  error, and a download cut short leaves no file. Delete, rename and
  new file/folder show the server's error too. Reported by @coRpTitan
  (#150).

Full details: https://github.com/alexandrosnt/Reach/blob/main/CHANGELOG.md
