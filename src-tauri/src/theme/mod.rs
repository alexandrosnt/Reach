//! Theme registry and installation.
//!
//! Themes are data, not code. There is no archive to extract, nothing to
//! sandbox and no permissions to grant — a theme is a single JSON document of
//! colours. That makes this a much smaller surface than the plugin marketplace,
//! and the checks here focus on *completeness* rather than containment: a theme
//! missing a key would leave part of the app unstyled, so it is rejected.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Default registry. The maintainer owns the repo and merges PRs to admit a
/// theme; that merge is the trust boundary, exactly as for plugins.
pub const DEFAULT_THEMES_URL: &str =
    "https://raw.githubusercontent.com/alexandrosnt/reach-themes-registry/main/themes.json";

/// A theme document is tiny. Anything approaching this is not a theme.
const MAX_THEME_BYTES: usize = 256 * 1024;

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("reqwest client build should not fail with default features")
    })
}

/// One row of the registry index.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeEntry {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub version: String,
    /// "dark" or "light" — drives the root class so native chrome matches.
    pub appearance: String,
    #[serde(default)]
    pub description: String,
    /// HTTPS URL of the theme JSON.
    pub url: String,
    /// Hex SHA-256 of the bytes at `url`, verified before the theme is stored.
    pub sha256: String,
}

/// Keys every theme must define. Kept in step with the client-side validator and
/// the registry's CI check; all three must agree or a theme can pass one gate
/// and fail another.
const COLOR_KEYS: &[&str] = &[
    "bg-primary", "bg-secondary", "bg-elevated", "border",
    "text-primary", "text-secondary", "text-tertiary",
    "accent", "accent-hover", "success", "warning", "danger",
    "surface-hover", "surface-active", "surface-sunken", "surface-strong",
];

const TERMINAL_KEYS: &[&str] = &[
    "background", "foreground", "cursor", "cursorAccent",
    "selectionBackground", "selectionForeground",
    "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
    "brightBlack", "brightRed", "brightGreen", "brightYellow",
    "brightBlue", "brightMagenta", "brightCyan", "brightWhite",
];

/// Fetch the registry index.
pub async fn fetch_index(url: &str) -> Result<Vec<ThemeEntry>, String> {
    let resp = client()
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Fetch failed: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("Registry returned HTTP {}", resp.status()));
    }
    let bytes = resp.bytes().await.map_err(|e| format!("Read failed: {}", e))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("Invalid registry JSON: {}", e))
}

/// Reject anything that would leave the app half-painted, or that is not a
/// theme at all. Returns the parsed document so the caller can store it.
pub fn validate(doc: &serde_json::Value) -> Result<(), String> {
    let obj = doc.as_object().ok_or("Theme must be a JSON object")?;

    for key in ["id", "name", "appearance"] {
        match obj.get(key).and_then(|v| v.as_str()) {
            Some(s) if !s.is_empty() => {}
            _ => return Err(format!("Missing or empty \"{}\"", key)),
        }
    }

    let appearance = obj["appearance"].as_str().unwrap_or_default();
    if appearance != "dark" && appearance != "light" {
        return Err("appearance must be \"dark\" or \"light\"".into());
    }

    let colors = obj
        .get("colors")
        .and_then(|v| v.as_object())
        .ok_or("Missing \"colors\"")?;
    for key in COLOR_KEYS {
        if !colors.get(*key).map(|v| v.is_string()).unwrap_or(false) {
            return Err(format!("Missing colors.{}", key));
        }
    }

    let terminal = obj
        .get("terminal")
        .and_then(|v| v.as_object())
        .ok_or("Missing \"terminal\"")?;
    for key in TERMINAL_KEYS {
        if !terminal.get(*key).map(|v| v.is_string()).unwrap_or(false) {
            return Err(format!("Missing terminal.{}", key));
        }
    }

    Ok(())
}

/// Where installed themes live.
pub fn themes_dir(app_dir: &Path) -> PathBuf {
    app_dir.join("themes")
}

/// Download, verify and store a theme.
///
/// The hash is checked before anything touches the filesystem, and the document
/// is validated before it is stored, so a theme that would break the UI never
/// reaches disk.
pub async fn install(app_dir: &Path, entry: &ThemeEntry) -> Result<serde_json::Value, String> {
    // The id becomes a filename.
    if entry.id.is_empty()
        || !entry
            .id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(format!("Invalid theme id '{}'", entry.id));
    }
    if !entry.url.starts_with("https://") {
        return Err("Theme url must be https".into());
    }

    let resp = client()
        .get(&entry.url)
        .send()
        .await
        .map_err(|e| format!("Download failed: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("Download returned HTTP {}", resp.status()));
    }
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("Download read failed: {}", e))?;
    if bytes.len() > MAX_THEME_BYTES {
        return Err(format!(
            "Theme too large ({} bytes, cap {})",
            bytes.len(),
            MAX_THEME_BYTES
        ));
    }

    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let actual = hex_lower(&hasher.finalize());
    let expected = entry.sha256.trim().to_ascii_lowercase();
    if expected.is_empty() {
        return Err("Registry entry missing sha256 — refusing to install".into());
    }
    if actual != expected {
        return Err(format!(
            "SHA-256 mismatch (expected {}, got {})",
            expected, actual
        ));
    }

    let doc: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|e| format!("Theme is not valid JSON: {}", e))?;
    validate(&doc)?;

    if doc.get("id").and_then(|v| v.as_str()) != Some(entry.id.as_str()) {
        return Err("Theme id does not match the registry entry".into());
    }

    let dir = themes_dir(app_dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("Cannot create themes dir: {}", e))?;
    let dest = dir.join(format!("{}.json", entry.id));
    std::fs::write(&dest, &bytes).map_err(|e| format!("Cannot write theme: {}", e))?;

    Ok(doc)
}

/// Every installed theme, skipping any that no longer validate rather than
/// failing the whole list.
pub fn list_installed(app_dir: &Path) -> Vec<serde_json::Value> {
    let dir = themes_dir(app_dir);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Ok(raw) = std::fs::read(&path) else { continue };
        let Ok(doc) = serde_json::from_slice::<serde_json::Value>(&raw) else {
            tracing::warn!("Skipping unreadable theme {:?}", path);
            continue;
        };
        if let Err(e) = validate(&doc) {
            tracing::warn!("Skipping invalid theme {:?}: {}", path, e);
            continue;
        }
        out.push(doc);
    }
    out
}

/// Remove an installed theme.
pub fn uninstall(app_dir: &Path, theme_id: &str) -> Result<(), String> {
    if theme_id.is_empty()
        || !theme_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(format!("Invalid theme id '{}'", theme_id));
    }
    let path = themes_dir(app_dir).join(format!("{}.json", theme_id));
    if !path.exists() {
        return Ok(());
    }
    std::fs::remove_file(&path).map_err(|e| format!("Cannot remove theme: {}", e))
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(char_for_nibble(b >> 4));
        s.push(char_for_nibble(b & 0xf));
    }
    s
}

fn char_for_nibble(n: u8) -> char {
    match n {
        0..=9 => (b'0' + n) as char,
        10..=15 => (b'a' + n - 10) as char,
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn complete_theme() -> serde_json::Value {
        let mut colors = serde_json::Map::new();
        for k in COLOR_KEYS {
            colors.insert((*k).to_string(), serde_json::json!("#000000"));
        }
        let mut terminal = serde_json::Map::new();
        for k in TERMINAL_KEYS {
            terminal.insert((*k).to_string(), serde_json::json!("#000000"));
        }
        serde_json::json!({
            "id": "example",
            "name": "Example",
            "appearance": "dark",
            "colors": colors,
            "terminal": terminal
        })
    }

    #[test]
    fn accepts_a_complete_theme() {
        assert!(validate(&complete_theme()).is_ok());
    }

    /// A theme missing one key would leave that part of the app unstyled, which
    /// looks like an app bug rather than a theme bug — so it is refused.
    #[test]
    fn rejects_a_theme_missing_one_colour() {
        let mut t = complete_theme();
        t["colors"].as_object_mut().unwrap().remove("accent");
        let err = validate(&t).unwrap_err();
        assert!(err.contains("colors.accent"), "got: {}", err);
    }

    #[test]
    fn rejects_a_theme_missing_one_terminal_colour() {
        let mut t = complete_theme();
        t["terminal"].as_object_mut().unwrap().remove("brightCyan");
        let err = validate(&t).unwrap_err();
        assert!(err.contains("terminal.brightCyan"), "got: {}", err);
    }

    #[test]
    fn rejects_an_unknown_appearance() {
        let mut t = complete_theme();
        t["appearance"] = serde_json::json!("sepia");
        assert!(validate(&t).unwrap_err().contains("appearance"));
    }

    #[test]
    fn rejects_a_non_object() {
        assert!(validate(&serde_json::json!("nope")).is_err());
    }
}
