---
title: Themes
description: Change how Reach looks, and install themes other people made.
---

Reach ships with themes and can install more. A theme is data, not code — a
set of colour tokens the whole application reads — so switching one repaints
every panel, not just the terminal.

## Switching

**Settings → General → Theme**. The change applies immediately and is
remembered.

Every theme declares whether it is `dark` or `light`. That is not decorative:
it drives the root class, so the native window chrome, the scrollbars and the
form controls follow the theme rather than staying whatever the operating
system thought.

## Installing more

**Settings → General → Theme → Browse**, which reads the theme registry.

Each entry carries a name, an author, a version, a one-line description and a
declared appearance, so you know whether a theme is light or dark before
installing it.

### How a theme is verified

The registry entry names an HTTPS URL for the theme's JSON **and a SHA-256 hash
of the bytes at that URL**. Reach downloads the file, hashes it, and installs it
only if the two match.

That matters because the registry and the file it points at are separate
things. Without the hash, anyone able to change the file at that URL could
change what gets installed on every machine that installs it afterwards. With
it, changing the file breaks the install instead.

Themes contain no executable code — only colour values — so a malicious theme
cannot do more than look wrong. The hash is about knowing you got the theme the
registry described.

## Using your own registry

The registry URL is a setting, not a constant. Point it at your own JSON file
and the Browse list becomes yours — useful if your team has house themes, or
if you would rather not fetch from a public URL at all.

The default is:

```
https://raw.githubusercontent.com/alexandrosnt/reach-themes-registry/main/themes.json
```

There is a **Reset** beside it that puts the default back.

## Removing one

Installed themes can be uninstalled from the same panel. If the theme you
remove is the one in use, Reach falls back to a built-in one rather than
leaving you with no colours.

## Related

Plugins and recipes use the same registry pattern, with the same
hash-before-install rule. See [Plugins](../plugins/) and
[Recipes](../recipes/).
