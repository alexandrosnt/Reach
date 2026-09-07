---
title: Development Setup
description: How to build Reach from source.
---

## Prerequisites

You'll need the following installed before you start:

- **Rust** - Install via [rustup.rs](https://rustup.rs)
- **Node.js 22** or newer
- **Tauri v2 prerequisites** for your OS - See the [official guide](https://v2.tauri.app/start/prerequisites/)

On **Linux**, you also need these system packages:

```bash
sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf libudev-dev
```

## Clone and run

```bash
git clone https://github.com/alexandrosnt/Reach.git
cd Reach
npm install
npm run tauri dev
```

This starts the Vite dev server on port 1420 and launches the Tauri window. Hot module replacement works for the frontend. Rust changes trigger an automatic rebuild (Tauri handles this for you).

## Other useful commands

```bash
npm run dev              # Vite dev server only (no Tauri window)
npm run build            # Build frontend only
npm run tauri build      # Full production build (outputs installers)
npm run check            # TypeScript and Svelte type checking
npm run i18n:check       # Validate all locale files match en.json
cd src-tauri && cargo check  # Check Rust code without building
```

## Production builds

Running `npm run tauri build` outputs platform-specific installers in `src-tauri/target/release/bundle/`. On Windows you get an MSI and NSIS installer, on macOS a .dmg, and on Linux a .deb and .AppImage.
