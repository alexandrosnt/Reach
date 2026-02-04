use thiserror::Error;

use super::schema::Playbook;

#[derive(Debug, Error)]
pub enum ParserError {
    #[error("YAML parse error: {0}")]
    YamlError(#[from] serde_yaml::Error),
    #[error("Validation error: {0}")]
    ValidationError(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Parse a YAML string into a Playbook.
pub fn parse_yaml(yaml: &str) -> Result<Playbook, ParserError> {
    let playbook: Playbook = serde_yaml::from_str(yaml)?;
    validate_playbook(&playbook)?;
    Ok(playbook)
}

/// Parse a YAML file from disk into a Playbook.
pub async fn parse_file(path: &str) -> Result<Playbook, ParserError> {
    let content = tokio::fs::read_to_string(path).await?;
    parse_yaml(&content)
}

/// Validate a parsed playbook for logical consistency.
fn validate_playbook(playbook: &Playbook) -> Result<(), ParserError> {
    if playbook.name.is_empty() {
        return Err(ParserError::ValidationError(
            "Playbook name cannot be empty".to_string(),
        ));
    }
    if playbook.steps.is_empty() {
        return Err(ParserError::ValidationError(
            "Playbook must have at least one step".to_string(),
        ));
    }
    // Validate each step
    for (i, step) in playbook.steps.iter().enumerate() {
        if step.name.is_empty() {
            return Err(ParserError::ValidationError(
                format!("Step {} has an empty name", i + 1),
            ));
        }
        if step.command.is_empty() {
            return Err(ParserError::ValidationError(
                format!("Step '{}' has an empty command", step.name),
            ));
        }
        // Validate on_failure values
        if let Some(ref on_failure) = step.on_failure {
            match on_failure.as_str() {
                "stop" | "continue" => {}
                other => {
                    return Err(ParserError::ValidationError(
                        format!(
                            "Step '{}' has invalid on_failure value '{}' (expected 'stop' or 'continue')",
                            step.name, other
                        ),
                    ));
                }
            }
        }
    }
    Ok(())
}
