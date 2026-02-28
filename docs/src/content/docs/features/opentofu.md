---
title: OpenTofu
description: Manage infrastructure as code with OpenTofu directly from Reach.
---

Reach has a full OpenTofu workspace that covers the entire IaC workflow. You can manage projects, configure providers and resources through a visual UI, run plan/apply/destroy with streaming output, inspect and modify state, visualize dependency graphs, manage workspaces, and generate HCL files — all without touching the command line.

## Installing OpenTofu

Open the OpenTofu tab from the sidebar. If `tofu` isn't found in your PATH, Reach shows a setup screen with an **Install** button.

Click Install and Reach downloads the official OpenTofu binary for your platform. Installation progress streams in real-time. When it finishes, click **Continue** and Reach re-checks the tool status.

If OpenTofu is already installed, you see a green checkmark with the version number and go straight to the project list.

## Projects

Everything is organized around projects. A project maps to a directory on disk containing your `.tf` files. Project metadata (name, description, provider configs, variables, resources, outputs, backend, modules, locals, data sources, environments) is stored encrypted in the Reach vault.

### Creating a project

Two ways to create a project:

**New Project** — Click the button, fill in:
- **Project Name** — whatever you want to call it
- **Project Path** — click **Browse** to pick a directory, or type manually
- **Description** — optional

Reach creates the directory if needed and scaffolds a starter `main.tf`.

**From Template** — Click "From Template" to pick from a list of pre-built project templates (e.g., AWS EC2, Docker, Kubernetes). The template pre-populates providers, variables, and resources so you don't start from scratch.

### Project list

Cards show project name, path, description, and last-opened timestamp. Click to open. Trash icon to delete (with confirmation dialog). Deleting removes vault metadata but doesn't touch files on disk.

## Workspace

Opening a project gives you a two-panel layout.

### Left panel — file browser

Lists all files in the project directory. Click a file to view its contents in the right panel with syntax highlighting. The panel collapses with the chevron button. **Back to Projects** at the bottom returns to the project list.

### Right panel — 13 workspace tabs

| Tab | What it does |
|-----|-------------|
| **Actions** | Run tofu commands (init, plan, apply, destroy, validate, fmt) |
| **Providers** | Add and configure infrastructure providers |
| **Variables** | Define input variables with types and defaults |
| **Resources** | Add and configure infrastructure resources |
| **Data Sources** | Define data sources for reading external data |
| **Environments** | Manage variable value sets for different environments |
| **Backend** | Configure state storage backend |
| **Locals** | Define local computed values |
| **Modules** | Reference external modules |
| **State** | Inspect, move, remove, and import state resources |
| **Graph** | Visualize resource dependency graph |
| **Outputs** | Define outputs and view live values |
| **Workspaces** | Manage OpenTofu workspaces |

There's also a **Generate HCL** button on the right side of the tab bar that opens a preview of all the `.tf` files Reach would generate from your current configuration.

## Actions

The Actions tab is the command center.

### Command buttons

Six buttons across the top:

| Button | Command | What it does |
|--------|---------|-------------|
| **Init** | `tofu init` | Download providers and initialize the project |
| **Plan** | `tofu plan` | Preview what changes will be made |
| **Apply** | `tofu apply` | Apply changes to infrastructure |
| **Destroy** | `tofu destroy` | Tear down all managed resources |
| **Validate** | `tofu validate` | Check configuration syntax |
| **Format** | `tofu fmt` | Auto-format `.tf` files |

All buttons are disabled while a command is running.

### Execution target

Same as Ansible — pick **Local** or **SSH** to control where commands run. SSH shows a dropdown of active connections.

### Auto Approve

A toggle below the target selector. When enabled, `--auto-approve` is passed to apply and destroy commands so they don't prompt for confirmation. Use with caution.

### Plan viewer

After running a plan, a **View Plan** button appears. Click it to see a structured summary:

- **Summary bar** with color-coded badges: X to create (green), X to update (yellow), X to destroy (red), X to replace (blue)
- **Resource change cards** — each card shows the resource address, action badge, and provider name. Click to expand and see attribute-level changes in a table with old value → new value. Removed attributes show in red, added in green, sensitive values are masked.
- **Output changes** — if any outputs changed, they're listed separately

### Command output

Same streaming output as Ansible: stdout in white, stderr in red, system messages in accent color. Auto-scrolls. Clear button to reset.

## Providers

The Providers tab manages infrastructure providers (AWS, Azure, GCP, Docker, Kubernetes, etc.).

### Adding a provider

Click **Add Provider** to open the provider picker. It shows a catalog of available providers. Select one, give it a name, and it's added to your project.

### Configuring a provider

Click **Configure** on any provider card to open a configuration modal. The form fields are driven by the provider's schema — for example, AWS shows fields for `region`, `access_key`, `secret_key`, etc. Field types include text, numbers, booleans, dropdowns, and password fields for sensitive values.

### Fetching schema

Click **Fetch Schema** to download the provider's resource schema from the registry. This enables Reach to show you available resource types and their configuration fields when adding resources. A badge shows how many resource types are available after fetching.

### Removing a provider

Click **Remove** on the provider card. The provider configuration is removed from the project.

## Variables

The Variables tab defines input variables for your project.

### Variable table

Each variable shows: name, type, default value, sensitive flag, description, and edit/delete buttons. Sensitive variables show their default value masked.

### Adding / editing a variable

Click **Add Variable** or the edit button on an existing one. The editor modal has:

- **Name** — variable name (must be unique)
- **Type** — dropdown: `string`, `number`, `bool`, `list`, `map`
- **Default** — optional default value
- **Sensitive** — checkbox to mark the variable as sensitive
- **Description** — optional description

## Resources

The Resources tab manages infrastructure resources.

### Adding a resource

Click **Add Resource** to open the resource picker. If you've fetched a provider schema, the picker shows all available resource types for that provider (e.g., `aws_instance`, `aws_s3_bucket`). Select one and give it a logical name.

### Configuring a resource

Click **Configure** on a resource card. The configuration modal shows schema-driven fields specific to that resource type. For example, an `aws_instance` might show fields for `ami`, `instance_type`, `tags`, etc. Required fields are marked. Help text is shown where available.

### Removing a resource

Click **Remove** on the card.

## Data Sources

Data sources let you read information from external sources (existing infrastructure, APIs, etc.).

### Adding a data source

Click **Add Data Source** to open a picker with:
- **Search** — filter by name, type, or description
- **Category pills** — All, Compute, Network, Storage, Container, Utility
- **Grid** of available data sources

Select one, name it, and configure it through the schema-driven form.

## Environments

Environments let you maintain different sets of variable values for different deployments (dev, staging, production).

### Managing environments

- **Dropdown** to select an existing environment (active one marked with *)
- **Delete button** to remove an environment
- **New environment input** + **Add** button to create one

### Setting values

When an environment is selected, you see a form with one row per variable:
- Variable name and description
- Input field (password field if the variable is sensitive)
- Placeholder shows the variable's default value

Click **Save Values** to persist. Click **Set Active** to make this environment the one used by commands.

## Backend

The Backend tab configures where OpenTofu stores state.

### Selecting a backend

Dropdown with available backend types from the catalog (local, s3, azurerm, gcs, consul, http, etc.). Each shows a description.

### Configuring

After selecting a type, a form appears with schema-driven fields specific to that backend. For example, S3 shows bucket, key, region, encrypt, etc. Sensitive fields use password inputs.

### Current backend

If a backend is already configured, it shows the type and field values (sensitive ones masked). A **Remove** button clears it.

## Locals

Locals define computed values that can reference variables and other resources.

### Adding a local

Click **Add Local**. The form has:
- **Name** — must start with a letter or underscore, alphanumeric characters only. Duplicate names are rejected.
- **Expression** — HCL expression (e.g., `"${var.prefix}-${var.environment}"`)

### Managing locals

Each local card shows its name and expression. Edit and Delete buttons on each.

## Modules

Modules reference reusable OpenTofu configurations.

### Adding a module

Click **Add Module**. The form has:
- **Name** — required, validated
- **Source** — module source (local path like `./modules/vpc`, Git URL, or registry reference)
- **Version** — optional version constraint (e.g., `~> 1.0`)
- **Inputs** — dynamic key-value pairs. Click **Add Input** to add rows with key and value fields. Remove button per row.

### Managing modules

Each module card shows name, source, and version. Edit and Remove buttons.

## State

The State tab lets you inspect and modify the OpenTofu state file.

### Resource list

Shows all resources currently tracked in state. Click **Refresh** to reload.

### Operations per resource

- **Show** — runs `tofu state show <address>` and displays the result in the output panel
- **Move** — opens an input for the new address. Confirm to run `tofu state mv <old> <new>`. Useful for renaming resources.
- **Remove** — confirmation dialog, then runs `tofu state rm <address>`. Removes the resource from state without destroying it.

### Importing

Click **Import Resource** to open a modal with:
- **Address** — the resource address to import into (e.g., `aws_instance.web`)
- **ID** — the real-world resource ID (e.g., `i-1234567890abcdef0`)

Runs `tofu import <address> <id>` to bring an existing resource under management.

## Graph

The Graph tab visualizes the dependency relationships between your resources.

### Interactive canvas

- **Pan** — click and drag to move around
- **Zoom** — mouse wheel (0.3x to 3x scale)
- **Nodes** — colored rectangles representing resources. Colors vary by provider: AWS in amber, Azure in blue, Google in red, Docker in cyan, Kubernetes in blue.
- **Edges** — lines with arrows showing which resources depend on which

Click **Refresh** to regenerate the graph from the current configuration.

## Outputs

The Outputs tab has two sections.

### Output definitions

Define what values should be exported from your configuration.

Click **Add Output**. The editor has:
- **Name** — unique output name
- **Value** — HCL expression (e.g., `aws_instance.web.public_ip`)
- **Description** — optional
- **Sensitive** — checkbox to mask the value in logs

Edit and Delete buttons on each defined output.

### Live values

Click **Refresh** to fetch actual output values from the current state. Each value shows:
- Name
- Type badge (string, number, etc.)
- Actual value (or `[sensitive]` if marked sensitive)

This is the equivalent of running `tofu output` on the command line.

## Workspaces

Workspaces let you manage multiple isolated state environments within the same configuration.

### Current workspace

Highlighted box showing which workspace is active.

### Workspace list

Each workspace shows its name, a **Select** button to switch to it, and a **Delete** button (disabled for the `default` workspace — you can't delete it).

### Creating a workspace

Input field + **Create** button at the bottom. Creates the workspace with `tofu workspace new <name>` and switches to it.

## Generate HCL

The **Generate HCL** button in the tab bar opens a preview showing all the `.tf` files that Reach would generate from your current project configuration (providers, variables, resources, outputs, backend, locals, modules, data sources). You can review the generated code and write it to disk.

This is the bridge between the visual UI and actual HCL files. You can configure everything through the UI panels and then generate the corresponding Terraform-compatible code.

## Data storage

All project metadata — providers, variables, resources, outputs, backend config, environments, locals, modules, data sources — is stored encrypted in the Reach vault. The actual `.tf` files and state live on disk in your project directory.

Sensitive values like provider credentials, backend access keys, and sensitive variable defaults are encrypted at rest in the vault. The state file itself is managed by OpenTofu and should be secured through its own mechanisms (remote backends with encryption, etc.).

Projects persist across app restarts. When you reopen Reach, your project list and all configuration is loaded from the vault.
