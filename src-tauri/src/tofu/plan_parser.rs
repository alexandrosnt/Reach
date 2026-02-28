use crate::tofu::types::{
    TofuAttributeChange, TofuChangeAction, TofuOutputChange, TofuPlanSummary,
    TofuResourceChange,
};

/// Parse `tofu show -json .reach-plan` output into a structured summary.
pub fn parse_plan_json(json_str: &str) -> Result<TofuPlanSummary, String> {
    let root: serde_json::Value =
        serde_json::from_str(json_str).map_err(|e| format!("Failed to parse plan JSON: {}", e))?;

    let mut resource_changes = Vec::new();
    let mut output_changes = Vec::new();

    // Parse resource_changes
    if let Some(changes) = root.get("resource_changes").and_then(|v| v.as_array()) {
        for change in changes {
            let address = change
                .get("address")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let resource_type = change
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let name = change
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let provider = change
                .get("provider_name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let action = if let Some(ch) = change.get("change") {
                parse_actions(ch.get("actions").and_then(|v| v.as_array()))
            } else {
                TofuChangeAction::NoOp
            };

            // Compute attribute diffs
            let attribute_changes = if let Some(ch) = change.get("change") {
                compute_attribute_changes(
                    ch.get("before"),
                    ch.get("after"),
                    ch.get("after_sensitive"),
                )
            } else {
                vec![]
            };

            resource_changes.push(TofuResourceChange {
                address,
                resource_type,
                name,
                provider,
                action,
                attribute_changes,
            });
        }
    }

    // Parse output_changes
    if let Some(changes) = root.get("output_changes").and_then(|v| v.as_object()) {
        for (name, change) in changes {
            let action = parse_actions(change.get("actions").and_then(|v| v.as_array()));
            output_changes.push(TofuOutputChange {
                name: name.clone(),
                action,
            });
        }
    }

    let has_changes = resource_changes
        .iter()
        .any(|r| !matches!(r.action, TofuChangeAction::NoOp));

    Ok(TofuPlanSummary {
        resource_changes,
        output_changes,
        has_changes,
    })
}

/// Map action strings array to a TofuChangeAction.
fn parse_actions(actions: Option<&Vec<serde_json::Value>>) -> TofuChangeAction {
    let actions = match actions {
        Some(a) => a,
        None => return TofuChangeAction::NoOp,
    };

    let strs: Vec<&str> = actions.iter().filter_map(|v| v.as_str()).collect();

    match strs.as_slice() {
        ["create"] => TofuChangeAction::Create,
        ["update"] => TofuChangeAction::Update,
        ["delete"] => TofuChangeAction::Delete,
        ["delete", "create"] | ["create", "delete"] => TofuChangeAction::Replace,
        ["read"] => TofuChangeAction::Read,
        ["no-op"] | [] => TofuChangeAction::NoOp,
        _ => TofuChangeAction::Update, // fallback
    }
}

/// Compute attribute-level diffs between before and after objects.
fn compute_attribute_changes(
    before: Option<&serde_json::Value>,
    after: Option<&serde_json::Value>,
    after_sensitive: Option<&serde_json::Value>,
) -> Vec<TofuAttributeChange> {
    let mut changes = Vec::new();

    let before_obj = before.and_then(|v| v.as_object());
    let after_obj = after.and_then(|v| v.as_object());
    let sensitive_obj = after_sensitive.and_then(|v| v.as_object());

    // Collect all keys from both before and after
    let mut all_keys = std::collections::BTreeSet::new();
    if let Some(obj) = before_obj {
        for key in obj.keys() {
            all_keys.insert(key.clone());
        }
    }
    if let Some(obj) = after_obj {
        for key in obj.keys() {
            all_keys.insert(key.clone());
        }
    }

    for key in all_keys {
        let old_val = before_obj.and_then(|o| o.get(&key));
        let new_val = after_obj.and_then(|o| o.get(&key));

        // Skip if both are the same
        if old_val == new_val {
            continue;
        }

        let sensitive = sensitive_obj
            .and_then(|o| o.get(&key))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        changes.push(TofuAttributeChange {
            attribute: key,
            old_value: old_val.cloned(),
            new_value: new_val.cloned(),
            sensitive,
        });
    }

    changes
}
