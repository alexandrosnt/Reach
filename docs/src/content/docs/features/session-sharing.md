---
title: Session Sharing
description: Share a terminal with another person, peer to peer, encrypted end to end, with no server in between.
---

Session sharing puts your terminal and another person's side by side. Your terminal is always on the left. Theirs appears on the right while they share one. Each of you decides what the other may do to your own terminal: watch, or watch and type.

There is no server of Reach's in the path. The two machines connect to each other directly, and everything that travels between them is encrypted with a key only the two of you hold.

## Turning it on

Sharing is **off by default**. Go to **Settings > Sharing** and turn on **Session sharing**.

Off means more than hidden: while it is off, none of the sharing code is loaded at all. Turning it on loads nothing either. Nothing connects to anything until you click **Share**.

## Starting a share

1. Open the terminal you want to share.
2. Click **Share** in the bottom bar.
3. Choose **Start a share**.
4. Pick which terminal to share, or **None** to only watch theirs.
5. Choose what they may do: **Read only** or **Read and write**. Read only is the default.
6. Click **Start a share**. Reach takes a few seconds to find a route, then shows a code beginning `REACH-`.
7. **Copy** the code and send it to the other person however you normally talk to them.
8. When they send a code back, paste it under **Paste their reply** and click **Connect**.

## Joining a share

1. Click **Share** in the bottom bar.
2. Choose **Join a share**.
3. Paste the code you were sent.
4. Optionally pick one of your own terminals to share back, and what they may do to it.
5. Click **Join a share**. Reach shows a reply code.
6. **Copy** the reply and send it back. The connection completes on its own once the other side pastes it.

Both people can share a terminal in the same session. Both can also choose to share nothing and only watch.

## Read only and read and write

The setting is about **your** terminal, and it is enforced on **your** machine. If you set read only, keystrokes from the other side never reach your shell, whatever their client claims. You can switch at any time from the share dialog; the other side is told immediately.

The chip in the bottom bar shows the current state: `RO` or `RW` next to the other person's name.

Read and write means the other person types as you. Treat it accordingly.

## How it connects

The two machines find each other with **STUN**, a small public service that answers one question: "what is my public address?" With both addresses known, the two Reach instances open a direct connection through your routers, a technique called hole punching. No port forwarding is needed and nothing listens permanently.

This works for most pairs of machines. It does not work for roughly 15–30% of them, usually because one side is behind a **symmetric NAT** — common on corporate networks and mobile carriers. When that happens Reach says so and tells you what fixes it, rather than quietly routing your terminal through someone else's server:

- Put both machines on the same network or VPN (Tailscale, WireGuard, and similar all work).
- Add a **TURN** server, if you or your team run one. See below.

## Connection servers

**Settings > Sharing** lists the servers used to find a route. They are yours to change.

- **STUN** servers answer the address question. Reach ships two public ones, run by Google and Cloudflare. They carry none of your traffic and cost nothing to run.
- **TURN** servers relay traffic when no direct route exists. They carry your traffic, so they cost real bandwidth, which is why there is no trustworthy free public one and Reach bundles none. If you run one — coturn on a VPS, Cloudflare Calls, or whatever your team uses — click **Add TURN** and enter its URL, username and credential. The credential is stored in your vault with your other secrets.
- **Remove** any entry. Clearing the list entirely is allowed: sharing then works only between machines on the same network, and contacts nothing outside it.
- **Reset to defaults** restores the two STUN entries.

Servers are contacted only when you click Share, and only to find a route.

## Security

The `REACH-` code contains a random 128-bit token. Both sides derive an encryption key from it, and every message between them is sealed with **AES-256-GCM** under that key — on top of the transport encryption WebRTC already provides.

The reply code is sealed with the same key, so a reply the host accepts is proof the other person actually had the token. Nobody who intercepted the code in transit can substitute themselves without it.

A message that fails to verify ends the connection. There is no reason to keep a channel that has already carried something forged.

Two consequences:

- **Send the code over a channel you trust.** It is the key. Anyone who reads it before the other person pastes it could join instead of them.
- **A code is for one share.** Once connected, or once the share ends, it is useless.

## Ending a share

Click the chip in the bottom bar, then **End share**. The other side is told and their pane closes. Closing Reach ends the share too.

## Limits

- One share at a time.
- The other person needs Reach. There is no browser client.
- The remote pane is sized to the other terminal, not to your window. If theirs is wider, the pane scrolls rather than reflowing — a mirror with a different column count would render every wrapped line wrong.
