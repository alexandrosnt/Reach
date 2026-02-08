---
title: Playbooks
description: Automate server tasks with Ansible-compatible YAML playbooks.
---

Playbooks let you automate multi-step tasks on remote servers using Ansible-compatible YAML. The execution engine is built directly into Reach in Rust — no Python, no Ansible installation, no external tools. It works on Windows, macOS, and Linux out of the box.

You write a playbook, select an SSH connection, and hit Run. Each task is translated into shell commands and executed over your existing SSH session.

## Format

Playbooks use the same YAML format as Ansible. A playbook contains one or more plays, each with a list of tasks. Each task uses a module to perform an action.

A task can have:

- **name** — What this task does (shows up in the output)
- **module** — One of the 8 supported modules (see below)
- **when** — A condition that must be true for the task to run
- **register** — Store the output in a variable for later tasks
- **become** — Run this task with sudo
- **ignore_errors** — Keep going even if this task fails

## Supported Modules

| Module | What it does |
|--------|-------------|
| `shell` | Run a shell command (with shell expansion) |
| `command` | Run a command (no shell expansion) |
| `copy` | Write content to a file on the remote server |
| `file` | Create directories, set permissions, delete files |
| `apt` | Install or remove packages (Debian/Ubuntu) |
| `systemd` / `service` | Start, stop, enable, or restart services |
| `lineinfile` | Add or replace a line in a file |
| `template` | Write content to a file with variable substitution |

## Variables

Define variables in the `vars` section of a play and reference them in task arguments with `{{ variable_name }}`. Variables from `register` are also available to later tasks.

## Example

```yaml
- name: Deploy and restart app
  become: true
  vars:
    app_dir: /opt/myapp
    service_name: myapp
  tasks:
    - name: Pull latest code
      shell: cd {{ app_dir }} && git pull origin main

    - name: Install dependencies
      apt:
        name: nginx
        state: present

    - name: Write config file
      copy:
        content: |
          server {
            listen 80;
            location / { proxy_pass http://localhost:3000; }
          }
        dest: /etc/nginx/sites-available/myapp

    - name: Restart the service
      systemd:
        name: "{{ service_name }}"
        state: restarted

    - name: Check service status
      shell: systemctl is-active {{ service_name }}
      register: status_result

    - name: Verify it's running
      shell: echo "Service is {{ status_result.stdout }}"
```

## Running a Playbook

1. Open the Playbook panel from the sidebar
2. Select an active SSH connection from the dropdown
3. Paste or type your YAML playbook (or load a saved project)
4. Optionally enable **Become (sudo)** and add extra variables
5. Click **Run**

Output streams in real time as each task executes. You'll see the task name, stdout/stderr, and whether it passed or failed. If a task fails and `ignore_errors` isn't set, the playbook stops there.

You can also click **Validate** to check your YAML syntax without running anything. It parses the playbook and shows you the list of tasks it found.

## Cancellation

Click **Cancel** while a playbook is running to stop it. The current task finishes and no further tasks are executed.

## Saved Projects

You can save playbook configurations (YAML content, connection, become setting) as named projects. They're stored encrypted in the vault, same as sessions and credentials. Load a saved project from the dropdown to quickly re-run it.

## Differences from Ansible

This is a lightweight engine, not a full Ansible replacement. Key differences:

- **No inventory files** — you select an SSH connection from the app
- **No roles, includes, or handlers** — tasks run sequentially in order
- **8 modules** — shell, command, copy, file, apt, systemd, lineinfile, template
- **Simple `when` conditions** — supports `is defined`, `==`, `!=`, and `.rc` checks
- **No local execution** — everything runs on the remote server over SSH
