# Contributing a recipe

A recipe is a bash script that Reach can run against a server the user already
has open. Contributing one means asking people to run your code as root on
their infrastructure, so this document is mostly about earning that.

## The short version

1. Fork, then add `recipes/<your-id>.sh`.
2. `node build-index.mjs --write`
3. `node validate.mjs` — CI runs this, so save yourself a round trip.
4. Open a pull request describing **what it changes on the machine** and
   **how you tested it**, naming the distro and version.

## The file

```bash
#!/usr/bin/env bash
# @reach-recipe
# id: harden-ssh              <- must equal the filename, minus .sh
# name: Harden the SSH daemon
# description: One line. This is what people read in the list.
# version: 1.0.0              <- semver; bump it when you change the script
# author: your-handle
# tags: security, ssh
# targets: debian, ubuntu     <- advisory; nothing enforces it
# danger: sensitive
# param: SSH_PORT | Port sshd should listen on | 22
# param: ADMIN_USER | Account that keeps access |
# @end

set -euo pipefail
```

Everything after `@end` is the script. Parameters arrive as environment
variables, single-quoted, so a value can never become a command. A parameter
with no default is required before the recipe can run.

The file stays an ordinary bash script — `bash recipes/x.sh` runs it and
shellcheck reads it. That is deliberate: a format only Reach can execute is a
format nobody can review.

## How it runs, and what that forces

Reach sends the whole recipe as one heredoc:

```bash
SSH_PORT='22' bash -s <<'REACH_RECIPE_…'
…
REACH_RECIPE_…
```

Two consequences to design around:

- **`set -euo pipefail` works, and so does the exit status.** A failing step
  stops the recipe instead of continuing into a half-configured machine.
  `validate.mjs` requires this line.
- **A recipe cannot prompt.** `bash -s` owns stdin. Anything you would have
  asked, take as a `param`.

## Declaring danger honestly

| Level | Means |
| ----- | ----- |
| `benign` | Reads only. No side effects. |
| `mutating` | Changes state, recoverable. |
| `sensitive` | Changes system configuration or service availability. |
| `destructive` | Data loss, or a machine that will not boot. |

Reach analyses your script independently and **tells the user when the two
disagree**:

> The author declared this Benign, but it looks Destructive.

Installing a package or enabling a service is `sensitive`, not `mutating`. That
rule caught `fail2ban-baseline` in this very directory during review, so it is
not a hypothetical.

Above `mutating`, Reach makes the user type the recipe's id to confirm. Being
honest costs you one click and is what keeps the warning meaningful for
everyone else.

## House rules

Every one of these comes from a way a recipe can ruin someone's afternoon.

- **Check before you break.** `harden-ssh` verifies the admin account exists
  *and* has an authorized key before disabling password auth. Locking someone
  out of the machine they are hardening is the failure that matters most.
- **Back up what you edit**, to a timestamped path, and restore it if
  validation fails.
- **Validate before applying.** `sshd -t` before reload, `nginx -t` before
  reload, every time.
- **`reload`, not `restart`**, where the daemon supports it. The session
  running your recipe is one of the connections a restart drops.
- **Write to the override file, not the packaged one.** `jail.local`, not
  `jail.conf`; a drop-in under `conf.d`, not the file the package owns.
  Anything the package owns is overwritten on upgrade, silently undoing you.
- **Be idempotent.** Someone will run it twice. The second run should be
  boring.
- **Say what to do if it went wrong**, on the last line, while the operator is
  still looking at the terminal.
- **Quote your expansions.** `"${VAR}"`, always. A path with a space in it
  should not be a security incident.

## What validation checks

`validate.mjs` covers the mechanical part:

- the header parses, and `id` matches the filename
- `description`, `version` (semver), `tags` and `danger` are present and sane
- parameter names are valid shell identifiers — they are interpolated into a
  shell assignment, so a lax one is command injection
- every declared parameter is used, and every uppercase variable read is either
  a parameter, assigned by the script, or ambient
- `set -euo pipefail` is present, and there is a shebang
- `recipes.json` is current, and every `sha256` matches the file on disk

It does not read your script for intent. That is what review is for.

## What review is for

The merge is the trust boundary. The SHA-256 pin in `recipes.json` guarantees
that what a user installs is byte-for-byte what a maintainer read — it says
nothing about whether reading it was enough. So a pull request should make
review easy:

- say what the recipe changes, in plain terms
- say which distro and version you ran it on, and what you saw
- keep it to one job. A recipe that installs a database *and* tunes the kernel
  *and* opens a firewall port is three recipes and one review nobody can do
  carefully.
- avoid `curl … | sh`. Reach flags it separately because nothing — not the
  analyser, not the reviewer — can see what will arrive. If a vendor only ships
  an install script, fetch it, pin its hash, and run it in a second step.

## Updating a recipe you already contributed

Bump `version`, edit the script, re-run `build-index.mjs --write`, and open a
PR. The hash changes, which is the point: a client that already installed the
old one keeps running it until the user chooses to update.
