use crate::playbook::schema::{Play, Task};
use std::collections::HashMap;

/// Keys that are NOT module names in a task definition.
const RESERVED_KEYS: &[&str] = &[
    "name",
    "when",
    "register",
    "become",
    "ignore_errors",
    "vars",
    "tags",
    "notify",
    "changed_when",
    "failed_when",
    "loop",
    "with_items",
    "environment",
    "no_log",
    "delegate_to",
    "run_once",
    "block",
    "rescue",
    "always",
];

/// Parse a YAML playbook string into a list of plays.
pub fn parse_playbook(yaml_str: &str) -> Result<Vec<Play>, String> {
    let value: serde_yaml::Value =
        serde_yaml::from_str(yaml_str).map_err(|e| format!("YAML parse error: {}", e))?;

    match value {
        serde_yaml::Value::Sequence(items) => {
            // Could be list of plays (each has 'hosts' or 'tasks') or a single play as list of tasks
            if items.is_empty() {
                return Ok(Vec::new());
            }

            // Check if items look like plays (have 'tasks' or 'hosts' key)
            let first = &items[0];
            if first.is_mapping()
                && (first.get("tasks").is_some()
                    || first.get("hosts").is_some()
                    || first.get("name").is_some() && first.get("tasks").is_some())
            {
                // List of plays
                let mut plays = Vec::new();
                for item in &items {
                    let play: Play = serde_yaml::from_value(item.clone())
                        .map_err(|e| format!("Failed to parse play: {}", e))?;
                    plays.push(play);
                }
                Ok(plays)
            } else {
                // Treat entire list as tasks in a single play
                Ok(vec![Play {
                    name: None,
                    use_become: None,
                    vars: HashMap::new(),
                    tasks: items,
                }])
            }
        }
        serde_yaml::Value::Mapping(_) => {
            // Single play as a mapping
            let play: Play = serde_yaml::from_value(value)
                .map_err(|e| format!("Failed to parse play: {}", e))?;
            Ok(vec![play])
        }
        _ => Err("Playbook must be a YAML mapping or list".to_string()),
    }
}

/// Extract structured tasks from raw YAML task values.
pub fn extract_tasks(raw_tasks: &[serde_yaml::Value]) -> Result<Vec<Task>, String> {
    let mut tasks = Vec::new();

    for raw in raw_tasks {
        let map = match raw.as_mapping() {
            Some(m) => m,
            None => return Err("Each task must be a YAML mapping".to_string()),
        };

        let name = map
            .get(serde_yaml::Value::String("name".to_string()))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let when = map
            .get(serde_yaml::Value::String("when".to_string()))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let register = map
            .get(serde_yaml::Value::String("register".to_string()))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let use_become = map
            .get(serde_yaml::Value::String("become".to_string()))
            .and_then(|v| v.as_bool());

        let ignore_errors = map
            .get(serde_yaml::Value::String("ignore_errors".to_string()))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Find the module key: first key not in RESERVED_KEYS
        let mut module = None;
        let mut args = serde_yaml::Value::Null;

        for (k, v) in map {
            if let Some(key_str) = k.as_str() {
                if !RESERVED_KEYS.contains(&key_str) {
                    module = Some(key_str.to_string());
                    args = v.clone();
                    break;
                }
            }
        }

        let module = module.ok_or_else(|| {
            format!(
                "No module found in task: {}",
                name.as_deref().unwrap_or("(unnamed)")
            )
        })?;

        tasks.push(Task {
            name,
            module,
            args,
            when,
            register,
            use_become,
            ignore_errors,
        });
    }

    Ok(tasks)
}

/// Interpolate `{{ var }}` in a string using the provided variables map.
pub fn interpolate_vars(input: &str, vars: &HashMap<String, String>) -> String {
    let mut result = input.to_string();
    for (key, value) in vars {
        // Match both {{ key }} and {{key}}
        let patterns = [
            format!("{{{{ {} }}}}", key),
            format!("{{{{{}}}}}", key),
        ];
        for pattern in &patterns {
            result = result.replace(pattern, value);
        }
    }
    result
}

/// Evaluate a simple `when` condition.
/// Supports: `var is defined`, `var == "value"`, `var != "value"`, `result.rc != 0`, bare variables (truthy check).
pub fn evaluate_when(condition: &str, vars: &HashMap<String, String>) -> bool {
    let condition = condition.trim();

    // "var is defined"
    if condition.ends_with(" is defined") {
        let var_name = condition.trim_end_matches(" is defined").trim();
        return vars.contains_key(var_name);
    }

    // "var is not defined"
    if condition.ends_with(" is not defined") {
        let var_name = condition.trim_end_matches(" is not defined").trim();
        return !vars.contains_key(var_name);
    }

    // "var == value" or "var != value"
    if let Some(pos) = condition.find("!=") {
        let left = condition[..pos].trim();
        let right = condition[pos + 2..].trim().trim_matches('"').trim_matches('\'');
        let left_val = resolve_var(left, vars);
        return left_val != right;
    }

    if let Some(pos) = condition.find("==") {
        let left = condition[..pos].trim();
        let right = condition[pos + 2..].trim().trim_matches('"').trim_matches('\'');
        let left_val = resolve_var(left, vars);
        return left_val == right;
    }

    // Bare variable — truthy check
    if let Some(val) = vars.get(condition) {
        return !val.is_empty() && val != "0" && val.to_lowercase() != "false";
    }

    // Unknown condition — default to true (run the task)
    true
}

/// Resolve a dotted variable name like `result.rc` from the vars map.
fn resolve_var<'a>(name: &str, vars: &'a HashMap<String, String>) -> &'a str {
    // Direct lookup first
    if let Some(val) = vars.get(name) {
        return val;
    }
    // For dotted notation like `result.rc`, we stored individual fields as `result.stdout`, `result.rc`, etc.
    ""
}
