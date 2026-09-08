//! IPC for the theme registry.
//!
//! Mirrors the plugin marketplace, minus everything that exists to contain
//! executable code — a theme is inert JSON, so there is no archive, no sandbox
//! and no permission prompt.

use crate::theme::{self, ThemeEntry, DEFAULT_THEMES_URL};

/// Fetch the registry index.
#[tauri::command]
pub async fn theme_fetch_registry(url: Option<String>) -> Result<Vec<ThemeEntry>, String> {
    let url = url.unwrap_or_else(|| DEFAULT_THEMES_URL.to_string());
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err("Registry URL must be http(s)".into());
    }
    theme::fetch_index(&url).await
}

/// Download, verify and store a theme, returning the stored document so the
/// frontend can apply it immediately.
#[tauri::command]
pub async fn theme_install(entry: ThemeEntry) -> Result<serde_json::Value, String> {
    let app_dir = crate::app_data_dir();
    theme::install(&app_dir, &entry).await
}

/// Every installed theme.
#[tauri::command]
pub async fn theme_list_installed() -> Result<Vec<serde_json::Value>, String> {
    Ok(theme::list_installed(&crate::app_data_dir()))
}

/// Remove an installed theme.
#[tauri::command]
pub async fn theme_uninstall(theme_id: String) -> Result<(), String> {
    theme::uninstall(&crate::app_data_dir(), &theme_id)
}

/// The registry URL currently in effect.
#[tauri::command]
pub async fn theme_default_registry_url() -> Result<String, String> {
    Ok(DEFAULT_THEMES_URL.to_string())
}
