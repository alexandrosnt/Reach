---
title: Remote Desktop (RDP)
description: Open a Windows desktop in a tab, with the clipboard and files shared both ways.
---

Reach opens a remote desktop the same way it opens a shell: in a tab, from a saved session or from the **+** menu. The protocol is RDP, spoken by [IronRDP](https://github.com/Devolutions/IronRDP), which is pure Rust from the TLS handshake to the bitmap decoders. That is why it works on Windows, Linux, macOS and Android alike, with no native RDP library involved.

## Connecting

Two ways in:

- **+ → Remote desktop (RDP)** opens a desktop once, without saving anything. Host, port, username, password and an optional logon domain.
- **New session → Protocol: Remote desktop (RDP)** saves it next to your SSH sessions. It gets a Windows icon, the port defaults to `3389`, and only the fields RDP uses are shown: keys, agents, jump hosts, proxies and login shells are SSH ideas and stay out of the way. Leave the password empty to be asked on connect, or save it and it is stored encrypted in the vault like every other credential.

Network Level Authentication is on. The desktop is rendered at the size of the panel, and follows it when you resize the window.

## What is shared

**Clipboard, both directions.** Copy text or files here, click into the desktop, paste there. Copy on the remote and it is on your local clipboard at once; files the remote copied are fetched into a temporary folder and put on your clipboard as file paths, so a paste in your file manager works. The folder is removed when the session ends. Copies over 2 GB are left on the remote rather than downloaded on the chance of a paste.

Android has no file clipboard, so it shares text only.

**The cursor** is drawn by Reach from the shapes the server sends, not by the server into the picture. It moves with your real mouse and changes to an I-beam or a hand where the remote application wants one.

## How the picture is delivered

The server sends what changed, as tiles. Reach keeps its own copy of the framebuffer, applies every tile the moment it arrives, and paints the canvas only with whole frames: it waits for the server to go quiet for a few milliseconds before sending what changed, so a video never shows a mosaic of old and new tiles. Under load the picture updates less often, but it is never behind the server, which is how Microsoft's own client behaves.

Frames are paced by acknowledgement. The webview says when it has painted, and the next frame goes out only then, so a busy screen costs the interface exactly as many frames as it can draw and no queue ever builds up.

## Not yet

- **Sound and drive redirection.** Each pulls a native backend into the build; neither is needed to see a desktop and drive it.
- **Certificate pinning.** The server's certificate is accepted on connect. The same trust-on-first-use prompt the SSH side has is the next thing to add here.
- **H.264 video.** Windows offers it and IronRDP can decode it, but the client we build on does not yet let a decoder be plugged in. Until then video is delivered as tiles, which is fine on a LAN and heavier over a slow link.
- **Telling Windows from xrdp.** RDP carries an OS field, but xrdp fills it in as Windows to keep clients happy, so a saved RDP session is shown as Windows.
