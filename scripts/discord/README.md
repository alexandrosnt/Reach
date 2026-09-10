# Discord server setup

Two separate things, on purpose.

**The bot** is a throwaway. It creates the channel structure once and exits.
It does not stay online, it has no gateway connection, and nothing in the
server depends on it afterwards — you can kick it the moment it finishes.

**The GitHub feed** is not the bot. Discord has a built-in receiver for
GitHub's webhook payloads, so GitHub POSTs straight to Discord and nothing of
ours has to be running for updates to appear. A bot polling GitHub would need
to run forever to do what this does for free.

---

## 1. Create the bot

1. <https://discord.com/developers/applications> → **New Application**, name it
   something like `Reach Setup`.
2. **Bot** in the sidebar → **Reset Token** → copy it. This is the one secret
   here; it is shown exactly once.
3. Leave every Privileged Gateway Intent **off**. The script only uses the REST
   API, so it needs none of them.

## 2. Invite it

**OAuth2 → URL Generator**:

- Scopes: `bot`
- Bot Permissions: **Manage Channels**, **Manage Webhooks**

Open the generated URL and add it to the Reach server. Those two permissions
are all it needs; do not grant Administrator.

## 3. Give the script the two values

Get the guild id: in Discord, **Settings → Advanced → Developer Mode** on, then
right-click the server icon → **Copy Server ID**.

Put both in `.env` at the repo root (already gitignored):

```
DISCORD_BOT_TOKEN=your-token-here
DISCORD_GUILD_ID=your-server-id-here
```

## 4. Run it

Dry run first — this prints the plan and changes nothing:

```
npm run discord:setup
```

Then for real:

```
npm run discord:setup -- --apply
```

It is idempotent: it matches on category + channel name and creates only what
is missing, so re-running after editing `plan.mjs` adds the new channels and
leaves the rest alone. It never renames, retopics or deletes anything — those
stay manual, so the script can never undo a change you made in the client.

Edit `plan.mjs` to change the structure. Don't edit `setup.mjs` for that.

## 5. Wire up GitHub

`--apply` creates two webhooks and writes their URLs to
`.discord/webhooks.json` (gitignored). **A webhook URL is a posting
credential** — anyone holding it can post into that channel as that webhook.
Treat it like a token: don't paste it into a chat, an issue, or a screenshot.

In the repo → **Settings → Webhooks → Add webhook**, twice:

| Channel     | Payload URL                    | Content type       | Events                                              |
| ----------- | ------------------------------ | ------------------ | --------------------------------------------------- |
| `#releases` | the `githubUrl` for `releases` | `application/json` | Let me select individual events → **Releases** only |
| `#github`   | the `githubUrl` for `github`   | `application/json` | Pushes, Pull requests, Issues, Issue comments        |

The `/github` suffix is what tells Discord to parse GitHub's payload shape
instead of treating it as a plain webhook message. The values in
`webhooks.json` already have it appended — use the `githubUrl` field, not
`url`.

Set a **Secret** on the GitHub side too. GitHub signs each delivery with it,
and Discord verifies the signature, so nobody who guesses the URL can forge a
release announcement.

Two feeds rather than one because they have different audiences: most people
want to know that v0.5.2 shipped, and only contributors want every push and
issue comment. Anyone can mute `#github` and keep `#releases`.

### Checking it worked

GitHub → the webhook → **Recent Deliveries**. A green tick with `204` means
Discord accepted it. `401` means the URL is wrong; a `400` usually means the
`/github` suffix is missing.

## 6. Clean up

Once the channels exist, the bot has no further job. Kick it, or at minimum
remove Manage Channels. Keep the token out of the repo either way — if it ever
leaks, reset it in the Developer Portal, which invalidates the old one
immediately.
