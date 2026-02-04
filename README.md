# Reach

Cross-platform SSH & remote management tool. A modern, open-source MobaXterm alternative.

## Features

- **SSH Terminal** — Connect to remote servers with interactive shell
- **SFTP Explorer** — Browse and transfer files with drag-and-drop
- **Session Manager** — Save, organize, and encrypt connection credentials
- **System Monitoring** — Real-time CPU, RAM, and disk usage from connected hosts
- **Playbooks** — Automate multi-step deployment and maintenance tasks with YAML
- **Port Tunneling** — Local, remote, and dynamic (SOCKS) port forwarding
- **Serial Console** — Connect to COM/TTY serial devices
- **Multi-Execution** — Broadcast commands to multiple terminals simultaneously

## Stack

| Layer | Technology |
|-------|-----------|
| Framework | Tauri v2 (Rust) |
| Frontend | Svelte 5 + SvelteKit |
| Styling | Tailwind CSS v4 |
| Terminal | xterm.js (WebGL) |
| SSH | russh (pure Rust, async) |
| Encryption | AES-256-GCM + Argon2id |

## Development

```bash
# Install dependencies
npm install

# Run in development mode
npm run tauri dev

# Build for production
npm run tauri build
```

## License

MIT
