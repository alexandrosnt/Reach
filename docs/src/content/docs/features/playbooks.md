---
title: Playbooks
description: Automate server tasks with YAML scripts.
---

Playbooks let you define a sequence of shell commands to run on a remote server. Think of it as a lightweight version of Ansible, built right into Reach. You write some YAML, point it at a server, and hit run.

## Format

Each playbook is a YAML file with a name, optional description, optional variables, and a list of steps.

A step can have:

- **name** - What this step does (shows up in the UI)
- **command** - The shell command to run
- **timeout** - Seconds before the step is killed (optional)
- **expect_exit_code** - What exit code counts as success (optional)
- **expect_output** - A regex pattern the output must match (optional)
- **retries** and **retry_delay** - Retry on failure (optional)
- **on_failure** - Either `stop` (halt the playbook) or `continue` (keep going)

## Variables

Define variables at the top of the playbook and reference them in commands with `{{ variable_name }}`. This keeps your playbooks reusable across different servers and environments.

## Example

```yaml
name: "Deploy App"
description: "Pull code and restart service"
variables:
  APP_HOME: "/opt/myapp"
  SERVICE: "myapp"
steps:
  - name: "Pull latest code"
    command: "cd {{ APP_HOME }} && git pull origin main"
    timeout: 60
    on_failure: stop
  - name: "Restart service"
    command: "systemctl restart {{ SERVICE }}"
    expect_exit_code: 0
  - name: "Check status"
    command: "systemctl status {{ SERVICE }}"
    expect_output: "active (running)"
```

This pulls the latest code, restarts the service, and checks that it came back up. If the git pull fails, the whole thing stops. If the restart or status check fails, you'll see it in the output.

## AI Generation

If you have an AI provider configured in settings, you can describe what you want the playbook to do in plain English and Reach will generate the YAML for you. There's also a "Fix with AI" button that shows up when your YAML has syntax errors or other issues. It's a nice shortcut when you don't want to look up the exact format.

## Running a Playbook

Select a playbook from the list, make sure you're connected to a server, and hit Run. Each step runs in order and you see the output as it goes. If a step fails and `on_failure` is set to `stop`, the playbook halts there. Otherwise it keeps going and you can review what failed afterward.

## Storage

Playbooks are saved encrypted in the vault, same as sessions and credentials. They don't sit around as plaintext YAML files on disk.
