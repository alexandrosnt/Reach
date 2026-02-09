---
title: Plugins
description: Extend Reach with Lua plugins for custom workflows and automation.
---

Reach has a plugin system that lets you extend the app with custom Lua scripts. Plugins run in a sandboxed environment and can interact with SSH connections, store data, and hook into app events.

This feature is currently in **beta**.

## How plugins work

Each plugin is a Lua script (Luau dialect) that runs inside a sandboxed VM. Plugins can:

- Execute commands on active SSH connections
- Store and retrieve persistent data
- Register hooks that run on specific events (e.g., on connect, on disconnect)
- Provide UI elements that appear in the plugin panel

The sandbox restricts file system access and network calls to prevent plugins from doing anything unexpected. Plugins can only interact with Reach through the provided host API.

## Managing plugins

Go to **Settings > Plugins** to manage your plugins. From there you can:

- **Install** a plugin from a `.lua` file
- **Enable / Disable** installed plugins
- **Remove** plugins you no longer need
- **View** plugin details including name, version, and description

## Writing a plugin

A plugin is a single `.lua` file with a metadata header and one or more hook functions.

### Basic structure

```lua
-- Plugin metadata
PLUGIN = {
    name = "My Plugin",
    version = "1.0.0",
    description = "A simple example plugin",
    author = "Your Name"
}

-- Called when the plugin is loaded
function on_load()
    log("Plugin loaded!")
end

-- Called when an SSH connection is established
function on_connect(connection_id, host)
    log("Connected to " .. host)
end

-- Called when an SSH connection is closed
function on_disconnect(connection_id, host)
    log("Disconnected from " .. host)
end
```

### Host API

Plugins have access to the following host API functions:

| Function | Description |
|----------|-------------|
| `log(message)` | Print a message to the plugin log |
| `ssh_exec(connection_id, command)` | Execute a command on an SSH connection and return the output |
| `storage_get(key)` | Retrieve a value from persistent plugin storage |
| `storage_set(key, value)` | Store a value persistently (survives app restarts) |
| `storage_delete(key)` | Remove a value from storage |

### Available hooks

| Hook | Arguments | When it runs |
|------|-----------|--------------|
| `on_load()` | — | Plugin is loaded or enabled |
| `on_unload()` | — | Plugin is disabled or removed |
| `on_connect(id, host)` | connection ID, hostname | SSH connection established |
| `on_disconnect(id, host)` | connection ID, hostname | SSH connection closed |

### Storage

Plugin storage is persistent and scoped to each plugin. Data is stored encrypted in the vault alongside your other secrets. Use `storage_get` and `storage_set` to read and write key-value pairs.

```lua
-- Save a counter
local count = storage_get("run_count") or 0
count = count + 1
storage_set("run_count", count)
log("This plugin has run " .. count .. " times")
```

## Example: Server Info Plugin

Here's a complete example plugin that collects basic info from a server whenever you connect and logs it. You can use this as a starting point for your own plugins.

```lua
PLUGIN = {
    name = "Server Info",
    version = "1.0.0",
    description = "Logs basic system info when you connect to a server",
    author = "Reach"
}

function on_load()
    log("Server Info plugin ready")
end

function on_connect(connection_id, host)
    log("Collecting info from " .. host .. "...")

    -- Get hostname
    local hostname = ssh_exec(connection_id, "hostname")
    log("Hostname: " .. hostname)

    -- Get OS info
    local os_info = ssh_exec(connection_id, "cat /etc/os-release 2>/dev/null | grep PRETTY_NAME | cut -d'\"' -f2")
    if os_info and os_info ~= "" then
        log("OS: " .. os_info)
    end

    -- Get uptime
    local uptime = ssh_exec(connection_id, "uptime -p 2>/dev/null || uptime")
    log("Uptime: " .. uptime)

    -- Get memory usage
    local mem = ssh_exec(connection_id, "free -h 2>/dev/null | grep Mem | awk '{print $3 \"/\" $2}'")
    if mem and mem ~= "" then
        log("Memory: " .. mem)
    end

    -- Track how many times we've connected to this host
    local key = "connect_count_" .. host
    local count = storage_get(key) or 0
    count = count + 1
    storage_set(key, count)
    log("You've connected to " .. host .. " " .. count .. " time(s)")
end

function on_disconnect(connection_id, host)
    log("Disconnected from " .. host)
end
```

Save this as `server-info.lua` and install it from **Settings > Plugins**. Next time you connect to a server, check the plugin log to see the output.

## Security

Plugins run in a sandboxed Luau VM with no access to:
- The local file system
- Network sockets
- OS-level operations
- Other plugins' storage

The only way a plugin can interact with the outside world is through the host API. This means a misbehaving plugin can't read your files, make network requests, or interfere with other plugins.

## Limitations

- Plugins cannot create custom UI beyond the built-in elements
- No async/await — SSH commands block until they return
- Plugin storage values must be strings or numbers
- The plugin API is in beta and may change in future versions
