---
title: Recipes
description: Reusable bash scripts with parameters and a risk check, run against the terminal you have open.
---

A recipe is a bash script that Reach can run against the session you are looking at. Unlike a snippet, which is one command you paste, a recipe is a small program: it has a name and a version, takes parameters, declares how risky it is, and is meant to be run more than once on more than one machine.

Recipes live in the **Recipes** panel in the sidebar. **Installed** lists what is on this machine; **Browse** lists what the registry offers.

## Running a recipe

1. Open the terminal you want to run it on. Recipes only run against an open session.
2. In the **Recipes** panel, click **Run** on a recipe.
3. The dialog names the target session, shows the risk analysis, and asks for any parameters.
4. **Show the exact command** displays what will be written to the terminal, verbatim.
5. For anything above *mutating*, type the recipe's id to confirm. A button you can hit by reflex is not a confirmation.
6. Click **Run**. The recipe runs in your terminal, where you can watch it.

## The recipe file

A recipe is an ordinary bash script with its metadata in leading `#` comments. It runs with `bash recipe.sh` outside Reach and shellcheck reads it — a format only Reach could execute would be a format nobody could audit, and a recipe is arbitrary code aimed at a server.

```bash
#!/usr/bin/env bash
# @reach-recipe
# id: harden-ssh
# name: Harden the SSH daemon
# description: Disables root login and password auth, then reloads sshd.
# version: 1.0.0
# author: you
# tags: security, ssh
# targets: debian, ubuntu
# danger: sensitive
# param: SSH_PORT | Port sshd should listen on | 22
# param: ADMIN_USER | Account that keeps shell access |
# @end

set -euo pipefail
echo "Hardening sshd on port ${SSH_PORT} for ${ADMIN_USER}"
```

| Field | Meaning |
| --- | --- |
| `id` | Lowercase letters, digits and hyphens. Becomes the filename. |
| `name`, `description` | What people see in the list. |
| `version` | Bump it when the script changes. |
| `tags` | Search terms, comma separated. |
| `targets` | Which systems you wrote it for. Advisory — nothing enforces it. |
| `danger` | `benign`, `mutating`, `sensitive` or `destructive`. See below. |
| `param` | `NAME \| label \| default`. Arrives as an environment variable. A parameter with no default is required. |

Everything after `# @end` is the script.

## How a recipe runs

Reach sends the whole script to the session as one heredoc:

```bash
SSH_PORT='22' ADMIN_USER='alex' bash -s <<'REACH_RECIPE_…'
…
REACH_RECIPE_…
```

Two consequences worth knowing:

- **`set -euo pipefail` works, and so does the exit status.** A failing step stops the recipe rather than carrying on into a half-configured machine.
- **A recipe cannot prompt.** `bash -s` owns standard input. Anything you would have asked the user, take as a `param` instead.

Parameter values are single-quoted before they reach the shell. A value cannot become a command.

## Risk analysis

Before running, Reach reads the script and classifies every line using the same rules the [MCP server](/Reach/features/mcp-server/) applies to an AI's commands.

| Level | Means |
| --- | --- |
| Benign | Reads only. No side effects found. |
| Mutating | Changes state, recoverable. |
| Sensitive | Changes system configuration or service availability. Installing a package or restarting a service is sensitive. |
| Destructive | Data loss, or a machine that will not boot. |

The dialog shows the worst finding, quotes the lines responsible, and calls out two things separately:

- **Lines that download and run code** — `curl … | sh` and the like. Nothing can see what will arrive, so the pattern itself is the warning.
- **An author who understated the risk.** If the header says `benign` and the analysis says `destructive`, you are told. Usually it is a stale header. Sometimes it is not.

Be clear about what this is: pattern matching over shell text. It makes an obvious hazard impossible to miss. It cannot make a subtle one impossible to write. What actually decides whether a registry recipe is safe is the person who reviewed it.

## Writing your own

Click **+** in the Recipes panel. The template that opens explains every field in place, so the first recipe is an edit rather than a lookup. The editor parses as you type and shows the same errors the save will, and the status bar shows the parsed id, name, parameter count and risk level.

Recipes are stored as plain `.sh` files in Reach's data directory. Edit them there if you prefer; Reach reads them back.

## The registry

**Browse** fetches the community registry. Each entry carries a SHA-256 of the script, and Reach refuses to install one whose bytes do not match — what you install is byte for byte what a maintainer reviewed.

To contribute a recipe, open a pull request against [reach-recipes-registry](https://github.com/alexandrosnt/reach-recipes-registry). The merge is the trust boundary; the hash pin is what makes it hold. The repository's `CONTRIBUTING.md` covers the format, the validator that CI runs, and the house rules — check before you break, back up what you edit, validate before applying, `reload` rather than `restart`, and say on the last line what to do if it went wrong.

Installed recipes are marked **From registry** and keep working if the registry is later unreachable.
