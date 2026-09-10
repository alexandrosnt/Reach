# Reach recipes registry

This directory is a registry repo in a box. Copy it into
`reach-recipes-registry`, push, and Reach clients will find it at the default
URL. Nothing here is imported by the app.

## What a recipe is

A bash script that carries its own metadata in leading `#` comments, so it
stays a valid, runnable, lintable script outside Reach. That is the point: a
recipe is arbitrary code aimed at someone's production server, and being
readable with `cat` matters more than being elegant.

```bash
#!/usr/bin/env bash
# @reach-recipe
# id: harden-ssh              <- must match the filename
# name: Harden the SSH daemon
# description: One line.
# version: 1.0.0
# author: you
# tags: security, ssh
# targets: debian, ubuntu
# danger: sensitive
# param: SSH_PORT | Port sshd should listen on | 22
# param: ADMIN_USER | Account that keeps access |     <- no default = required
# @end

set -euo pipefail
...
```

Everything after `@end` is the script. Parameters arrive as environment
variables, single-quoted, so a value can never become a command.

## How it runs

Reach sends the whole thing as one heredoc:

```bash
SSH_PORT='22' ADMIN_USER='alex' bash -s <<'REACH_RECIPE_…'
…
REACH_RECIPE_…
```

Two consequences worth designing around:

- **`set -euo pipefail` works, and so does the exit status.** A failing step
  stops the recipe instead of carrying on into a half-configured machine.
- **A recipe cannot prompt.** `bash -s` owns stdin. Take parameters instead —
  which is what you want anyway from something meant to be run twice.

## Adding a recipe

1. Write `recipes/<id>.sh`. The `id` in the header must equal the filename.
2. `node build-index.mjs --write` and commit the regenerated `recipes.json`.
3. Open a pull request.

The merge is the trust boundary. Everything else — the SHA-256 pin, the size
cap, the risk analysis in the client — narrows the gap between "a maintainer
read these bytes" and "these bytes ran as root on someone's server. None of it
replaces the review.

## The danger field

State the truth. Reach analyses the script independently and **shows the user
when the two disagree**:

> The author declared this Benign, but it looks Destructive.

| Level | Means |
| ----- | ----- |
| `benign` | No side effects. Reads only. |
| `mutating` | Changes state, recoverable. |
| `sensitive` | Changes system configuration or service availability. |
| `destructive` | Data loss, or a machine that will not boot. |

Installing a package or enabling a service is `sensitive`, not `mutating` —
that caught the fail2ban recipe in this very directory during review.

Anything above `mutating` makes the user type the recipe's id to confirm, so
being honest costs you a click and buys the warning its credibility.

## House rules for the scripts here

Learned from the recipes already in this directory:

- **Check before you break.** `harden-ssh` verifies the admin account exists
  *and* has an authorized key before disabling password auth. Locking someone
  out of the machine they are hardening is the failure that matters.
- **Back up what you edit**, to a timestamped path, and restore it if
  validation fails.
- **Validate before applying.** `sshd -t` before reload, every time.
- **`reload`, not `restart`**, where the daemon supports it — the session
  running the recipe is one of the connections a restart would drop.
- **Write to `jail.local`, not `jail.conf`.** Anything the package owns will be
  overwritten on upgrade, silently undoing the recipe.
- **Say what to do if it went wrong**, on the last line, while the operator is
  still looking at the terminal.

## Testing

`build-index.mjs` validates every header and refuses to emit an index if one is
malformed. Reach's own test suite additionally parses the recipes in this
directory with the Rust parser and asserts that none of them understate their
risk — the JS builder and the Rust parser have separate copies of the grammar,
and this is where drift surfaces.
