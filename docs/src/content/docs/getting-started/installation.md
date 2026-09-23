---
title: Installation
description: How to install Reach on your platform.
---

Grab the latest release from the [GitHub Releases page](https://github.com/alexandrosnt/Reach/releases). Pick the installer for your platform and you're good to go.

## Windows

Download the `.exe` installer (NSIS). Run it, click through the prompts, done. It supports both per-user and system-wide installation. If you install per-user, you don't need admin rights.

The installer registers Reach in your Start Menu and optionally creates a desktop shortcut.

## macOS

Download the `.dmg` file. Open it and drag Reach to your Applications folder. Both Intel and Apple Silicon Macs are supported.

**Important:** macOS will block the app because it's not signed with an Apple Developer certificate. You'll see a message saying the app is "damaged" or can't be opened. To fix this, open Terminal and run:

```bash
sudo xattr -cr /Applications/Reach.app
```

This strips the quarantine flag that macOS adds to downloaded apps. You only need to do this once after installing (or after updating).

### Hosts on your local network

On macOS 15 and later, connecting to a host on your own network — a NAS, a homelab box, anything on a private address like `10.x`, `192.168.x` or `172.16–31.x` — can fail with **No route to host (os error 65)** even though `ping` to the same address works from Terminal. The route is fine. macOS puts every app's access to the local network behind a permission, enforces it with a packet filter, and reports the dropped packets to the app as if the host were unreachable.

Reach tells you this is what happened when it sees that error against a local address. The switch is **System Settings → Privacy & Security → Local Network**: turn on Reach.

The honest catch: macOS only offers that prompt, and only lists the app there, for builds signed with an Apple-issued developer certificate — and the current macOS build is not signed. Until it is, the workarounds are to connect through a jump host on a public address, to use a hostname that resolves through a router rather than a link-local path, or to run Reach from a location macOS has already granted (some people report success launching it from Terminal with `open -a Reach`, since Terminal carries its own grant). The release pipeline signs and notarizes the build automatically once a certificate is configured, and that is the real fix.

## Linux

There are a few options depending on your distro:

- **`.deb`** for Debian, Ubuntu, and derivatives. Install with `sudo dpkg -i reach_*.deb` or just double-click it in your file manager.
- **`.rpm`** for Fedora, RHEL, openSUSE. Install with `sudo rpm -i reach_*.rpm`.
- **`.AppImage`** works on pretty much any distro. Make it executable (`chmod +x Reach_*.AppImage`) and run it.

### Dependencies

The `.deb` and `.rpm` packages handle dependencies automatically, but if you're using the AppImage or building from source, make sure you have:

- `libwebkit2gtk-4.1-0` (or `webkit2gtk4.1` on Fedora)
- `libappindicator3-1` (for the system tray icon)

On Ubuntu/Debian:
```bash
sudo apt install libwebkit2gtk-4.1-0 libappindicator3-1
```

On Fedora:
```bash
sudo dnf install webkit2gtk4.1 libappindicator-gtk3
```

## Android

Download the `.apk` file from the releases page and sideload it. Reach isn't on the Play Store yet.

You'll need to enable "Install from unknown sources" in your device settings if you haven't already.

## Auto-updates

Once Reach is installed, it checks for updates on startup and then every 45 minutes while the app is running. When a new version is available, you'll see a notification with the version number and changelog. You can download and install the update right from that notification, or dismiss it and update later.

Updates are downloaded in the background and applied on restart.

## Next steps

After installation, the first time you open Reach you'll see a quick setup wizard. Head over to [First Run & Setup](../first-run/) to see what that looks like.
