---
title: DevOps Tools
description: Switch Ansible, OpenTofu and Databases on or off in Settings → DevOps.
---

Reach is a remote-access app first. The tools for automating and managing infrastructure — **Ansible**, **OpenTofu** and **Databases** — are there when you want them and out of the way when you don't. Each has its own switch under **Settings → DevOps**.

## Where to find it

Open Settings with **Ctrl+,** (**Cmd+,** on Mac) and choose **DevOps**. Each tool has a row with a switch and a line saying whether it is ready on this device — the installed Ansible or OpenTofu version, or *Built into Reach* for Databases, which needs nothing installed.

## What a switch does

- **On:** the tool appears in the tab bar at the top of the window, next to Terminal.
- **Off:** the tab disappears, and Reach refuses the tool's commands entirely rather than just hiding its page. Open database connections and their SSH tunnels are closed.
- **Nothing is deleted.** Projects, saved connections and settings stay where they are and come back when you switch the tool on again.
- **A run in progress keeps its switch.** While an Ansible or OpenTofu run, or a database query, is still going, its switch is greyed out until it finishes.

## Defaults

| | Fresh install | Updating from 0.6.7 or earlier |
|---|---|---|
| Ansible | Off | On, as before |
| OpenTofu | Off | On, as before |
| Databases | Off | Off — new in 0.6.8 |

The switches belong to the device, not your account: they are not synced, because the tools themselves are installed per machine.
