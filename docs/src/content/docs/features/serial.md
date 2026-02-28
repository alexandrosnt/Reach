---
title: Serial Console
description: Connect to devices over COM/TTY serial ports.
---

:::note
Serial console is a desktop-only feature. It's not available on Android.
:::

Reach can talk to serial devices — routers, switches, embedded boards, anything with a serial port.

## Connecting

The backend provides three operations: list available ports, open a port, and close it.

When you open a serial connection, you specify:

- **Port name** — the system port identifier (e.g., `COM3` on Windows, `/dev/ttyUSB0` on Linux)
- **Baud rate** — the speed (e.g., `9600`, `115200`)

Data shows up in the same terminal interface used for SSH, so it feels familiar. Keystrokes are sent to the device in real time.

## Port Detection

Reach scans your system for available serial ports and shows them with their type:

- **USB** — USB-to-serial adapters. Shows the product name if available (e.g., "USB - FTDI FT232R").
- **PCI** — Built-in serial ports on the motherboard.
- **Bluetooth** — Bluetooth serial profiles.
- **Unknown** — Ports that don't report their type.

The port list refreshes when you open the serial section.

## How It Works

When you open a port, Reach:

1. Opens the serial port at the specified baud rate with a 100ms timeout
2. Spawns a dedicated reader thread that continuously reads data (1024-byte buffer)
3. Incoming data is emitted as events (`serial-data-{port_name}`) and displayed in the terminal
4. Outgoing data (your keystrokes) goes through an async write channel and gets flushed immediately

The reader thread runs on a real OS thread (not an async task) so it can do blocking reads without holding up anything else. When you close the port, a shutdown signal stops both the reader and writer, with a 3-second timeout for cleanup.

## Supported Platforms

| Platform | Port format | Example |
|----------|-----------|---------|
| **Windows** | COM ports | `COM1`, `COM3` |
| **macOS** | TTY devices | `/dev/tty.usbserial-110` |
| **Linux** | TTY devices | `/dev/ttyUSB0`, `/dev/ttyACM0` |

## Common Uses

- Configuring network equipment (Cisco, Juniper, MikroTik, etc.)
- Debugging embedded systems and microcontrollers
- Accessing device consoles that only have serial output
- Setting up headless servers or single-board computers (Raspberry Pi, etc.)
- Talking to Arduino and other development boards
