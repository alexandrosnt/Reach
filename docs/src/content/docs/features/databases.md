---
title: Databases
description: PostgreSQL, MySQL, MariaDB, SQL Server, SQLite and Redis — browse, query, edit and design, directly or through SSH.
---

The Databases workspace connects to **PostgreSQL**, **MySQL**, **MariaDB**, **SQL Server**, **SQLite** and **Redis**. Servers that speak the PostgreSQL or MySQL protocol — CockroachDB, TimescaleDB, Supabase, Neon, TiDB, PlanetScale — work as those engines. Everything is built into Reach, on Windows, macOS, Linux and Android; there are no drivers or client tools to install.

## Switching it on

Databases is off until you switch it on in **Settings → DevOps** (see [DevOps Tools](/settings/devops/)). A **Databases** tab then appears next to Terminal.

## Connecting

Click **+** at the top of the Databases sidebar. Pick the database type, then how to reach it:

- **Directly, from this device** — host and port as usual.
- **Through a saved SSH session** — Reach logs in with that session (its key, password, jump hosts and proxy) and forwards the database port for you. The host is where the database is *as that server sees it*, usually `127.0.0.1`. No port forwarding to set up by hand.

The password is stored encrypted in your vault with the rest of the connection. **Test connection** tries it without saving.

Two switches decide how careful Reach is with a connection:

- **Read-only** — only reads run. Reach refuses anything else, and on PostgreSQL, MySQL, MariaDB and SQLite the session itself is put in read-only mode too.
- **Production** — every change asks first, not only the destructive ones, and the connection is tinted red. Destructive changes also ask you to type the connection's name.

A label colour marks the connection in the sidebar and on its tabs.

### Finding databases on your servers

At the bottom of the sidebar, **On your machines** lists this device and every server you have a terminal tab open to. **Find databases** looks at what is listening there — `ss`, running Docker containers, and the well-known ports — and lists what it finds, MariaDB told apart from MySQL. Click one and the connection form opens filled in, going through that same SSH connection. You only add the password.

If a MySQL or MariaDB login fails with a message about a sign-in method Reach cannot use (Windows/GSSAPI, ed25519, unix_socket), the account has no password login. Enter its password if it has one, or connect as a user that does.

## The tree

An open connection shows its databases, then schemas (PostgreSQL, SQL Server), then tables, views, functions, procedures and sequences. The filter box at the top narrows it by name. Right-click anything for what you can do with it: open, design, query, import, export, back up, empty or drop. Dropping and emptying go through the same confirmation as SQL typed in the editor.

## Queries

Each query tab has its own session on the server, so a transaction or `SET` in one tab stays there.

- **Ctrl+Enter** runs the selection, or the statement under the cursor.
- **Ctrl+Shift+Enter** (or **F5**) runs everything.
- **Stop** cancels a running statement from a separate session.
- **Explain** shows the database's plan for the statement.
- Autocomplete knows the tables and columns of the database you are in.

Results appear one tab per result set, with a **Messages** tab for row counts, timings and errors. **History** on the right keeps the queries you ran on this device. The two lightbulb buttons ask the [AI Assistant](/features/ai-assistant/) to explain or speed up the query.

Before anything runs, Reach reads the script. `DROP`, `TRUNCATE`, and `UPDATE` or `DELETE` without a `WHERE` are held back and shown to you with the reason; nothing in the script runs until you say yes.

## Editing rows

Open a table to see its rows a page at a time. Type a condition in the filter box (`status = 'open'`) to narrow them, click a column header to sort (Shift-click to sort by more than one).

Double-click a cell, press Enter, or just start typing to edit. Changed cells turn yellow, new rows green, deleted rows are struck through. Nothing is written until **Save**, which shows the exact `INSERT`, `UPDATE` and `DELETE` statements and runs them as one transaction. If someone else changed or removed one of those rows in the meantime, nothing is saved and Reach tells you which.

A table without a primary key opens read-only, because a row cannot be picked out safely; edit it with SQL or give it a key in the designer.

## Designing tables

**Design table** opens the table designer, laid out like Navicat's:

- **Fields** — name, type, length, decimals, not null, key and comment, with a property panel below for the default, auto-increment or identity, collation, generated expressions and, on MySQL, unsigned and on-update.
- **Indexes**, **Foreign keys**, **Checks** and **Triggers**.
- **Options** (MySQL and MariaDB) — storage engine, character set, collation, next auto-increment value.
- **Comment**, and **SQL Preview**, which updates as you type.

Renaming a field keeps its data. **Save** shows the `CREATE` or `ALTER` statements before they run. On PostgreSQL, SQL Server and SQLite they run in one transaction; MySQL applies each change on its own, and says so. SQLite cannot alter most things in place, so Reach rebuilds the table the way SQLite's documentation describes, keeping its rows, indexes and triggers.

## Backup, restore, import and export

- **Back up** writes a plain `.sql` file: tables, rows, indexes, keys, triggers, views and routines, in an order that restores cleanly into an empty database. Reach writes it itself, so it works on every platform without `pg_dump` or `mysqldump`.
- **Restore** runs a `.sql` file statement by statement, stopping at the first error or carrying on and listing what failed. MySQL `DELIMITER` lines and SQL Server `GO` lines are understood.
- **Import CSV** shows the file's first rows, pairs columns by name, and imports everything in one transaction.
- **Export** a table or a query's result as CSV, JSON or `INSERT` statements.

## Server monitor

Right-click a connection → **Server monitor** lists who is connected and what they are running, refreshed every few seconds. **Cancel query** stops a statement; **End session** disconnects it. Reach's own sessions are marked and cannot be ended from the list.

## Redis

A Redis connection opens its own workspace:

- **Keys** — search with a pattern (`user:*`), walked with `SCAN` so a large server never stalls. Each key shows its type and time to live.
- **Values** — an editor per type: strings, hashes, lists, sets, sorted sets and streams. Rename a key, set or remove its TTL, add and remove items. Large collections show their first thousand items and say so.
- **Console** — any command, answered the way `redis-cli` prints it. `FLUSHALL`, `FLUSHDB`, `KEYS`, `CONFIG SET` and `SHUTDOWN` ask first.
- **Info** and **Clients**, with a button to disconnect a client.

## Android

Everything works on Android except opening SQLite files and choosing backup files from the system file picker, which hands Reach a content link rather than a path.
