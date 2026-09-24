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

## A shared folder

A saved RDP session can name one local folder, which appears inside the desktop as a drive named after the folder; IronRDP's native filesystem backends serve it on Windows, macOS and Linux. Android has no such backend and says so in the editor.

## Sound

Playback is built in, through IronRDP's audio channel and the default output device, and the channel is announced exactly as Microsoft's client announces it. Against the Windows 11 host it was tested on, the server joins the channel and then never sends a byte on it, and never opens the audio dynamic channel either, so nothing plays yet. The next step is a packet capture of Microsoft's client against the same host to see what it does differently.

## The server's certificate

The certificate is checked strictly first, so one the platform trusts passes without a word. Anything else, which is nearly every RDP server since they self-sign, goes through the same trust-on-first-use dialog and known-hosts file as an SSH host key: accept once, silent thereafter, and a loud stop if it ever changes.

## Video

OpenH264 is compiled in and the client can hand it to the graphics pipeline, which is what lets a Windows host send video as AVC420. The pipeline is off by default, though: the host it was tested on opened the pipeline and then declined AVC420, and the pipeline's remaining codecs cost more over a slow link than the plain bitmap path does. It is a one-line switch in the connection config for a host that does offer H.264.

## Windows or Linux

RDP cannot say what the machine is: xrdp fills in the OS field as Windows to keep clients happy. So the session editor asks, Windows unless told otherwise, and the icon in the session list follows.
