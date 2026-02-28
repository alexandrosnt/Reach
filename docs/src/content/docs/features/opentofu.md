---
title: OpenTofu
description: Manage infrastructure as code with OpenTofu directly from Reach.
---

Reach includes an OpenTofu workspace for managing infrastructure-as-code projects. Plan, apply, and destroy infrastructure, inspect state, and manage providers and modules — all with streaming output and vault-backed project storage.

## Getting Started

Open the **OpenTofu** tab from the sidebar. If OpenTofu (`tofu`) isn't installed, Reach will detect this and offer a one-click install that downloads the official binary for your platform.

## Projects

OpenTofu work is organized into projects. Each project points to a directory containing your `.tf` files.

### Creating a Project

Click **New Project**, give it a name, pick a directory, and optionally add a description. Reach scaffolds a starter `main.tf` file in that directory.

### Opening a Project

Click on any project card to open its workspace.

## Workspace

The workspace has a file panel on the left and tabbed panels on the right.

### File Panel

Lists all files in the project directory. Click a file to open it in the built-in editor with HCL syntax highlighting. The panel can be collapsed.

### Execution Target

Choose where commands run:

- **Local** — Runs on your machine
- **SSH** — Runs on a connected remote host via SSH

### Commands

The main panel provides buttons for core OpenTofu operations:

| Command | What it does |
|---------|-------------|
| **Init** | Initialize the project (download providers and modules) |
| **Validate** | Check configuration syntax and consistency |
| **Plan** | Preview what changes will be made |
| **Apply** | Apply the planned changes to your infrastructure |
| **Destroy** | Tear down all managed resources |
| **Format** | Auto-format `.tf` files to canonical style |

Each command streams output in real-time with color-coded lines.

### State

Inspect the current Terraform state:

- **State List** — Show all resources currently managed
- **State Show** — View detailed attributes of a specific resource
- **Pull** — Download the current remote state

### Providers

View and manage providers used by the project. See installed provider versions and registry sources.

### Modules

View modules used in the project. Install new modules or update existing ones.

## Command Output

All command output streams in real-time. Lines are color-coded:

- Standard output in the default text color
- Errors (stderr) highlighted in red
- System messages (start/finish/exit code) in a muted color

The output panel auto-scrolls as new lines arrive.

## Data Storage

Project metadata is stored encrypted in the Reach vault. Your `.tf` files and state live on disk in the project directory. Sensitive values like state files and variable definitions should be managed through OpenTofu's own mechanisms (remote backends, `.tfvars` files, etc.).
