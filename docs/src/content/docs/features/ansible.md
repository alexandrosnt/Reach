---
title: Ansible
description: Run Ansible playbooks, manage inventories, and automate infrastructure directly from Reach.
---

Reach has a full Ansible workspace built in. You can manage projects, run playbooks, edit inventories, install roles and collections, run ad-hoc commands, and handle vault-encrypted files — all from one place. On Windows, everything routes through WSL automatically.

## Windows and WSL

Ansible doesn't run natively on Windows. Reach detects this and handles it for you.

When you open the Ansible tab on Windows, Reach checks two things in order:

1. **Is WSL installed?** — It runs `wsl.exe --list --quiet` to see if any Linux distribution is available.
2. **Is Ansible installed inside WSL?** — It runs `bash -lc "which ansible"` inside WSL to find the binary.

If both checks pass, you see two green checkmarks and you're good to go. All Ansible commands will be routed through WSL transparently — you don't need to think about it.

If WSL is installed but Ansible isn't, you'll see a red X on the second check with an **Install** button. Click it and Reach installs Ansible via `pip3` inside your WSL distribution.

If WSL isn't installed at all, you get a warning banner telling you to install WSL or use SSH execution instead.

On Linux and macOS, Reach checks for Ansible in your PATH directly. If it's not found, the same one-click install works (via pip or pipx).

### How WSL routing works

When you run any Ansible command on Windows, Reach converts the project path from Windows format (`C:\Users\you\project`) to WSL format (`/mnt/c/Users/you/project`), then executes the command inside WSL:

```
wsl.exe -- bash -c "cd /mnt/c/Users/you/project && ansible-playbook site.yml"
```

Color output is disabled (`ANSIBLE_FORCE_COLOR=0`) so the terminal output stays clean.

## Installing Ansible

If Ansible isn't detected, the setup screen shows an **Install** button. The install process:

1. Reach looks for `pipx`, `pip3`, or `pip` (in that order)
2. Runs the install command (e.g., `pipx install ansible` or `pip3 install --user ansible`)
3. Streams the installation output in real-time so you can see what's happening
4. When it finishes, a **Continue** button appears
5. Click Continue and Reach re-checks the tool status

On Windows, the install happens inside WSL. On Linux/macOS, it runs directly.

## Projects

Everything in the Ansible workspace is organized around projects. A project is a directory on disk that contains your playbooks, inventory files, roles, and other Ansible content. Project metadata (name, description, inventory configuration) is stored encrypted in the Reach vault.

### Creating a project

Click **New Project** on the project list screen. You'll get a modal with three fields:

- **Project Name** — whatever you want to call it
- **Project Path** — the directory where your Ansible files live. Click **Browse** to open a folder picker, or type the path manually.
- **Description** — optional, for your own reference

When you click **Create**, Reach creates the directory (if it doesn't exist), scaffolds a starter `site.yml` playbook and `inventory.ini` file, and saves the project metadata to the vault.

### Project list

The project list shows cards for each project with:

- Project name
- Truncated path
- Description (if set, max 2 lines)
- Last opened timestamp

Click a card to open the project workspace. To delete a project, click the trash icon — you'll get a confirmation dialog first. Deleting a project removes the metadata from the vault but doesn't touch the files on disk.

## Workspace

When you open a project, you get a two-panel layout.

### Left panel — file browser

Shows all files in the project directory. Supported file types: `.yml`, `.yaml`, `.ini`, `.cfg`, `.j2`, `.json`, `.txt`, `.md`. Click a file to view its contents in the right panel.

The panel can be collapsed with the chevron button to give more room to the main content. A **Back to Projects** button at the bottom takes you back to the project list.

### Right panel — workspace tabs

Six tabs across the top:

| Tab | What it does |
|-----|-------------|
| **Playbooks** | Select and run playbooks |
| **Inventory** | Visual host and group editor |
| **Roles** | Install and remove Ansible Galaxy roles |
| **Collections** | Install Ansible Galaxy collections |
| **Ad-Hoc** | Run one-off Ansible commands |
| **Vault** | Encrypt, decrypt, and view vault files |

### Execution target

Below the tabs, there's an **Execution Target** selector. This controls where commands actually run:

- **Local** — runs on your machine (through WSL on Windows)
- **SSH** — runs on a remote server through an active SSH connection

If you pick SSH, a dropdown appears listing all your active SSH connections in `user@host:port` format.

## Playbooks

The Playbooks tab has three inputs and two action buttons.

**Inputs:**
- **Select Playbook** — dropdown listing all `.yml` and `.yaml` files in the project. Required.
- **Inventory File** — dropdown listing all `.ini`, `.cfg` files and any file named `hosts`. Optional.
- **Extra Arguments** — free text input for additional flags like `-v`, `--tags deploy`, `--limit webservers`, etc.

**Buttons:**
- **Run Playbook** — executes `ansible-playbook <playbook> [-i inventory] [extra args]`
- **Check Syntax** — runs `ansible-playbook --syntax-check` to validate the playbook without executing it

Both buttons are disabled while a command is already running. If no playbook files exist in the project, you'll see "No playbooks found in project."

Output streams in real-time on the right side of the panel.

## Inventory

The Inventory tab is a visual editor for managing hosts and groups. Instead of hand-editing INI files, you build the inventory through the UI and Reach generates the file for you.

### Hosts

Click **Add Host** to create a new host entry. Each host has:

- **Name** — logical name (e.g., `web1`)
- **Address** — IP or hostname (e.g., `192.168.1.10`)
- **Port** — SSH port (e.g., `22`)
- **User** — SSH username (e.g., `admin`)
- **Groups** — comma-separated group names this host belongs to

Click a host card to edit it. Changes are local until you save.

### Groups

Click **Add Group** to create a group. Each group has:

- **Name** — group name (e.g., `webservers`)
- **Children** — comma-separated child group names for group nesting

Click a group card to edit it.

### Saving and generating

Two buttons at the bottom:

- **Save** — persists the host and group configuration to the vault. This saves the structured data, not a file.
- **Generate INI** — builds the inventory in standard INI format and shows a preview. The preview includes `[group]` sections, `[group:vars]`, and `[group:children]` blocks.

When the INI preview is showing, a **Write Inventory** button appears. Click it to write the generated INI to `inventory.ini` in your project directory. The file list refreshes automatically.

## Roles

The Roles tab manages Ansible Galaxy roles.

**Install row:**
- Text input for the role name (e.g., `geerlingguy.apache` or a Git URL)
- **Install Role** button — runs `ansible-galaxy role install <name>` in the project directory

**Installed roles list:**
- Each role shows its name and version (if detected)
- **Remove** button on each role — runs `ansible-galaxy role remove <name>`

The list refreshes automatically after install or remove operations.

## Collections

The Collections tab manages Ansible Galaxy collections.

**Install row:**
- Text input for the collection name (e.g., `community.general`)
- **Install Collection** button — runs `ansible-galaxy collection install <name>`

**Installed collections list:**
- Each collection shows its `namespace.name` format and version
- Collections are detected by scanning the `collections/ansible_collections/` directory structure

## Ad-Hoc

The Ad-Hoc tab runs one-off Ansible commands without a playbook.

**Inputs:**
- **Host Pattern** — who to target (e.g., `all`, `webservers`, `192.168.1.*`). Defaults to `all`.
- **Module** — dropdown with common modules: `ping`, `shell`, `command`, `copy`, `yum`, `apt`, `service`, `file`, `setup`, `debug`
- **Module Arguments** — arguments for the selected module (e.g., `name=httpd state=present` for yum)
- **Inventory File** — optional inventory file selector (same as Playbooks tab)

**Button:**
- **Run Ad-Hoc Command** — executes `ansible <pattern> -m <module> [-a "args"] [-i inventory]`

Example: selecting host pattern `all`, module `ping`, no args, runs `ansible all -m ping` — a quick way to check if all hosts are reachable.

## Vault

The Vault tab handles Ansible Vault encryption operations on files in your project.

**Input:**
- **Vault File** — dropdown listing all files in the project

**Buttons** (appear after selecting a file):
- **Encrypt** — runs `ansible-vault encrypt <file>` to encrypt a plaintext file
- **Decrypt** — runs `ansible-vault decrypt <file>` to decrypt an encrypted file
- **View** — runs `ansible-vault view <file>` to display the decrypted contents without modifying the file on disk

Output from these operations streams to the command output panel on the right.

## Command output

All tabs that execute commands (Playbooks, Roles, Ad-Hoc, Vault) share a streaming output panel. It shows:

- **stdout** lines in the default text color
- **stderr** lines in red
- **System messages** (command start, finish, exit code) in the accent color

The output auto-scrolls as new lines arrive. A **Clear** button in the top-right corner empties the output. While a command is running, a spinner and "Command running..." indicator appear.

Under the hood, commands are executed asynchronously. The backend spawns a process, assigns it a unique run ID, and emits events (`ansible-output-{runId}`) as lines come in. The frontend subscribes to these events and appends them to the output in real-time.

## Data storage

Project metadata, inventory configuration, and vault passwords are stored encrypted in the Reach vault using the same XChaCha20-Poly1305 encryption used for SSH sessions and credentials. The actual playbook files, inventory files, and roles live on disk in whatever directory you pointed the project to.

Projects persist across app restarts. When you reopen Reach, your project list is loaded from the vault and ready to go.
