use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::plugin::schema::PluginPermission;

/// Default registry URL. The user owns this repo and merges PRs to admit
/// plugins to the marketplace; that PR-merge is the trust boundary.
pub const DEFAULT_INDEX_URL: &str =
    "https://raw.githubusercontent.com/alexandrosnt/reach-plugins-registry/main/plugins.json";

/// Marketplace download client — separate from the per-plugin HTTP client so
/// we can give it a longer timeout suited for release-zip downloads.
fn download_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .expect("reqwest client build should not fail with default features")
    })
}

/// Hard cap on a downloaded plugin archive. Plugins are tiny (KB to a few MB);
/// anything larger is almost certainly a misconfigured registry entry.
const MAX_DOWNLOAD_BYTES: usize = 16 * 1024 * 1024;

/// One entry in the marketplace registry JSON file.
///
/// Authors submit a PR to the registry repo adding their entry; the registry
/// owner merges. Reach clients read this list and show the install option.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketplaceEntry {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    /// "user/repo" GitHub identifier — informational, used for the "Source" link.
    #[serde(default)]
    pub repo: String,
    /// Direct URL to the plugin's release zip archive.
    pub download_url: String,
    /// Hex-encoded SHA-256 of the bytes at `download_url`. Verified before
    /// extraction — install fails on mismatch.
    pub sha256: String,
    /// Permissions the plugin's manifest will declare. Surfaced *before*
    /// install so the user knows what they're agreeing to.
    #[serde(default)]
    pub permissions: Vec<PluginPermission>,
}

/// Fetch the registry index from a URL.
pub async fn fetch_index(url: &str) -> Result<Vec<MarketplaceEntry>, String> {
    let resp = download_client()
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Fetch failed: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("Registry returned HTTP {}", resp.status()));
    }
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("Read failed: {}", e))?;
    let entries: Vec<MarketplaceEntry> = serde_json::from_slice(&bytes)
        .map_err(|e| format!("Invalid registry JSON: {}", e))?;
    Ok(entries)
}

/// Download `entry.download_url`, verify SHA-256 against `entry.sha256`,
/// extract into `plugins_dir/{entry.id}/`, validating every entry path against
/// zip-slip and rejecting symlinks.
pub async fn install_entry(
    plugins_dir: &Path,
    entry: &MarketplaceEntry,
) -> Result<(), String> {
    if entry.id.is_empty() || entry.id.contains(['/', '\\', '\0', '.']) {
        return Err(format!("Invalid plugin id '{}'", entry.id));
    }

    let resp = download_client()
        .get(&entry.download_url)
        .send()
        .await
        .map_err(|e| format!("Download failed: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("Download returned HTTP {}", resp.status()));
    }
    if let Some(len) = resp.content_length() {
        if len as usize > MAX_DOWNLOAD_BYTES {
            return Err(format!(
                "Plugin archive too large ({} bytes, cap {})",
                len, MAX_DOWNLOAD_BYTES
            ));
        }
    }
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("Download read failed: {}", e))?;
    if bytes.len() > MAX_DOWNLOAD_BYTES {
        return Err(format!(
            "Plugin archive too large ({} bytes, cap {})",
            bytes.len(),
            MAX_DOWNLOAD_BYTES
        ));
    }

    // SHA-256 verify before touching the filesystem.
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

    // Extract into a staging directory next to the final destination so a
    // failed extraction can't leave a half-written plugin behind.
    std::fs::create_dir_all(plugins_dir)
        .map_err(|e| format!("Cannot create plugins dir: {}", e))?;
    let dest = plugins_dir.join(&entry.id);
    let staging = plugins_dir.join(format!(".{}.installing", &entry.id));
    if staging.exists() {
        let _ = std::fs::remove_dir_all(&staging);
    }
    std::fs::create_dir_all(&staging)
        .map_err(|e| format!("Cannot create staging dir: {}", e))?;

    extract_zip_safely(&bytes, &staging).map_err(|e| {
        let _ = std::fs::remove_dir_all(&staging);
        e
    })?;

    // Manifest must exist at the staging root.
    if !staging.join("plugin.toml").is_file() {
        let _ = std::fs::remove_dir_all(&staging);
        return Err("Archive does not contain plugin.toml at the root".into());
    }

    // Replace the existing install (if any) atomically-ish.
    if dest.exists() {
        std::fs::remove_dir_all(&dest)
            .map_err(|e| format!("Cannot remove existing install: {}", e))?;
    }
    std::fs::rename(&staging, &dest)
        .map_err(|e| format!("Cannot finalize install: {}", e))?;

    Ok(())
}

/// Remove a plugin's directory. Caller is responsible for unloading the VM
/// first via PluginManager::unload_plugin.
pub fn uninstall_entry(plugins_dir: &Path, plugin_id: &str) -> Result<(), String> {
    if plugin_id.is_empty() || plugin_id.contains(['/', '\\', '\0', '.']) {
        return Err(format!("Invalid plugin id '{}'", plugin_id));
    }
    let dir = plugins_dir.join(plugin_id);
    if !dir.exists() {
        return Ok(());
    }
    std::fs::remove_dir_all(&dir).map_err(|e| format!("Cannot remove plugin: {}", e))
}

/// Extract a zip into `dest`, rejecting any entry whose path escapes `dest`
/// (zip-slip) or that is a symbolic link. Caller has already SHA-verified the
/// bytes and capped the archive size.
fn extract_zip_safely(bytes: &[u8], dest: &Path) -> Result<(), String> {
    let dest_canon = dest
        .canonicalize()
        .map_err(|e| format!("Cannot canonicalize staging dir: {}", e))?;

    let reader = Cursor::new(bytes);
    let mut archive =
        zip::ZipArchive::new(reader).map_err(|e| format!("Bad zip: {}", e))?;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Bad zip entry: {}", e))?;

        // Reject symlinks outright — the zip-2.3 patch validates symlink
        // targets, but our threat model doesn't need symlinks at all.
        if file.is_symlink() {
            return Err(format!(
                "Archive contains a symbolic link ({:?}) — refusing to extract",
                file.name()
            ));
        }

        // `enclosed_name` returns Some only if the path is safe (no .., no
        // absolute paths, no drive prefixes). None means the entry is hostile
        // or malformed and must be rejected.
        let Some(rel_path): Option<PathBuf> = file.enclosed_name() else {
            return Err(format!("Unsafe path in archive: {:?}", file.name()));
        };
        if rel_path.as_os_str().is_empty() {
            continue;
        }

        let target = dest_canon.join(&rel_path);

        if file.is_dir() {
            std::fs::create_dir_all(&target)
                .map_err(|e| format!("Cannot create dir {:?}: {}", target, e))?;
            continue;
        }

        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Cannot create parent {:?}: {}", parent, e))?;
        }

        // Defense in depth: confirm the parent of `target` resolves inside
        // the staging dir. (`enclosed_name` already rules this out, but a
        // belt-and-braces check is cheap.)
        if let Some(parent) = target.parent() {
            let parent_canon = parent
                .canonicalize()
                .map_err(|e| format!("Cannot canonicalize {:?}: {}", parent, e))?;
            if !parent_canon.starts_with(&dest_canon) {
                return Err(format!(
                    "Refusing to write {:?} outside staging root",
                    target
                ));
            }
        }

        let mut out = std::fs::File::create(&target)
            .map_err(|e| format!("Cannot create file {:?}: {}", target, e))?;
        let mut buf = Vec::with_capacity(file.size() as usize);
        file.read_to_end(&mut buf)
            .map_err(|e| format!("Cannot read entry: {}", e))?;
        std::io::Write::write_all(&mut out, &buf)
            .map_err(|e| format!("Cannot write file: {}", e))?;
    }

    Ok(())
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
