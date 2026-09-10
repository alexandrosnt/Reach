# Discord server setup

Three separate things, on purpose.

**The bot** is a throwaway. It brings the server in line with `plan.mjs` and
exits. It holds no gateway connection and nothing depends on it afterwards —
kick it the moment it has run.

**The GitHub feed** is not the bot. Discord has a built-in receiver for
GitHub's webhook payloads, so GitHub POSTs straight to Discord and nothing of
ours has to be running. A bot polling GitHub would need to run forever to do
what this does for free.

**The verifier** asks Discord what is actually true, rather than trusting what
the setup script says it did. Those two came apart once already.

## The files

| File               | What it is                                                    |
| ------------------ | ------------------------------------------------------------- |
| `plan.mjs`         | The server as data: roles, categories, channels. **Edit this.** |
| `permissions.mjs`  | Permission bits by name. `bits()` throws on a name it doesn't know, so a typo fails loudly instead of granting nothing. |
| `setup.mjs`        | The runner. Rarely needs editing.                              |
| `verify.mjs`       | Read-only check of the live server against the plan.           |
| `grant.mjs`        | Assigns roles to a member. Additive; never removes one.        |
| `github-webhook.mjs` | Points the GitHub repo at the two feed channels.            |
| `setup.test.mjs`   | Runs the whole thing against a fake Discord. No token needed.  |

```
npm run discord:setup              # dry run — prints the plan, changes nothing
npm run discord:setup -- --apply   # for real
npm run discord:verify             # read the live server back and check it
npm run discord:test               # offline, mocked, no token

npm run discord:grant -- Maintainer Contributor           # roles for the owner
npm run discord:grant -- 1234567890 Translator            # roles for someone else

npm run discord:github -- --apply  # point GitHub at the two feed channels
```

---

## 1. Create the bot

1. <https://discord.com/developers/applications> → **New Application**.
2. **Bot** → **Reset Token** → copy it. Shown exactly once.
3. Leave every Privileged Gateway Intent **off**. This only uses the REST API.

## 2. Invite it

**OAuth2 → URL Generator**, scope `bot`, permissions **Manage Channels**,
**Manage Roles**, **Manage Webhooks**.

Nothing to drag afterwards. Discord starts every new role at position 1, the
bot's own included, and a bot may neither order nor assign a role at or above
its own — but it *may* raise itself, so `setup.mjs` does exactly that when it
finds no room, then orders the rest beneath.

## 3. Give the script the two values

Guild id: **Settings → Advanced → Developer Mode** on, then right-click the
server icon → **Copy Server ID**.

`.env` at the repo root (gitignored):

```
DISCORD_BOT_TOKEN=your-token-here
DISCORD_GUILD_ID=your-server-id-here
```

## 4. Run it

Dry run first. Then `-- --apply`.

What it will and will not touch:

| Thing           | Behaviour                                                                 |
| --------------- | ------------------------------------------------------------------------- |
| **Roles**       | Created if missing, then ordered. An existing role is left completely alone — colour, permissions, everything. |
| **Channels**    | Created if missing. Renamed when the plan's emoji or casing differs. Never deleted; an existing topic is never rewritten. |
| **Permissions** | **Authoritative.** Rewritten to match the plan on every `--apply`. |

That last row is the only place this overwrites your manual work, and it is
deliberate: a half-applied permission is worse than none, because the channel
looks locked and is not. If you tune permissions in the client, put the change
in `plan.mjs` too or the next run will undo it.

Matching is by *slug* — the channel name with leading decoration stripped — so
changing an emoji renames the channel in place instead of creating a second
one beside it.

## The structure

```
📌 INFORMATION   👋 welcome   📣 announcements   🎭 roles   🚀 releases ←GitHub
💻 DEVELOPMENT   🔧 dev-chat  🐙 github ←GitHub  🌍 translations
                 🎨 themes    🧩 plugins         🤖 mcp
🆘 SUPPORT       ❓ help      🐛 bug-reports     💡 feature-requests
🌐 COMMUNITY     💬 general   ✨ showcase        🎲 off-topic
🔒 STAFF         🔐 staff-chat   📋 mod-log        (private)
```

## The roles

Three tiers that carry responsibility, hoisted so they show as their own group
in the member list. Then recognition roles, which are deliberately powerless —
they colour a name and make a group pingable, and that is the whole job. A
member list with nine headings is not a structure, it is a wall.

| Role                      | Powers                                                        |
| ------------------------- | ------------------------------------------------------------- |
| **Maintainer**            | Administrator. Bypasses every channel override below.          |
| **Moderator**             | Delete messages, timeout, kick, ban, manage threads and nicknames, view the audit log. **No** Manage Channels, Manage Roles or Manage Guild — moderation should not be able to rewrite the server. |
| **Core Contributor**      | Manage threads, private threads, events.                       |
| Contributor               | none — merged a PR                                             |
| Translator                | none — maintains a locale                                      |
| Theme Author              | none — has a theme in the registry                             |
| Plugin Author             | none — has a plugin in the marketplace                         |
| Bug Hunter                | none — reported something real, reproducibly                   |
| Release Notifications     | none — self-assigned ping, so you never need @everyone         |

`@everyone` gets talking, reacting, threads, external emoji, nickname changes,
voice and invites. Not `MENTION_EVERYONE`, and not a single `MANAGE_*` bit.

## 5. Wire up GitHub

One command:

```
npm run discord:github              # dry run
npm run discord:github -- --apply   # for real
```

It needs `GITHUB_TOKEN` in `.env`, with `admin:repo_hook` (classic) or
Webhooks: Read and write (fine-grained). It reads the Discord URLs from
`.discord/webhooks.json`, then reads the delivery back from GitHub so you can
see the `204` rather than take its word for it.

**A webhook URL is a posting credential** — anyone holding it can post into
that channel as that webhook. It stays in `.discord/`, which is gitignored, and
nothing here ever prints it.

| Channel     | Events                                                        |
| ----------- | ------------------------------------------------------------- |
| `#releases` | `release`                                                     |
| `#github`   | `push`, `pull_request`, `pull_request_review`, `issues`, `issue_comment` |

Two feeds because they have different audiences: most people want to know a
version shipped, only contributors want every push and issue comment. Anyone
can mute `#github` and keep `#releases`.

### Why those events and not others

Discord's `/github` receiver renders a fixed subset of GitHub's events:
`push`, `pull_request`, `pull_request_review`, `pull_request_review_comment`,
`issues`, `issue_comment`, `commit_comment`, `create`, `delete`, `fork`,
`gollum`, `member`, `public`, `release`, `watch`.

Anything else it accepts with a `204` and then does nothing with — which looks
exactly like a working webhook until you notice the channel has been quiet for
a week. **`workflow_run` and `check_run` are not on the list**, so CI results
cannot come through this path; they need a GitHub Action posting an ordinary
Discord message. `setup.test.mjs`'s sibling `github-webhook.test.mjs` asserts
the plan contains nothing outside that set.

`create` and `delete` are supported but left out on purpose: branch and tag
events drown an active repo.

### Doing it by hand instead

Repo → **Settings → Webhooks → Add webhook**.

| Field | Value |
| ----- | ----- |
| Payload URL | the `githubUrl` field from `.discord/webhooks.json` — **not** `url` |
| Content type | `application/json` |
| Secret | **leave empty** |
| SSL verification | Enable |
| Events | *Let me select individual events*, per the table above |

The `/github` suffix is what switches Discord from its own payload format to
GitHub's; without it, deliveries are rejected.

`application/json` matters just as much. GitHub's form defaults to
`x-www-form-urlencoded`, which Discord's receiver does not parse — leave the
default and you get deliveries that look successful and post nothing.

### Why no secret

A GitHub webhook secret is a shared key: GitHub signs each delivery with it and
the *receiver* verifies the signature. That only works if the receiver has been
given the same secret. Discord never is — there is no field for it anywhere in
Discord — so it cannot verify anything, and GitHub would just be computing an
HMAC for a header nobody reads.

The webhook URL is the credential. Anyone holding it can post to that channel,
secret or no secret, which is why `.discord/` is gitignored and nothing here
ever prints it. If the URL leaks, delete the Discord webhook and re-run
`discord:setup -- --apply` to mint a new one.

**Checking it worked:** GitHub → the webhook → **Recent Deliveries**. Green
tick and `204` means Discord accepted it. `401` means the URL is wrong; `400`
usually means the `/github` suffix is missing.

## 6. Clean up

Once the server is built the bot has no further job. Kick it, or at least strip
Manage Roles and Manage Channels. Reset the token in the Developer Portal if it
has ever been pasted anywhere — that invalidates the old one immediately.
