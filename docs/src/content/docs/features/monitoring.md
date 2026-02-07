---
title: System Monitoring
description: Live CPU, RAM, and disk stats from connected servers.
---

When you're connected to a server via SSH, Reach shows a monitoring bar at the bottom of the terminal. It gives you a quick glance at what's going on without needing to run `htop` or `top` yourself.

## What it shows

The monitoring bar displays four things:

- **CPU usage** (%)
- **RAM usage** (%)
- **Disk usage** (%)
- **Logged-in users**

If your username shows up in the logged-in users list, it'll have "(you)" next to it so you can spot yourself quickly.

## How it works

Stats are polled every 3 seconds by running commands over the existing SSH connection. No agent or extra software needed on the remote server.

Here's what it reads under the hood:

- **CPU**: Two snapshots of `/proc/stat` taken 500ms apart. The difference between them gives the actual CPU usage percentage.
- **RAM**: Reads `/proc/meminfo` to get total and available memory.
- **Disk**: Runs `df` to get filesystem usage.
- **Users**: Runs `w`/`who` to see who's logged in.

Since it's all done through your existing SSH session, there's nothing to install or configure on the server side.

## Per-connection stats

Each SSH connection has its own monitoring data. When you switch between tabs, the monitoring bar updates to show the stats for whichever server you're looking at.
