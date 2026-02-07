---
title: Serial Console
description: Connect to devices over COM/TTY serial ports.
---

:::note
Serial console is a desktop-only feature. It's not available on Android.
:::

Reach can talk to serial devices like routers, switches, and embedded hardware directly from the app.

## Getting started

1. Go to the serial section
2. Pick a port from the list of detected serial ports
3. Set the baud rate
4. Connect

That's it. Data shows up in the same terminal interface used for SSH sessions, so it feels familiar.

## Supported platforms

- **Windows** - COM ports (COM1, COM2, etc.)
- **macOS** - TTY devices
- **Linux** - TTY devices (`/dev/ttyUSB0`, `/dev/ttyACM0`, etc.)

## Common uses

- Configuring network equipment (Cisco, Juniper, MikroTik, etc.)
- Debugging embedded systems
- Accessing device consoles
- Setting up headless servers or SBCs that only have serial output
