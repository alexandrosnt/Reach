Multi-line snippets arrive whole; long links open in full; a security update.

- Snippets: a snippet of several lines lost the start of a line when a
  program asked something in between (`apt upgrade` arrived as
  `t upgrade`), and the palette dropped the last line. It now goes in
  as one paste. Reported by @coRpTitan (#154).
- Terminal: clicking a long link opens all of it, not just the part
  before a `!`, `*`, `'` or bracket.
- Security: pageant 0.2.5, so a malformed reply from the Windows SSH
  agent can no longer make Reach buffer without limit.

Full details: https://github.com/alexandrosnt/Reach/blob/main/CHANGELOG.md
