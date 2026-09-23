---
title: Marketplace
description: Install plugins, themes and recipes from a registry, or point Reach at your own.
---

Reach installs three kinds of add-on from registries: plugins, themes and
recipes. They share one design, so learning it once covers all three.

A registry is a JSON file listing what is available. Reach fetches it, shows
you the list, and installs the one you pick. There is no account, no upload
flow and no server belonging to Reach in the middle — a registry is a file at
a URL.

## What a listing tells you

| Field | Why it is there |
| --- | --- |
| **Name** and **description** | What it is |
| **Author** | Who wrote it |
| **Version** | What you are about to install |
| **Source** | The `user/repo` it came from, so you can go and read it |
| **Keywords** | Extra search terms, matched with the name and description |

The search box matches across name, description, author and keywords at once,
which is what makes a registry usable once it stops being short.

## Installing, and staying current

Installing downloads the add-on, verifies it, and enables it.

Reach compares installed versions against the registry and marks anything with
a newer version available. Updating is the same button; nothing updates itself
behind your back.

Uninstalling removes it. A theme in use falls back to a built-in one rather
than leaving the application with no colours.

## Verification

Every registry entry names an HTTPS URL **and a SHA-256 hash of the bytes at
that URL**. Reach downloads, hashes, compares, and refuses the install if they
differ.

This matters because the registry and the files it points at are separate
things that can be changed independently. Without the hash, whoever controls
the file at that URL could change what gets installed afterwards on every
machine. With it, changing the file breaks the install rather than silently
shipping something else.

For recipes the same pin does a second job: it is how a shared script is known
to be the script that was reviewed.

## Pointing at your own registry

Every registry URL is a setting. Replace it and the list becomes yours —
useful for a team with its own plugins, or if you would rather not fetch from
a public URL at all.

```
Plugins  https://raw.githubusercontent.com/alexandrosnt/reach-plugins-registry/main/plugins.json
Themes   https://raw.githubusercontent.com/alexandrosnt/reach-themes-registry/main/themes.json
Recipes  https://raw.githubusercontent.com/alexandrosnt/reach-recipes-registry/main/recipes.json
```

Each has a **Reset** beside it that restores the default. The custom URL is
remembered between restarts.

A registry is a static JSON file, so hosting your own needs nothing more than
somewhere to serve a file over HTTPS.

## How much each can actually do

Worth knowing before installing anything, because the three are not equally
powerful:

- **Themes** are colour values. No code. The worst a bad one does is look
  wrong.
- **[Plugins](../plugins/)** are sandboxed Lua, reaching the host through a
  declared API, with permissions granted from their manifest.
- **[Recipes](../recipes/)** are bash, run on your server. They carry a
  declared risk level, every script is analysed before running, and anything
  above *mutating* needs a typed confirmation.

Read a recipe before you run it. The pin tells you the script has not changed
since it was published; it does not tell you the script is a good idea.
