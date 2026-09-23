---
title: Host Key Verification
description: How Reach decides a server is the server you meant, and what it does when the key changes.
---

Every SSH server proves who it is with a host key. Checking that key is what
stops someone sitting between you and the machine you think you are talking to.
Reach checks it on every connection, and never accepts one silently.

## Trust on first use

The first time you connect to a machine, Reach has nothing to compare against.
It shows you the fingerprint and the key type and asks.

Accept, and the fingerprint is recorded. Every later connection to that host
and port is compared against it, and if they match you connect without being
asked again.

This is the same model OpenSSH uses, and it has the same honest limitation: the
first connection is the one you cannot verify from inside the tool. If the
machine matters, compare the fingerprint against one you obtained another way —
your provider's console, the output of `ssh-keyscan` from a machine you
already trust, or whoever set the server up.

## When a key changes

If the stored fingerprint and the presented one differ, Reach tells you so
explicitly, shows you **both** fingerprints, and asks before doing anything.

A changed key has two explanations. The ordinary one is that the server was
rebuilt, reinstalled, or migrated, and it genuinely has a new key. The other is
that something is sitting between you and it.

Reach cannot tell those apart, and does not pretend to. It shows you what
changed and lets you decide — but it will not connect until you have.

## Nothing is accepted silently

Three situations, three behaviours:

| Situation | What happens |
| --- | --- |
| Known host, matching key | Connects, no prompt |
| Unknown host | Prompt showing the fingerprint |
| Known host, **different** key | Prompt showing both, marked as changed |

The prompt fails closed. If the interface cannot be reached, if the prompt
cannot be displayed, or if it goes unanswered for two minutes, the connection
is **refused** rather than allowed. A verification step that gives up and
connects anyway is not a verification step.

Declining a prompt aborts the connection and records nothing, so declining by
accident costs you one reconnect.

## Where the fingerprints are stored

```
<app data>/ssh/known_hosts.json
```

Keyed by host and port, so the same machine on two ports is two entries — which
is correct, since they may genuinely be different servers.

It is a plain JSON file you can read, and deleting an entry makes Reach treat
that host as new again. That is the way to recover from accepting something you
should not have: remove the entry, reconnect, and verify the fingerprint
properly this time.

The path resolves inside the application's own data directory, which is why
this works identically on Android, where an app cannot write wherever it likes.

## Related

- [Jump Hosts](../jump-hosts/) — each hop in a chain is verified in its own
  right
- [Sessions](../sessions/) — where connections are saved
