---
title: Snippets
description: Save the commands you keep retyping, and complete them with Tab.
---

Snippets are the commands you type often enough to resent typing. Save one,
start typing it, press <kbd>Tab</kbd>.

They are not scripts. A snippet is a single command with a short name, and
nothing about it is analysed, confirmed or sandboxed — it is your own text,
typed for you. For anything with parameters, a risk level and a confirmation
step, see [Recipes](../recipes/).

## Saving one

Open the **Snippets** panel from the sidebar rail and add one. A snippet has:

| Field | What it is for |
| --- | --- |
| **Name** | What you type to summon it |
| **Command** | The text that gets inserted |
| **Description** | Optional reminder of what it does |
| **Tags** | Optional grouping, so a long list stays searchable |

The name is what matters day to day. `dcl` for `docker container ls -a`,
`logs` for `journalctl -fu`, `ports` for `ss -tulpn`. Short enough to be worth
it.

## Completing in the terminal

Type the first characters of a snippet's name in any terminal and the match
appears. <kbd>Tab</kbd> accepts it and the full command replaces what you
typed. Carry on typing and it disappears — accepting is always deliberate.

Matching is by prefix, against a trie, so lookup cost depends on how much you
have typed rather than on how many snippets you have. A hundred snippets feel
the same as three.

Only the first complete match is offered. If `log` and `logs` both exist,
typing `log` offers `log`; you have to type the `s` to reach the other. Names
that prefix one another are worth avoiding for that reason.

## Where they live

Snippets are stored with the rest of your Reach data, not on any server, and
they are not tied to a session — a snippet saved while connected to one machine
is available on every machine. They are yours, not the server's.

## Snippets or recipes?

Both put text into a terminal, and the difference is how much ceremony that
deserves.

- **Snippet** — one command, no parameters, no checks, inserted on
  <kbd>Tab</kbd>. For things you would otherwise type from memory.
- **[Recipe](../recipes/)** — a bash script with declared parameters and a
  risk level, analysed before it runs, with anything above *mutating* requiring
  a typed confirmation. For things you would otherwise paste from a wiki.

A rule of thumb: if you would be comfortable typing it blindfolded, it is a
snippet.
