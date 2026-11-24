RDP keyboard fixes; readable TUI dialogs in the light theme.

- RDP: NumLock, CapsLock and ScrollLock follow your keyboard in every
  session. Reported by @ecotwist (#179); Linux fixes by @rsipka (#181).
- RDP: a held key repeats. Reported by @ecotwist (#178).
- Light theme: TUI dialogs (such as gdu's) show their text instead of a
  black box. Reported by @elizabevil (#174).
- Hardening of the local MCP server's request limits and the Windows
  Passbolt CLI permission check.

Full details: https://github.com/alexandrosnt/Reach/blob/main/CHANGELOG.md
