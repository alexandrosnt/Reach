---
title: MCP Server
description: Let an AI client read a terminal session you choose, and type into it only on your terms.
---

Reach can expose a terminal session to an AI client — Claude Code, Cursor, Gemini CLI, and others — over the **Model Context Protocol**. The AI sees the session's output, with secrets redacted, and can send commands only through a guard that enforces its rules rather than suggesting them.

Nothing is exposed unless you turn the server on, and nothing is shared unless you share it.

## Turning it on

Go to **Settings > MCP** and turn on **MCP server**.

The server listens on `127.0.0.1` only. It is never reachable from another machine. Leave the **Port** at `0` to let the system pick a free one, or set one if a client needs it fixed.

Turning the server on shares nothing by itself. Each session is shared individually.

## Sharing a session

Every terminal tab has its own share control in the bottom bar. Only the sessions you share are visible to the AI; everything else does not exist as far as it is concerned.

Stop sharing from the same place, or from the list under **Settings > MCP**, which shows each shared session by name — `root@10.144.144.2`, not a UUID — along with its agent and mode.

## Agents

An agent decides **which tools** the AI gets, not just how it behaves. Choose one under **Settings > MCP**:

| Agent | What it can do |
| --- | --- |
| Linux Administrator | Day-to-day administration: packages, services, users, filesystems. Can type. |
| DevOps Engineer | Pipelines, deployments, containers, infrastructure as code. Can type. |
| DevSecOps Engineer | Hardening, access, certificates, audit. Can type. |
| Architect | Designs and reviews. **Read only** — has no tool that can type. |

An agent can only narrow what the guard allows. It can never widen it.

You can override the agent per session from the bottom bar, so a production box can be pinned to the read-only Architect while a scratch machine gets the Administrator.

## Modes

The mode decides how much you are asked. The guard applies in every mode — a mode only decides whether a human is consulted first.

| Mode | Behaviour |
| --- | --- |
| Ask | Every command is shown to you with the AI's reasoning. Nothing runs until you approve it. |
| Auto (safe) | Runs commands without asking, except ones classed as destructive, which still ask. |
| Auto | Runs everything without asking. |
| Dangerous | Runs everything, including commands that destroy data, without asking. Chosen by you, warned about, and never the default. |

Choose the mode from the bottom bar, globally or per session.

## What the guard enforces

These are enforced by the server. An AI cannot talk its way past them.

- **Read before write.** The AI must have read the session's current output before it may send a command. If the screen changed since it last looked, the command is refused and the missed output is handed back with the refusal, so the next attempt is informed.
- **Describe before act.** The AI must ask what the session is — host, user, shell — before typing into it.
- **No typing while a password is being entered.** When the terminal turns echo off, sending input is locked until it comes back on.
- **A reason, every time.** Every command must come with a rationale worth showing to a human.
- **No secrets in commands.** A command that contains something that looks like a key, token or password is refused.
- **A rate limit.** Bursts of commands are throttled.

A refusal is returned to the AI as a normal tool result with an explanation, never as a protocol error, so a well-behaved client reads it and adjusts.

## What the AI sees

Session output reaches the AI through a rolling buffer with secrets redacted — API keys, tokens, private keys and the like are replaced before they leave Reach.

Every command the AI sends is echoed into your terminal under a dim banner naming the agent, so nothing happens off-screen.

## Connecting a client

**Settings > MCP > Connect your client** lists eleven clients with the exact command or configuration each one needs. They are not interchangeable: clients disagree on the top-level key, the URL field and even the spelling of the transport, and the wizard records each one's own form.

For Claude Code, for example:

```bash
claude mcp add --transport http reach http://127.0.0.1:3000/mcp \
  --header "Authorization: Bearer <token>"
```

Copy the command for your client from the wizard rather than from here; it fills in your actual port and token.

## The token

Every request must carry the token shown under **Settings > MCP**. It is stored encrypted in your vault and reused across restarts, so a client configured once keeps working.

**Regenerate** replaces it and immediately locks out every client configured with the old one. Reach asks before doing that.

## Sessions are yours

Sharing is never restored automatically. Restarting Reach brings the server back if you left it on, because that is a decision you made — but it does not re-share sessions, because that would be making one for you.
