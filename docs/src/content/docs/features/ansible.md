---
title: Ansible
description: Run Ansible playbooks, manage inventories, and automate infrastructure directly from Reach.
---

Reach includes a full Ansible workspace for managing playbooks, inventories, roles, collections, and vault-encrypted files. On Windows, everything runs automatically through WSL.

## Getting Started

Open the **Ansible** tab from the sidebar. If Ansible isn't installed yet, Reach will detect this and offer a one-click install.

### Windows (WSL)

Ansible doesn't run natively on Windows. Reach automatically detects WSL and routes all Ansible commands through it. The setup screen shows two checks:

1. **WSL available** — Windows Subsystem for Linux is installed and has at least one distribution
2. **Ansible installed** — Ansible is found inside the WSL environment

If Ansible isn't installed in WSL, click **Install** and Reach will install it via pip3 inside your default WSL distribution.

### Linux / macOS

Reach detects Ansible from your system PATH. If it's not found, click **Install** to install it via pip or pipx.

## Projects

Ansible work is organized into projects. Each project maps to a directory on disk and stores its configuration (name, description, inventory) encrypted in the vault.

### Creating a Project

Click **New Project**, give it a name, pick a directory, and optionally add a description. Reach scaffolds a starter `site.yml` playbook and `inventory.ini` file in that directory.

### Opening a Project

Click on any project card to open its workspace. The last-opened timestamp is tracked so your most recent work is easy to find.

## Workspace

The workspace has a file panel on the left and tabbed panels on the right.

### File Panel

Lists all files in the project directory. Click a file to open it in the built-in editor. The panel can be collapsed to give more room to the main content.

### Execution Target

At the top of the workspace, you can choose where commands run:

- **Local** — Runs on your machine (or through WSL on Windows)
- **SSH** — Runs on a connected remote host via SSH

### Playbooks

Select a playbook file (`.yml` / `.yaml`) and an inventory file, then click **Run**. You can also run a **Syntax Check** to validate the playbook without executing it. An extra args field is available for passing additional flags like `--tags`, `--limit`, etc.

### Inventory

A visual editor for managing hosts and groups:

- **Hosts** — Add hosts with name, address, port, user, group assignments, and custom variables
- **Groups** — Create groups with variables and child groups
- **Generate** — Preview the INI-format inventory and write it to a file in your project

### Roles

Lists installed Ansible roles. Install new roles by name (from Ansible Galaxy or a URL) and remove roles you no longer need.

### Collections

Lists installed Ansible collections. Install new collections by name from Ansible Galaxy.

### Ad Hoc

Run one-off Ansible commands without a playbook:

- Pick a **host pattern** (e.g. `all`, `webservers`, a specific host)
- Select a **module** (ping, shell, command, copy, yum, apt, service, file)
- Enter **module arguments** (e.g. `uptime` for the shell module)
- Choose an **inventory file**
- Click **Run**

### Vault

Encrypt, decrypt, and view vault-encrypted files:

- **Encrypt** — Encrypt a plaintext file with a vault password
- **Decrypt** — Decrypt an encrypted file
- **View** — View the contents of an encrypted file without decrypting it on disk

## Command Output

All command output streams in real-time. Lines are color-coded:

- Standard output in the default text color
- Errors (stderr) highlighted in red
- System messages (start/finish/exit code) in a muted color

The output panel auto-scrolls as new lines arrive. When a command finishes, the exit code is displayed.

## Data Storage

Project metadata (name, path, inventory configuration) is stored encrypted in the Reach vault alongside your sessions and credentials. The actual playbook and inventory files live on disk in the project directory you chose.
