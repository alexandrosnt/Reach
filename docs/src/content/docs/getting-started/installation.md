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

On first launch, macOS will probably warn you that the app is from an unidentified developer. Right-click the app, select Open, and confirm. You only need to do this once.

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

After installation, the first time you open Reach you'll see a quick setup wizard. Head over to [First Run & Setup](/Reach/getting-started/first-run/) to see what that looks like.
