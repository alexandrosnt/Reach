use crate::tofu::types::{ProviderSchema, ResourceSchema, SchemaAttribute};

/// Parse `tofu providers schema -json` output into structured provider schemas.
pub fn parse_schema_json(raw: &str) -> Result<Vec<ProviderSchema>, String> {
    let root: serde_json::Value =
        serde_json::from_str(raw).map_err(|e| format!("Failed to parse schema JSON: {}", e))?;

    let provider_schemas = root
        .get("provider_schemas")
        .and_then(|v| v.as_object())
        .ok_or_else(|| "Missing provider_schemas in JSON".to_string())?;

    let mut result = Vec::new();

    for (source_key, schema_val) in provider_schemas {
        // source_key is like "registry.opentofu.org/hashicorp/aws"
        let source = source_key.clone();

        // Provider config attributes
        let provider_attributes = if let Some(provider_block) = schema_val.get("provider") {
            parse_block_attributes(provider_block)
        } else {
            vec![]
        };

        // Resource schemas
        let resource_schemas =
            if let Some(rs) = schema_val.get("resource_schemas").and_then(|v| v.as_object()) {
                rs.iter()
                    .map(|(rt, rv)| ResourceSchema {
                        resource_type: rt.clone(),
                        attributes: parse_block_attributes(rv),
                    })
                    .collect()
            } else {
                vec![]
            };

        // Data source schemas
        let data_source_schemas = if let Some(ds) = schema_val
            .get("data_source_schemas")
            .and_then(|v| v.as_object())
        {
            ds.iter()
                .map(|(dt, dv)| ResourceSchema {
                    resource_type: dt.clone(),
                    attributes: parse_block_attributes(dv),
                })
                .collect()
        } else {
            vec![]
        };

        result.push(ProviderSchema {
            source,
            provider_attributes,
            resource_schemas,
            data_source_schemas,
        });
    }

    Ok(result)
}

/// Parse attributes from a schema block (provider, resource, or data source).
fn parse_block_attributes(block: &serde_json::Value) -> Vec<SchemaAttribute> {
    let attributes = block
        .get("block")
        .and_then(|b| b.get("attributes"))
        .and_then(|a| a.as_object());

    let attrs = match attributes {
        Some(a) => a,
        None => return vec![],
    };

    let mut result: Vec<SchemaAttribute> = attrs
        .iter()
        .filter_map(|(name, attr)| {
            let computed = attr.get("computed").and_then(|v| v.as_bool()).unwrap_or(false);
            let required = attr.get("required").and_then(|v| v.as_bool()).unwrap_or(false);
            let optional = attr.get("optional").and_then(|v| v.as_bool()).unwrap_or(false);

            // Skip computed-only attributes (not user-configurable)
            if computed && !required && !optional {
                return None;
            }

            let attr_type = simplify_type(attr.get("type"));
            let description = attr
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let sensitive = attr.get("sensitive").and_then(|v| v.as_bool()).unwrap_or(false);

            Some(SchemaAttribute {
                name: name.clone(),
                attr_type,
                description,
                required,
                optional,
                computed,
                sensitive,
            })
        })
        .collect();

    // Sort: required first, then alphabetical
    result.sort_by(|a, b| {
        b.required
            .cmp(&a.required)
            .then_with(|| a.name.cmp(&b.name))
    });

    result
}

/// Simplify a Terraform type expression to a basic string.
fn simplify_type(type_val: Option<&serde_json::Value>) -> String {
    match type_val {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Array(arr)) => {
            // ["list", "string"] → "list", ["map", "string"] → "map", ["set", "string"] → "set"
            if let Some(first) = arr.first().and_then(|v| v.as_str()) {
                match first {
                    "list" | "set" => "list".to_string(),
                    "map" | "object" => "map".to_string(),
                    _ => first.to_string(),
                }
            } else {
                "string".to_string()
            }
        }
        _ => "string".to_string(),
    }
}
