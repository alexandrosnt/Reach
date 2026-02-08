use crate::playbook::parser::interpolate_vars;
use std::collections::HashMap;

/// Translate a module + args into shell commands.
/// Returns a list of shell command strings to execute sequentially.
pub fn translate_module(
    module: &str,
    args: &serde_yaml::Value,
    vars: &HashMap<String, String>,
) -> Result<Vec<String>, String> {
    match module {
        "shell" => translate_shell(args, vars),
        "command" => translate_command(args, vars),
        "copy" => translate_copy(args, vars),
        "file" => translate_file(args, vars),
        "apt" => translate_apt(args, vars),
        "systemd" | "service" => translate_systemd(args, vars),
        "lineinfile" => translate_lineinfile(args, vars),
        "template" => translate_template(args, vars),
        _ => Err(format!("Unsupported module: {}", module)),
    }
}

/// Wrap a command with sudo if become is true.
pub fn wrap_become(commands: Vec<String>, use_become: bool) -> Vec<String> {
    if !use_become {
        return commands;
    }
    commands
        .into_iter()
        .map(|cmd| format!("sudo sh -c '{}'", cmd.replace('\'', "'\\''")))
        .collect()
}

fn get_str<'a>(
    args: &'a serde_yaml::Value,
    key: &str,
    vars: &HashMap<String, String>,
) -> Option<String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| interpolate_vars(s, vars))
}

fn get_bool(args: &serde_yaml::Value, key: &str) -> Option<bool> {
    args.get(key).and_then(|v| v.as_bool())
}

// ── shell ──

fn translate_shell(
    args: &serde_yaml::Value,
    vars: &HashMap<String, String>,
) -> Result<Vec<String>, String> {
    match args {
        serde_yaml::Value::String(cmd) => Ok(vec![interpolate_vars(cmd, vars)]),
        serde_yaml::Value::Mapping(_) => {
            let cmd = get_str(args, "cmd", vars)
                .ok_or("shell module requires 'cmd' or a string argument")?;
            Ok(vec![cmd])
        }
        _ => Err("shell module requires a string or mapping with 'cmd'".to_string()),
    }
}

// ── command ──

fn translate_command(
    args: &serde_yaml::Value,
    vars: &HashMap<String, String>,
) -> Result<Vec<String>, String> {
    match args {
        serde_yaml::Value::String(cmd) => Ok(vec![interpolate_vars(cmd, vars)]),
        serde_yaml::Value::Mapping(_) => {
            let cmd = get_str(args, "cmd", vars)
                .ok_or("command module requires 'cmd' or a string argument")?;
            Ok(vec![cmd])
        }
        _ => Err("command module requires a string or mapping with 'cmd'".to_string()),
    }
}

// ── copy ──

fn translate_copy(
    args: &serde_yaml::Value,
    vars: &HashMap<String, String>,
) -> Result<Vec<String>, String> {
    let dest = get_str(args, "dest", vars).ok_or("copy module requires 'dest'")?;

    if let Some(content) = get_str(args, "content", vars) {
        // Write content directly using heredoc
        let escaped_content = content.replace('\\', "\\\\");
        let mut cmds = vec![format!(
            "cat <<'REACH_EOF' > {}\n{}\nREACH_EOF",
            dest, escaped_content
        )];

        // Apply mode if specified
        if let Some(mode) = get_str(args, "mode", vars) {
            cmds.push(format!("chmod {} {}", mode, dest));
        }
        if let Some(owner) = get_str(args, "owner", vars) {
            if let Some(group) = get_str(args, "group", vars) {
                cmds.push(format!("chown {}:{} {}", owner, group, dest));
            } else {
                cmds.push(format!("chown {} {}", owner, dest));
            }
        }

        Ok(cmds)
    } else if let Some(src) = get_str(args, "src", vars) {
        // Copy from source (remote-to-remote copy)
        let mut cmds = vec![format!("cp -f {} {}", src, dest)];
        if let Some(mode) = get_str(args, "mode", vars) {
            cmds.push(format!("chmod {} {}", mode, dest));
        }
        Ok(cmds)
    } else {
        Err("copy module requires 'content' or 'src'".to_string())
    }
}

// ── file ──

fn translate_file(
    args: &serde_yaml::Value,
    vars: &HashMap<String, String>,
) -> Result<Vec<String>, String> {
    let path = get_str(args, "path", vars)
        .or_else(|| get_str(args, "dest", vars))
        .ok_or("file module requires 'path' or 'dest'")?;
    let state = get_str(args, "state", vars).unwrap_or_else(|| "file".to_string());

    let mut cmds = Vec::new();

    match state.as_str() {
        "directory" => cmds.push(format!("mkdir -p {}", path)),
        "absent" => cmds.push(format!("rm -rf {}", path)),
        "touch" => cmds.push(format!("touch {}", path)),
        "file" | "link" | "hard" => {
            // For "file" state, just ensure it exists or apply perms
        }
        _ => return Err(format!("Unsupported file state: {}", state)),
    }

    if let Some(mode) = get_str(args, "mode", vars) {
        cmds.push(format!("chmod {} {}", mode, path));
    }
    if let Some(owner) = get_str(args, "owner", vars) {
        if let Some(group) = get_str(args, "group", vars) {
            cmds.push(format!("chown {}:{} {}", owner, group, path));
        } else {
            cmds.push(format!("chown {} {}", owner, path));
        }
    }

    if cmds.is_empty() {
        cmds.push(format!("test -e {}", path));
    }

    Ok(cmds)
}

// ── apt ──

fn translate_apt(
    args: &serde_yaml::Value,
    vars: &HashMap<String, String>,
) -> Result<Vec<String>, String> {
    let mut cmds = Vec::new();

    let update_cache = get_bool(args, "update_cache").unwrap_or(false);
    if update_cache {
        cmds.push("apt-get update -qq".to_string());
    }

    if let Some(name) = get_str(args, "name", vars) {
        let state = get_str(args, "state", vars).unwrap_or_else(|| "present".to_string());
        let packages: Vec<&str> = name.split(',').map(|s| s.trim()).collect();
        let pkg_list = packages.join(" ");

        match state.as_str() {
            "present" | "latest" => {
                cmds.push(format!("DEBIAN_FRONTEND=noninteractive apt-get install -y {}", pkg_list));
            }
            "absent" => {
                cmds.push(format!("apt-get remove -y {}", pkg_list));
            }
            _ => return Err(format!("Unsupported apt state: {}", state)),
        }
    }

    if cmds.is_empty() {
        return Err("apt module requires 'name' or 'update_cache'".to_string());
    }

    Ok(cmds)
}

// ── systemd / service ──

fn translate_systemd(
    args: &serde_yaml::Value,
    vars: &HashMap<String, String>,
) -> Result<Vec<String>, String> {
    let name = get_str(args, "name", vars).ok_or("systemd module requires 'name'")?;
    let mut cmds = Vec::new();

    let daemon_reload = get_bool(args, "daemon_reload").unwrap_or(false);
    if daemon_reload {
        cmds.push("systemctl daemon-reload".to_string());
    }

    if let Some(state) = get_str(args, "state", vars) {
        match state.as_str() {
            "started" => cmds.push(format!("systemctl start {}", name)),
            "stopped" => cmds.push(format!("systemctl stop {}", name)),
            "restarted" => cmds.push(format!("systemctl restart {}", name)),
            "reloaded" => cmds.push(format!("systemctl reload {}", name)),
            _ => return Err(format!("Unsupported systemd state: {}", state)),
        }
    }

    if let Some(enabled) = get_bool(args, "enabled") {
        if enabled {
            cmds.push(format!("systemctl enable {}", name));
        } else {
            cmds.push(format!("systemctl disable {}", name));
        }
    }

    if cmds.is_empty() {
        return Err("systemd module requires 'state', 'enabled', or 'daemon_reload'".to_string());
    }

    Ok(cmds)
}

// ── lineinfile ──

fn translate_lineinfile(
    args: &serde_yaml::Value,
    vars: &HashMap<String, String>,
) -> Result<Vec<String>, String> {
    let path = get_str(args, "path", vars)
        .or_else(|| get_str(args, "dest", vars))
        .ok_or("lineinfile module requires 'path'")?;

    let state = get_str(args, "state", vars).unwrap_or_else(|| "present".to_string());

    if state == "absent" {
        if let Some(regexp) = get_str(args, "regexp", vars) {
            let escaped = regexp.replace('/', "\\/");
            return Ok(vec![format!("sed -i '/{}/d' {}", escaped, path)]);
        } else if let Some(line) = get_str(args, "line", vars) {
            let escaped = line.replace('/', "\\/");
            return Ok(vec![format!("sed -i '/^{}$/d' {}", escaped, path)]);
        }
        return Err("lineinfile absent requires 'regexp' or 'line'".to_string());
    }

    // state == present
    if let Some(line) = get_str(args, "line", vars) {
        if let Some(regexp) = get_str(args, "regexp", vars) {
            // Replace matching line with new line
            let escaped_regexp = regexp.replace('/', "\\/");
            let escaped_line = line.replace('/', "\\/").replace('&', "\\&");
            Ok(vec![format!(
                "grep -qP '{}' {} && sed -i 's/{}/{}/g' {} || echo '{}' >> {}",
                regexp, path, escaped_regexp, escaped_line, path, line, path
            )])
        } else {
            // Ensure line is present (append if not found)
            let escaped_line = line.replace('\'', "'\\''");
            Ok(vec![format!(
                "grep -qF '{}' {} || echo '{}' >> {}",
                escaped_line, path, escaped_line, path
            )])
        }
    } else {
        Err("lineinfile module requires 'line' when state=present".to_string())
    }
}

// ── template ──

fn translate_template(
    args: &serde_yaml::Value,
    vars: &HashMap<String, String>,
) -> Result<Vec<String>, String> {
    // Template works like copy with content, but content should already have vars interpolated
    let dest = get_str(args, "dest", vars).ok_or("template module requires 'dest'")?;

    if let Some(content) = get_str(args, "content", vars) {
        let escaped_content = content.replace('\\', "\\\\");
        let mut cmds = vec![format!(
            "cat <<'REACH_EOF' > {}\n{}\nREACH_EOF",
            dest, escaped_content
        )];
        if let Some(mode) = get_str(args, "mode", vars) {
            cmds.push(format!("chmod {} {}", mode, dest));
        }
        Ok(cmds)
    } else if let Some(src) = get_str(args, "src", vars) {
        // For template with src, we read the content inline (it should already be interpolated)
        let mut cmds = vec![format!("cp -f {} {}", src, dest)];
        if let Some(mode) = get_str(args, "mode", vars) {
            cmds.push(format!("chmod {} {}", mode, dest));
        }
        Ok(cmds)
    } else {
        Err("template module requires 'content' or 'src'".to_string())
    }
}
