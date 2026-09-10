//! IPC for recipes.
//!
//! Nothing here executes anything. The invocation is built and handed back to
//! the frontend, which writes it into the session the user is looking at —
//! the same path their own typing takes. That keeps one rule intact: a recipe
//! can only reach a machine the user already has open in front of them.

use std::collections::HashMap;

use serde::Serialize;

use crate::recipe::{
    self,
    invoke::{self, Invocation},
    registry::{self, RecipeEntry, DEFAULT_RECIPES_URL},
    risk::{self, Analysis},
    schema, Recipe,
};

/// A recipe with its analysis, which is how the UI always wants it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeView {
    #[serde(flatten)]
    pub recipe: Recipe,
    pub analysis: Analysis,
}

fn view(recipe: Recipe) -> RecipeView {
    let analysis = risk::analyse(&recipe);
    RecipeView { recipe, analysis }
}

/// Every recipe on disk, each with its risk analysis.
#[tauri::command]
pub async fn recipe_list() -> Result<Vec<RecipeView>, String> {
    Ok(recipe::list(&crate::app_data_dir())
        .into_iter()
        .map(view)
        .collect())
}

/// One recipe by id.
#[tauri::command]
pub async fn recipe_get(id: String) -> Result<RecipeView, String> {
    recipe::get(&crate::app_data_dir(), &id).map(view)
}

/// Parse a source without saving it, so the editor can show errors as you type.
#[tauri::command]
pub async fn recipe_preview(source: String) -> Result<RecipeView, String> {
    schema::parse(&source).map(view).map_err(|e| e.to_string())
}

/// Write a recipe. Fails rather than storing something that will not parse.
#[tauri::command]
pub async fn recipe_save(source: String) -> Result<RecipeView, String> {
    recipe::save(&crate::app_data_dir(), &source).map(view)
}

/// Delete a recipe.
#[tauri::command]
pub async fn recipe_delete(id: String) -> Result<(), String> {
    recipe::delete(&crate::app_data_dir(), &id)
}

/// A blank recipe with a valid header, for the "new" button.
#[tauri::command]
pub async fn recipe_template(id: String, name: String) -> Result<String, String> {
    if !schema::valid_id(&id) {
        return Err(format!(
            "Invalid id '{id}': use lowercase letters, digits and hyphens"
        ));
    }
    Ok(schema::template(&id, &name))
}

/// What the frontend needs to run a recipe.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedRun {
    /// The exact bytes to write into the session.
    pub command: String,
    /// The heredoc delimiter, so the UI can explain what it is about to send.
    pub delimiter: String,
    /// Re-run at prepare time rather than trusted from the list: the file may
    /// have been edited since it was last analysed.
    pub analysis: Analysis,
}

/// Build the command for a recipe without running it.
///
/// Separate from any send step on purpose. The user sees exactly what will be
/// written before it is written, and nothing is sent by the act of preparing.
#[tauri::command]
pub async fn recipe_prepare(
    id: String,
    values: HashMap<String, String>,
) -> Result<PreparedRun, String> {
    let recipe = recipe::get(&crate::app_data_dir(), &id)?;
    let analysis = risk::analyse(&recipe);
    let Invocation { command, delimiter } =
        invoke::build(&recipe, &values).map_err(|e| e.to_string())?;
    Ok(PreparedRun {
        command,
        delimiter,
        analysis,
    })
}

/// The registry index.
#[tauri::command]
pub async fn recipe_fetch_registry(url: Option<String>) -> Result<Vec<RecipeEntry>, String> {
    let url = url.unwrap_or_else(|| DEFAULT_RECIPES_URL.to_string());
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err("Registry URL must be http(s)".into());
    }
    registry::fetch_index(&url).await
}

/// Download, verify and store a registry recipe.
#[tauri::command]
pub async fn recipe_install(entry: RecipeEntry) -> Result<RecipeView, String> {
    registry::install(&crate::app_data_dir(), &entry)
        .await
        .map(view)
}

/// The default registry URL, for the settings field.
#[tauri::command]
pub async fn recipe_registry_url() -> Result<String, String> {
    Ok(DEFAULT_RECIPES_URL.to_string())
}
