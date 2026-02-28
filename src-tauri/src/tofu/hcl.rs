use crate::tofu::types::{
    DataSourceCatalogEntry, GeneratedFile, ProviderCatalogEntry, ProviderFieldType,
    ResourceCatalogEntry, TofuBackendConfig, TofuDataSource, TofuEnvironment, TofuLocal,
    TofuModuleConfig, TofuOutput, TofuProject, TofuProviderConfig, TofuResourceConfig,
    TofuVariable,
};

/// Generate `providers.tf` content. Sensitive fields become `var.<provider>_<field>` references.
pub fn generate_providers_tf(
    providers: &[TofuProviderConfig],
    catalog: &[ProviderCatalogEntry],
    backend: Option<&TofuBackendConfig>,
) -> String {
    if providers.is_empty() && backend.is_none() {
        return String::new();
    }

    let mut out = String::new();

    // terraform { required_providers { ... } backend "type" { ... } }
    out.push_str("terraform {\n");

    if !providers.is_empty() {
        out.push_str("  required_providers {\n");
        for prov in providers {
            let provider_name = provider_block_name(&prov.source);
            out.push_str(&format!(
                "    {} = {{\n      source  = \"{}\"\n",
                provider_name, prov.source
            ));
            if !prov.version.is_empty() {
                out.push_str(&format!("      version = \"{}\"\n", prov.version));
            }
            out.push_str("    }\n");
        }
        out.push_str("  }\n");
    }

    if let Some(backend) = backend {
        out.push_str(&generate_backend_block(backend));
    }

    out.push_str("}\n");

    // provider blocks
    for prov in providers {
        let provider_name = provider_block_name(&prov.source);
        let catalog_entry = catalog.iter().find(|c| c.id == prov.provider_id);

        out.push_str(&format!("\nprovider \"{}\" {{\n", provider_name));

        // Sort fields for deterministic output
        let mut fields: Vec<(&String, &serde_json::Value)> = prov.fields.iter().collect();
        fields.sort_by_key(|(k, _)| k.as_str());

        for (field_name, value) in &fields {
            let is_sensitive = catalog_entry
                .and_then(|c| c.fields.iter().find(|f| &f.name == *field_name))
                .map(|f| matches!(f.field_type, ProviderFieldType::Sensitive))
                .unwrap_or(false);

            if is_sensitive {
                // Reference a variable instead of embedding the value
                out.push_str(&format!(
                    "  {} = var.{}_{}\n",
                    field_name, provider_name, field_name
                ));
            } else {
                let val_str = value_to_hcl(value);
                if !val_str.is_empty() {
                    out.push_str(&format!("  {} = {}\n", field_name, val_str));
                }
            }
        }

        out.push_str("}\n");
    }

    out
}

/// Generate `variables.tf` content for user-defined vars + auto-generated sensitive provider vars + resource vars.
pub fn generate_variables_tf(
    variables: &[TofuVariable],
    providers: &[TofuProviderConfig],
    catalog: &[ProviderCatalogEntry],
    resources: &[TofuResourceConfig],
    resource_catalog: &[ResourceCatalogEntry],
) -> String {
    let mut out = String::new();

    // Auto-generated variables for sensitive provider fields
    for prov in providers {
        let provider_name = provider_block_name(&prov.source);
        let catalog_entry = catalog.iter().find(|c| c.id == prov.provider_id);

        if let Some(entry) = catalog_entry {
            for field in &entry.fields {
                if matches!(field.field_type, ProviderFieldType::Sensitive) {
                    let var_name = format!("{}_{}", provider_name, field.name);
                    out.push_str(&format!("variable \"{}\" {{\n", var_name));
                    out.push_str(&format!("  description = \"{} {} for {} provider\"\n", field.label, field.name, entry.name));
                    out.push_str("  type        = string\n");
                    out.push_str("  sensitive   = true\n");
                    if let Some(ref default) = field.default_value {
                        out.push_str(&format!("  default     = \"{}\"\n", escape_hcl_string(default)));
                    } else {
                        out.push_str("  default     = \"\"\n");
                    }
                    out.push_str("}\n\n");
                }
            }
        }
    }

    // Auto-generated variables for sensitive resource fields
    for res in resources {
        let res_entry = resource_catalog.iter().find(|c| c.id == res.resource_type);
        if let Some(entry) = res_entry {
            for field in &entry.fields {
                if matches!(field.field_type, ProviderFieldType::Sensitive) {
                    let var_name = format!("{}_{}_{}", res.resource_type, res.logical_name, field.name);
                    out.push_str(&format!("variable \"{}\" {{\n", var_name));
                    out.push_str(&format!("  description = \"{} for {} {}\"\n", field.label, entry.name, res.logical_name));
                    out.push_str("  type        = string\n");
                    out.push_str("  sensitive   = true\n");
                    out.push_str("  default     = \"\"\n");
                    out.push_str("}\n\n");
                }
            }
        }
    }

    // User-defined variables
    for var in variables {
        out.push_str(&format!("variable \"{}\" {{\n", var.name));
        if !var.description.is_empty() {
            out.push_str(&format!(
                "  description = \"{}\"\n",
                escape_hcl_string(&var.description)
            ));
        }
        out.push_str(&format!("  type        = {}\n", var.var_type.as_hcl()));
        if var.sensitive {
            out.push_str("  sensitive   = true\n");
        }
        if let Some(ref default) = var.default_value {
            if !default.is_empty() {
                let formatted = format_default_value(default, &var.var_type);
                out.push_str(&format!("  default     = {}\n", formatted));
            }
        }
        out.push_str("}\n\n");
    }

    out
}

/// Generate a `.tfvars` file for an environment.
pub fn generate_tfvars(
    env: &TofuEnvironment,
    variables: &[TofuVariable],
    providers: &[TofuProviderConfig],
    catalog: &[ProviderCatalogEntry],
    resources: &[TofuResourceConfig],
    resource_catalog: &[ResourceCatalogEntry],
) -> String {
    let mut out = String::new();

    // Sensitive provider field values
    for prov in providers {
        let provider_name = provider_block_name(&prov.source);
        let catalog_entry = catalog.iter().find(|c| c.id == prov.provider_id);

        if let Some(entry) = catalog_entry {
            for field in &entry.fields {
                if matches!(field.field_type, ProviderFieldType::Sensitive) {
                    let var_name = format!("{}_{}", provider_name, field.name);
                    // Check if this env has a value for it
                    if let Some(val) = env.values.get(&var_name) {
                        out.push_str(&format!(
                            "{} = \"{}\"\n",
                            var_name,
                            escape_hcl_string(val)
                        ));
                    } else {
                        // Check if the provider config has a direct value
                        if let Some(val) = prov.fields.get(&field.name) {
                            if let Some(s) = val.as_str() {
                                if !s.is_empty() {
                                    out.push_str(&format!(
                                        "{} = \"{}\"\n",
                                        var_name,
                                        escape_hcl_string(s)
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Sensitive resource field values
    for res in resources {
        let res_entry = resource_catalog.iter().find(|c| c.id == res.resource_type);
        if let Some(entry) = res_entry {
            for field in &entry.fields {
                if matches!(field.field_type, ProviderFieldType::Sensitive) {
                    let var_name = format!("{}_{}_{}", res.resource_type, res.logical_name, field.name);
                    if let Some(val) = env.values.get(&var_name) {
                        out.push_str(&format!(
                            "{} = \"{}\"\n",
                            var_name,
                            escape_hcl_string(val)
                        ));
                    } else if let Some(val) = res.fields.get(&field.name) {
                        if let Some(s) = val.as_str() {
                            if !s.is_empty() {
                                out.push_str(&format!(
                                    "{} = \"{}\"\n",
                                    var_name,
                                    escape_hcl_string(s)
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    // User-defined variable values
    for var in variables {
        if let Some(val) = env.values.get(&var.name) {
            if !val.is_empty() {
                let formatted = format_default_value(val, &var.var_type);
                out.push_str(&format!("{} = {}\n", var.name, formatted));
            }
        }
    }

    out
}

/// Generate `resources.tf` content.
pub fn generate_resources_tf(
    resources: &[TofuResourceConfig],
    resource_catalog: &[ResourceCatalogEntry],
) -> String {
    if resources.is_empty() {
        return String::new();
    }

    let mut out = String::new();

    for res in resources {
        let catalog_entry = resource_catalog.iter().find(|c| c.id == res.resource_type);

        out.push_str(&format!(
            "resource \"{}\" \"{}\" {{\n",
            res.resource_type, res.logical_name
        ));

        // Sort fields for deterministic output
        let mut fields: Vec<(&String, &serde_json::Value)> = res.fields.iter().collect();
        fields.sort_by_key(|(k, _)| k.as_str());

        // Collect special nested fields for known resource types
        let nested = collect_nested_fields(&res.resource_type, &fields);

        // Emit flat fields first
        for (field_name, value) in &fields {
            // Skip fields that are handled by nested block formatting
            if is_nested_field(&res.resource_type, field_name) {
                continue;
            }

            let is_sensitive = catalog_entry
                .and_then(|c| c.fields.iter().find(|f| &f.name == *field_name))
                .map(|f| matches!(f.field_type, ProviderFieldType::Sensitive))
                .unwrap_or(false);

            // Handle tags_name → tags block
            if field_name.as_str() == "tags_name" {
                let val_str = value_to_hcl(value);
                if !val_str.is_empty() {
                    out.push_str(&format!("  tags = {{\n    Name = {}\n  }}\n", val_str));
                }
                continue;
            }

            if is_sensitive {
                let var_name = format!("{}_{}_{}", res.resource_type, res.logical_name, field_name);
                out.push_str(&format!("  {} = var.{}\n", field_name, var_name));
            } else {
                let val_str = value_to_hcl(value);
                if !val_str.is_empty() {
                    out.push_str(&format!("  {} = {}\n", field_name, val_str));
                }
            }
        }

        // Emit nested blocks
        out.push_str(&nested);

        out.push_str("}\n\n");
    }

    out
}

/// Check if a field should be handled as part of a nested block.
fn is_nested_field(resource_type: &str, field_name: &str) -> bool {
    match resource_type {
        "docker_container" => matches!(field_name, "ports_internal" | "ports_external"),
        "google_compute_instance" => matches!(field_name, "boot_disk_image" | "network"),
        "kubernetes_deployment" => matches!(
            field_name,
            "container_name" | "container_image" | "container_port" | "replicas"
        ),
        "kubernetes_service" => matches!(
            field_name,
            "port" | "target_port" | "selector_app" | "type"
        ),
        "kubernetes_namespace" => matches!(field_name, "name"),
        "aws_security_group" => matches!(
            field_name,
            "ingress_from_port" | "ingress_to_port" | "ingress_protocol" | "ingress_cidr_blocks"
        ),
        "null_resource" => matches!(field_name, "triggers_always"),
        _ => false,
    }
}

/// Collect nested fields and format them as HCL nested blocks.
fn collect_nested_fields(
    resource_type: &str,
    fields: &[(&String, &serde_json::Value)],
) -> String {
    let get = |name: &str| -> Option<&serde_json::Value> {
        fields.iter().find(|(k, _)| k.as_str() == name).map(|(_, v)| *v)
    };

    let mut out = String::new();

    match resource_type {
        "docker_container" => {
            let internal = get("ports_internal");
            let external = get("ports_external");
            if internal.is_some() || external.is_some() {
                out.push_str("  ports {\n");
                if let Some(v) = internal {
                    let s = value_to_hcl(v);
                    if !s.is_empty() {
                        out.push_str(&format!("    internal = {}\n", s));
                    }
                }
                if let Some(v) = external {
                    let s = value_to_hcl(v);
                    if !s.is_empty() {
                        out.push_str(&format!("    external = {}\n", s));
                    }
                }
                out.push_str("  }\n");
            }
        }
        "google_compute_instance" => {
            if let Some(v) = get("boot_disk_image") {
                let s = value_to_hcl(v);
                if !s.is_empty() {
                    out.push_str("  boot_disk {\n    initialize_params {\n");
                    out.push_str(&format!("      image = {}\n", s));
                    out.push_str("    }\n  }\n");
                }
            }
            if let Some(v) = get("network") {
                let s = value_to_hcl(v);
                if !s.is_empty() {
                    out.push_str("  network_interface {\n");
                    out.push_str(&format!("    network = {}\n", s));
                    out.push_str("    access_config {}\n");
                    out.push_str("  }\n");
                }
            }
        }
        "kubernetes_deployment" => {
            let name = get("container_name");
            let image = get("container_image");
            let port = get("container_port");
            let replicas = get("replicas");

            out.push_str("  spec {\n");
            if let Some(v) = replicas {
                let s = value_to_hcl(v);
                if !s.is_empty() {
                    out.push_str(&format!("    replicas = {}\n", s));
                }
            }
            out.push_str("    template {\n      spec {\n        container {\n");
            if let Some(v) = name {
                let s = value_to_hcl(v);
                if !s.is_empty() {
                    out.push_str(&format!("          name  = {}\n", s));
                }
            }
            if let Some(v) = image {
                let s = value_to_hcl(v);
                if !s.is_empty() {
                    out.push_str(&format!("          image = {}\n", s));
                }
            }
            if let Some(v) = port {
                let s = value_to_hcl(v);
                if !s.is_empty() {
                    out.push_str(&format!(
                        "          port {{\n            container_port = {}\n          }}\n",
                        s
                    ));
                }
            }
            out.push_str("        }\n      }\n    }\n  }\n");
        }
        "kubernetes_service" => {
            let svc_type = get("type");
            let port = get("port");
            let target_port = get("target_port");
            let selector_app = get("selector_app");

            out.push_str("  spec {\n");
            if let Some(v) = svc_type {
                let s = value_to_hcl(v);
                if !s.is_empty() {
                    out.push_str(&format!("    type = {}\n", s));
                }
            }
            if let Some(v) = selector_app {
                let s = value_to_hcl(v);
                if !s.is_empty() {
                    out.push_str(&format!("    selector = {{\n      app = {}\n    }}\n", s));
                }
            }
            if port.is_some() || target_port.is_some() {
                out.push_str("    port {\n");
                if let Some(v) = port {
                    let s = value_to_hcl(v);
                    if !s.is_empty() {
                        out.push_str(&format!("      port        = {}\n", s));
                    }
                }
                if let Some(v) = target_port {
                    let s = value_to_hcl(v);
                    if !s.is_empty() {
                        out.push_str(&format!("      target_port = {}\n", s));
                    }
                }
                out.push_str("    }\n");
            }
            out.push_str("  }\n");
        }
        "kubernetes_namespace" => {
            if let Some(v) = get("name") {
                let s = value_to_hcl(v);
                if !s.is_empty() {
                    out.push_str(&format!("  metadata {{\n    name = {}\n  }}\n", s));
                }
            }
        }
        "aws_security_group" => {
            let from_port = get("ingress_from_port");
            let to_port = get("ingress_to_port");
            let protocol = get("ingress_protocol");
            let cidr = get("ingress_cidr_blocks");

            if from_port.is_some() || to_port.is_some() {
                out.push_str("  ingress {\n");
                if let Some(v) = from_port {
                    let s = value_to_hcl(v);
                    if !s.is_empty() {
                        out.push_str(&format!("    from_port   = {}\n", s));
                    }
                }
                if let Some(v) = to_port {
                    let s = value_to_hcl(v);
                    if !s.is_empty() {
                        out.push_str(&format!("    to_port     = {}\n", s));
                    }
                }
                if let Some(v) = protocol {
                    let s = value_to_hcl(v);
                    if !s.is_empty() {
                        out.push_str(&format!("    protocol    = {}\n", s));
                    }
                }
                if let Some(v) = cidr {
                    if let Some(s) = v.as_str() {
                        if !s.is_empty() {
                            let blocks: Vec<&str> = s.split(',').map(|b| b.trim()).collect();
                            let formatted: Vec<String> = blocks
                                .iter()
                                .map(|b| format!("\"{}\"", escape_hcl_string(b)))
                                .collect();
                            out.push_str(&format!(
                                "    cidr_blocks = [{}]\n",
                                formatted.join(", ")
                            ));
                        }
                    }
                }
                out.push_str("  }\n");
            }
        }
        "null_resource" => {
            if let Some(v) = get("triggers_always") {
                if v.as_bool() == Some(true) || v.as_str() == Some("true") {
                    out.push_str(
                        "  triggers = {\n    always_run = timestamp()\n  }\n",
                    );
                }
            }
        }
        _ => {}
    }

    out
}

/// Generate `outputs.tf` content for user-defined output blocks.
pub fn generate_outputs_tf(outputs: &[TofuOutput]) -> String {
    if outputs.is_empty() {
        return String::new();
    }

    let mut out = String::new();

    for output in outputs {
        out.push_str(&format!("output \"{}\" {{\n", output.name));
        out.push_str(&format!("  value = {}\n", output.value));
        if !output.description.is_empty() {
            out.push_str(&format!(
                "  description = \"{}\"\n",
                escape_hcl_string(&output.description)
            ));
        }
        if output.sensitive {
            out.push_str("  sensitive   = true\n");
        }
        out.push_str("}\n\n");
    }

    out
}

/// Generate `backend "type" { ... }` block for inside the terraform {} block.
fn generate_backend_block(backend: &TofuBackendConfig) -> String {
    let mut out = String::new();
    out.push_str(&format!("  backend \"{}\" {{\n", backend.backend_type));

    let mut fields: Vec<(&String, &serde_json::Value)> = backend.fields.iter().collect();
    fields.sort_by_key(|(k, _)| k.as_str());

    for (field_name, value) in &fields {
        let val_str = value_to_hcl(value);
        if !val_str.is_empty() {
            out.push_str(&format!("    {} = {}\n", field_name, val_str));
        }
    }

    out.push_str("  }\n");
    out
}

/// Generate `data_sources.tf` content.
pub fn generate_data_sources_tf(
    data_sources: &[TofuDataSource],
    _catalog: &[DataSourceCatalogEntry],
) -> String {
    if data_sources.is_empty() {
        return String::new();
    }

    let mut out = String::new();

    for ds in data_sources {
        out.push_str(&format!(
            "data \"{}\" \"{}\" {{\n",
            ds.data_type, ds.logical_name
        ));

        let mut fields: Vec<(&String, &serde_json::Value)> = ds.fields.iter().collect();
        fields.sort_by_key(|(k, _)| k.as_str());

        for (field_name, value) in &fields {
            let val_str = value_to_hcl(value);
            if !val_str.is_empty() {
                out.push_str(&format!("  {} = {}\n", field_name, val_str));
            }
        }

        out.push_str("}\n\n");
    }

    out
}

/// Generate `locals.tf` content.
pub fn generate_locals_tf(locals: &[TofuLocal]) -> String {
    if locals.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    out.push_str("locals {\n");

    for local in locals {
        // Expression is NOT quoted — raw HCL expression
        out.push_str(&format!("  {} = {}\n", local.name, local.expression));
    }

    out.push_str("}\n");
    out
}

/// Generate `modules.tf` content.
pub fn generate_modules_tf(modules: &[TofuModuleConfig]) -> String {
    if modules.is_empty() {
        return String::new();
    }

    let mut out = String::new();

    for module in modules {
        out.push_str(&format!("module \"{}\" {{\n", module.name));
        out.push_str(&format!(
            "  source = \"{}\"\n",
            escape_hcl_string(&module.source)
        ));
        if !module.version.is_empty() {
            out.push_str(&format!(
                "  version = \"{}\"\n",
                escape_hcl_string(&module.version)
            ));
        }

        // Sort inputs for deterministic output
        let mut inputs: Vec<(&String, &serde_json::Value)> = module.inputs.iter().collect();
        inputs.sort_by_key(|(k, _)| k.as_str());

        if !inputs.is_empty() {
            out.push('\n');
            for (key, value) in &inputs {
                let val_str = value_to_hcl(value);
                if !val_str.is_empty() {
                    out.push_str(&format!("  {} = {}\n", key, val_str));
                }
            }
        }

        out.push_str("}\n\n");
    }

    out
}

/// Generate all files for a project.
pub fn generate_all(
    project: &TofuProject,
    catalog: &[ProviderCatalogEntry],
    resource_catalog: &[ResourceCatalogEntry],
    data_source_catalog: &[DataSourceCatalogEntry],
) -> Vec<GeneratedFile> {
    let mut files = Vec::new();

    if !project.providers.is_empty() || project.backend.is_some() {
        let providers_tf = generate_providers_tf(
            &project.providers,
            catalog,
            project.backend.as_ref(),
        );
        if !providers_tf.is_empty() {
            files.push(GeneratedFile {
                filename: "providers.tf".into(),
                content: providers_tf,
            });
        }

        let variables_tf = generate_variables_tf(
            &project.variables,
            &project.providers,
            catalog,
            &project.resources,
            resource_catalog,
        );
        if !variables_tf.is_empty() {
            files.push(GeneratedFile {
                filename: "variables.tf".into(),
                content: variables_tf,
            });
        }
    }

    // Generate resources.tf
    if !project.resources.is_empty() {
        let resources_tf = generate_resources_tf(&project.resources, resource_catalog);
        if !resources_tf.is_empty() {
            files.push(GeneratedFile {
                filename: "resources.tf".into(),
                content: resources_tf,
            });
        }
    }

    // Generate data_sources.tf
    if !project.data_sources.is_empty() {
        let data_tf = generate_data_sources_tf(&project.data_sources, data_source_catalog);
        if !data_tf.is_empty() {
            files.push(GeneratedFile {
                filename: "data_sources.tf".into(),
                content: data_tf,
            });
        }
    }

    // Generate locals.tf
    if !project.locals.is_empty() {
        let locals_tf = generate_locals_tf(&project.locals);
        if !locals_tf.is_empty() {
            files.push(GeneratedFile {
                filename: "locals.tf".into(),
                content: locals_tf,
            });
        }
    }

    // Generate modules.tf
    if !project.modules.is_empty() {
        let modules_tf = generate_modules_tf(&project.modules);
        if !modules_tf.is_empty() {
            files.push(GeneratedFile {
                filename: "modules.tf".into(),
                content: modules_tf,
            });
        }
    }

    // Generate tfvars per environment
    for env in &project.environments {
        let tfvars = generate_tfvars(
            env,
            &project.variables,
            &project.providers,
            catalog,
            &project.resources,
            resource_catalog,
        );
        if !tfvars.is_empty() {
            files.push(GeneratedFile {
                filename: format!("{}.tfvars", env.name),
                content: tfvars,
            });
        }
    }

    // Generate outputs.tf
    if !project.outputs.is_empty() {
        let outputs_tf = generate_outputs_tf(&project.outputs);
        if !outputs_tf.is_empty() {
            files.push(GeneratedFile {
                filename: "outputs.tf".into(),
                content: outputs_tf,
            });
        }
    }

    files
}

/// Extract the provider block name from a source string (e.g. "hashicorp/aws" -> "aws").
fn provider_block_name(source: &str) -> String {
    source
        .rsplit('/')
        .next()
        .unwrap_or(source)
        .to_string()
}

/// Escape a string for use inside HCL double quotes.
fn escape_hcl_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

/// Convert a serde_json::Value to an HCL value string.
fn value_to_hcl(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => {
            if s.is_empty() {
                String::new()
            } else {
                format!("\"{}\"", escape_hcl_string(s))
            }
        }
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        _ => String::new(),
    }
}

/// Format a default value based on variable type.
fn format_default_value(value: &str, var_type: &crate::tofu::types::TofuVarType) -> String {
    use crate::tofu::types::TofuVarType;
    match var_type {
        TofuVarType::String => format!("\"{}\"", escape_hcl_string(value)),
        TofuVarType::Number => value.to_string(),
        TofuVarType::Bool => value.to_lowercase(),
        TofuVarType::List | TofuVarType::Map => {
            // If it looks like JSON, pass through; otherwise wrap in quotes
            if value.starts_with('[') || value.starts_with('{') {
                value.to_string()
            } else {
                format!("\"{}\"", escape_hcl_string(value))
            }
        }
    }
}
