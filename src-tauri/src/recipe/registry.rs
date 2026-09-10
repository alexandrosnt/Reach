//! The recipe registry.
//!
//! Same shape as the plugin marketplace and the theme registry, and the same
//! trust boundary: authors open a pull request against the registry repo, the
//! maintainer merges. That merge is what makes a recipe trusted. Everything
//! here — the hash pin, the size cap, the risk analysis — narrows the window
//! between "the maintainer approved these bytes" and "these bytes ran as root
//! on your server". None of it substitutes for the review.

use std::path::Path;
use std::sync::OnceLock;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::schema::{self, Origin, Recipe};

/// Default registry. The maintainer owns the repo and merges PRs to admit a
/// recipe, exactly as with plugins and themes.
pub const DEFAULT_RECIPES_URL: &str =
    "https://raw.githubusercontent.com/alexandrosnt/reach-recipes-registry/main/recipes.json";

/// Recipes are shell scripts — kilobytes. Anything approaching this is a
/// misconfigured entry, not a recipe.
const MAX_RECIPE_BYTES: usize = 512 * 1024;

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("reqwest client build should not fail with default features")
    })
}

/// One entry in `recipes.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeEntry {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    /// "user/repo", for the Source link.
    #[serde(default)]
    pub repo: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub targets: Vec<String>,
    /// What the author declares. Shown in the list; the real analysis happens
    /// against the downloaded script, and the two are compared.
    #[serde(default)]
    pub danger: Option<String>,
    /// Direct https URL to the `.sh` file.
    pub url: String,
    /// Hex SHA-256 of the bytes at `url`. Verified before anything is stored.
    pub sha256: String,
}

fn hex_lower(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Fetch the registry index.
pub async fn fetch_index(url: &str) -> Result<Vec<RecipeEntry>, String> {
    let resp = client()
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Fetch failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("Registry returned HTTP {}", resp.status()));
    }
    let text = resp
        .text()
        .await
        .map_err(|e| format!("Registry read failed: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("Invalid registry JSON: {e}"))
}

/// Download a recipe, verify it, and store it.
///
/// Order matters: hash first, then parse, then write. A recipe that fails
/// either check never touches the filesystem, so a bad registry entry cannot
/// leave a half-installed script behind for someone to run later.
pub async fn install(app_dir: &Path, entry: &RecipeEntry) -> Result<Recipe, String> {
    if !schema::valid_id(&entry.id) {
        return Err(format!("Invalid recipe id '{}'", entry.id));
    }
    if !entry.url.starts_with("https://") {
        return Err("Recipe url must be https".into());
    }
    let expected = entry.sha256.trim().to_ascii_lowercase();
    if expected.is_empty() {
        return Err("Registry entry has no sha256 — refusing to install".into());
    }

    let resp = client()
        .get(&entry.url)
        .send()
        .await
        .map_err(|e| format!("Download failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("Download returned HTTP {}", resp.status()));
    }
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("Download read failed: {e}"))?;
    if bytes.len() > MAX_RECIPE_BYTES {
        return Err(format!(
            "Recipe too large ({} bytes, cap {})",
            bytes.len(),
            MAX_RECIPE_BYTES
        ));
    }

    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let actual = hex_lower(&hasher.finalize());
    if actual != expected {
        return Err(format!("SHA-256 mismatch (expected {expected}, got {actual})"));
    }

    let source = String::from_utf8(bytes.to_vec())
        .map_err(|_| "Recipe is not valid UTF-8".to_string())?;
    let parsed = schema::parse(&source).map_err(|e| e.to_string())?;

    // A registry entry claiming one id while the file declares another means
    // the index and the script disagree about what is being installed. That is
    // never benign, so it fails rather than picking a winner.
    if parsed.id != entry.id {
        return Err(format!(
            "Recipe declares id '{}' but the registry lists '{}'",
            parsed.id, entry.id
        ));
    }

    let mut stored = super::save(app_dir, &source)?;
    let origin = Origin::Registry {
        repo: entry.repo.clone(),
        sha256: actual,
    };
    super::write_origin(app_dir, &entry.id, &origin)?;
    stored.origin = origin;
    Ok(stored)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_registry_index() {
        let json = r#"[{
            "id": "harden-ssh",
            "name": "Harden SSH",
            "version": "1.0.0",
            "description": "Locks down sshd.",
            "author": "someone",
            "repo": "someone/reach-recipes",
            "tags": ["security"],
            "targets": ["debian"],
            "danger": "sensitive",
            "url": "https://example.com/harden-ssh.sh",
            "sha256": "abc123"
        }]"#;
        let entries: Vec<RecipeEntry> = serde_json::from_str(json).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "harden-ssh");
        assert_eq!(entries[0].tags, vec!["security"]);
    }

    #[test]
    fn tolerates_a_minimal_entry() {
        // Older or hand-written registry files should not break the whole list.
        let json = r#"[{"id":"x","name":"X","url":"https://e/x.sh","sha256":"d"}]"#;
        let entries: Vec<RecipeEntry> = serde_json::from_str(json).unwrap();
        assert_eq!(entries[0].version, "");
        assert!(entries[0].tags.is_empty());
        assert_eq!(entries[0].danger, None);
    }

    #[test]
    fn hex_is_lowercase_and_padded() {
        assert_eq!(hex_lower(&[0x0a, 0xff, 0x00]), "0aff00");
    }

    #[tokio::test]
    async fn refuses_a_non_https_url() {
        let dir = std::env::temp_dir().join("reach-recipe-registry-http");
        let entry = RecipeEntry {
            id: "x".into(),
            name: "X".into(),
            version: String::new(),
            description: String::new(),
            author: String::new(),
            repo: String::new(),
            tags: vec![],
            targets: vec![],
            danger: None,
            url: "http://example.com/x.sh".into(),
            sha256: "abc".into(),
        };
        assert!(install(&dir, &entry).await.unwrap_err().contains("https"));
    }

    #[tokio::test]
    async fn refuses_an_entry_with_no_hash() {
        let dir = std::env::temp_dir().join("reach-recipe-registry-nohash");
        let entry = RecipeEntry {
            id: "x".into(),
            name: "X".into(),
            version: String::new(),
            description: String::new(),
            author: String::new(),
            repo: String::new(),
            tags: vec![],
            targets: vec![],
            danger: None,
            url: "https://example.com/x.sh".into(),
            sha256: "   ".into(),
        };
        assert!(install(&dir, &entry).await.unwrap_err().contains("sha256"));
    }

    #[tokio::test]
    async fn refuses_an_id_that_could_escape_the_directory() {
        let dir = std::env::temp_dir().join("reach-recipe-registry-escape");
        let entry = RecipeEntry {
            id: "../../evil".into(),
            name: "X".into(),
            version: String::new(),
            description: String::new(),
            author: String::new(),
            repo: String::new(),
            tags: vec![],
            targets: vec![],
            danger: None,
            url: "https://example.com/x.sh".into(),
            sha256: "abc".into(),
        };
        assert!(install(&dir, &entry).await.unwrap_err().contains("Invalid recipe id"));
    }
}
