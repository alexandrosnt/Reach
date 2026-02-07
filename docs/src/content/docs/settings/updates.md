---
title: Auto-Updates
description: How Reach keeps itself up to date.
---

Reach checks for updates automatically. On startup, it hits the GitHub releases page and looks for a newer version. If one exists, you'll see an update dialog.

The dialog shows the new version number and release notes. You can update right away or skip it for now.

After you dismiss the dialog, the app checks again every 45 minutes in the background. If an update is available, a banner shows up at the top of the window.

## How updates work

1. Click "Update Now" in the dialog or the banner.
2. The new version downloads. You'll see a progress bar.
3. Once the download finishes, the app restarts with the new version.

Updates are signed with a private key and verified before installation. You can't accidentally install a tampered build.

## Platform notes

- **Windows, macOS, Linux**: Full auto-update support on all three.
- **Android**: Auto-updates are not available. You'll need to grab the new APK manually from the [releases page](https://github.com/alexandrosnt/Reach/releases).
