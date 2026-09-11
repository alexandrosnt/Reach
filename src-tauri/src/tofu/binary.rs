//! OpenTofu binaries that Reach owns.
//!
//! OpenTofu is one static binary per platform, so the honest way to run it
//! is the way tenv does: download the release for this OS and CPU into a
//! directory Reach controls, verify it against the checksums the project
//! publishes, and run it by absolute path. No PATH, no "restart the app",
//! and the version that ran is a fact, not a guess.
//!
//! Which version runs is the project's decision. A `.opentofu-version` file
//! (tenv's convention) pins it exactly; an exact `required_version` in the
//! HCL does the same. Without a pin the newest managed version is used, and
//! only when nothing is managed at all does a `tofu` on PATH get a turn.

use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};

const RELEASES: &str = "https://github.com/opentofu/opentofu/releases/download";
const LATEST_API: &str = "https://api.github.com/repos/opentofu/opentofu/releases/latest";
const PIN_FILE: &str = ".opentofu-version";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BinarySource {
    /// Downloaded and verified by Reach, under the tools directory.
    Managed,
    /// Whatever `tofu` (or `terraform`) the PATH offers.
    Path,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedBinary {
    pub path: PathBuf,
    /// Known exactly for a managed binary; read from `--version` for PATH.
    pub version: Option<String>,
    pub source: BinarySource,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BinaryStatus {
    pub resolved: Option<ResolvedBinary>,
    /// Managed versions on disk, newest first.
    pub installed: Vec<String>,
    /// What the project asked for, and where it said so.
    pub pinned: Option<String>,
    pub pin_source: Option<String>,
    /// A `tofu` on PATH exists, so "use system" is a real choice.
    pub path_available: bool,
}

#[derive(Debug)]
pub enum ResolveError {
    /// The project pins a version Reach has not downloaded yet.
    PinnedNotInstalled(String),
    NotFound,
}

pub fn managed_root() -> PathBuf {
    crate::app_data_dir().join("tools").join("opentofu")
}

fn exe_name() -> &'static str {
    if cfg!(windows) {
        "tofu.exe"
    } else {
        "tofu"
    }
}

pub fn managed_binary(version: &str) -> PathBuf {
    managed_root().join(version).join(exe_name())
}

/// `1.11.4` → `[1, 11, 4]`, for ordering. Pre-release suffixes sort last.
fn version_key(v: &str) -> (Vec<u64>, bool) {
    let (core, pre) = match v.split_once('-') {
        Some((c, _)) => (c, true),
        None => (v, false),
    };
    let nums = core.split('.').map(|p| p.parse::<u64>().unwrap_or(0)).collect();
    (nums, !pre)
}

pub fn installed_versions() -> Vec<String> {
    let mut out: Vec<String> = match std::fs::read_dir(managed_root()) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .filter(|e| e.path().join(exe_name()).is_file())
            .filter_map(|e| e.file_name().into_string().ok())
            .collect(),
        Err(_) => Vec::new(),
    };
    out.sort_by(|a, b| version_key(b).cmp(&version_key(a)));
    out
}

/// A version the project pins, and the file that says so.
///
/// `.opentofu-version` wins. Failing that, an *exact* `required_version`
/// (`= 1.9.0` or plain `1.9.0`) counts; a range like `>= 1.6` is a
/// constraint, not a choice, and is left to the default.
pub fn pinned_version(project_dir: &Path) -> Option<(String, &'static str)> {
    if let Ok(text) = std::fs::read_to_string(project_dir.join(PIN_FILE)) {
        let v = text.trim().trim_start_matches('v').to_string();
        if !v.is_empty() {
            return Some((v, PIN_FILE));
        }
    }
    let entries = std::fs::read_dir(project_dir).ok()?;
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("tf") {
            continue;
        }
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Some(v) = exact_required_version(&text) {
                return Some((v, "required_version"));
            }
        }
    }
    None
}

/// The version out of `required_version = "..."` when it is exact.
pub fn exact_required_version(hcl: &str) -> Option<String> {
    for line in hcl.lines() {
        let l = line.trim();
        let Some(rest) = l.strip_prefix("required_version") else { continue };
        let rest = rest.trim_start().strip_prefix('=')?.trim();
        let quoted = rest.trim_matches('"').trim();
        let v = quoted.strip_prefix('=').map(str::trim).unwrap_or(quoted);
        if v.is_empty() || v.starts_with(['>', '<', '~', '!']) {
            return None;
        }
        if v.chars().all(|c| c.is_ascii_digit() || c == '.') {
            return Some(v.to_string());
        }
        return None;
    }
    None
}

fn path_binary() -> Option<ResolvedBinary> {
    let bin = which::which("tofu").or_else(|_| which::which("terraform")).ok()?;
    let version = std::process::Command::new(&bin)
        .arg("version")
        .output()
        .ok()
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout);
            s.lines()
                .next()
                .and_then(|l| l.split_whitespace().last())
                .map(|v| v.trim_start_matches('v').to_string())
        });
    Some(ResolvedBinary { path: bin, version, source: BinarySource::Path })
}

/// The binary a project should run.
pub fn resolve(project_dir: Option<&Path>) -> Result<ResolvedBinary, ResolveError> {
    if let Some((v, _)) = project_dir.and_then(pinned_version) {
        let p = managed_binary(&v);
        if p.is_file() {
            return Ok(ResolvedBinary { path: p, version: Some(v), source: BinarySource::Managed });
        }
        // A PATH tofu of exactly that version honours the pin too.
        if let Some(pb) = path_binary() {
            if pb.version.as_deref() == Some(v.as_str()) {
                return Ok(pb);
            }
        }
        return Err(ResolveError::PinnedNotInstalled(v));
    }
    if let Some(v) = installed_versions().into_iter().next() {
        return Ok(ResolvedBinary {
            path: managed_binary(&v),
            version: Some(v),
            source: BinarySource::Managed,
        });
    }
    path_binary().ok_or(ResolveError::NotFound)
}

pub fn status(project_dir: Option<&Path>) -> BinaryStatus {
    let (pinned, pin_source) = match project_dir.and_then(pinned_version) {
        Some((v, s)) => (Some(v), Some(s.to_string())),
        None => (None, None),
    };
    BinaryStatus {
        resolved: resolve(project_dir).ok(),
        installed: installed_versions(),
        pinned,
        pin_source,
        path_available: path_binary().is_some(),
    }
}

pub fn write_pin(project_dir: &Path, version: Option<&str>) -> Result<(), String> {
    let file = project_dir.join(PIN_FILE);
    match version {
        Some(v) => std::fs::write(&file, format!("{}\n", v.trim())).map_err(|e| e.to_string()),
        None => match std::fs::remove_file(&file) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.to_string()),
        },
    }
}

pub fn remove(version: &str) -> Result<(), String> {
    let dir = managed_root().join(version);
    if !dir.join(exe_name()).is_file() {
        return Err(format!("OpenTofu {} is not installed", version));
    }
    std::fs::remove_dir_all(dir).map_err(|e| e.to_string())
}

/// The release asset for a platform, as OpenTofu names them.
pub fn asset_name(version: &str, os: &str, arch: &str) -> Option<String> {
    let os = match os {
        "windows" => "windows",
        "macos" => "darwin",
        "linux" => "linux",
        _ => return None,
    };
    let arch = match arch {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        "x86" => "386",
        "arm" => "arm",
        _ => return None,
    };
    let ext = if os == "windows" { "zip" } else { "tar.gz" };
    Some(format!("tofu_{}_{}_{}.{}", version, os, arch, ext))
}

/// The checksum for one asset out of a `SHA256SUMS` file.
pub fn sum_for(sums: &str, asset: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        let hash = parts.next()?;
        let name = parts.next()?.trim_start_matches('*');
        (name == asset).then(|| hash.to_ascii_lowercase())
    })
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

async fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("Reach (https://github.com/alexandrosnt/Reach)")
        .build()
        .map_err(|e| e.to_string())
}

pub async fn latest_version() -> Result<String, String> {
    let c = client().await?;
    let v: serde_json::Value = c
        .get(LATEST_API)
        .send()
        .await
        .map_err(|e| format!("Could not reach GitHub: {}", e))?
        .error_for_status()
        .map_err(|e| format!("GitHub answered {}", e))?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    v.get("tag_name")
        .and_then(|t| t.as_str())
        .map(|t| t.trim_start_matches('v').to_string())
        .ok_or_else(|| "GitHub's latest release has no tag".to_string())
}

/// Download, verify and unpack one version. Says what it is doing through
/// `progress`; returns the version installed.
pub async fn install(
    version: Option<String>,
    progress: impl Fn(&str),
) -> Result<String, String> {
    let version = match version {
        Some(v) => v.trim_start_matches('v').to_string(),
        None => {
            progress("Asking GitHub for the latest release…");
            latest_version().await?
        }
    };
    if managed_binary(&version).is_file() {
        progress(&format!("OpenTofu {} is already installed.", version));
        return Ok(version);
    }
    let asset = asset_name(&version, std::env::consts::OS, std::env::consts::ARCH)
        .ok_or_else(|| format!("No OpenTofu build for {} {}", std::env::consts::OS, std::env::consts::ARCH))?;
    let c = client().await?;

    progress(&format!("Downloading {}…", asset));
    let bytes = c
        .get(format!("{}/v{}/{}", RELEASES, version, asset))
        .send()
        .await
        .map_err(|e| format!("Download failed: {}", e))?
        .error_for_status()
        .map_err(|e| format!("No such release asset ({})", e))?
        .bytes()
        .await
        .map_err(|e| e.to_string())?;

    progress("Verifying against the published SHA256SUMS…");
    let sums = c
        .get(format!("{}/v{}/tofu_{}_SHA256SUMS", RELEASES, version, version))
        .send()
        .await
        .map_err(|e| format!("Could not fetch checksums: {}", e))?
        .error_for_status()
        .map_err(|e| format!("No checksums for this release ({})", e))?
        .text()
        .await
        .map_err(|e| e.to_string())?;
    let want = sum_for(&sums, &asset).ok_or_else(|| format!("{} is not listed in SHA256SUMS", asset))?;
    let got = hex(&Sha256::digest(&bytes));
    if got != want {
        return Err(format!(
            "Checksum mismatch for {}: expected {}, got {}. Nothing was installed.",
            asset, want, got
        ));
    }

    progress("Unpacking…");
    let dir = managed_root().join(&version);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let target = dir.join(exe_name());
    let data = extract_tofu(&bytes, asset.ends_with(".zip"))?;
    std::fs::write(&target, data).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
    }
    progress(&format!("OpenTofu {} installed at {}", version, target.display()));
    Ok(version)
}

/// The `tofu` executable out of a release archive.
fn extract_tofu(bytes: &[u8], is_zip: bool) -> Result<Vec<u8>, String> {
    let wanted = exe_name();
    if is_zip {
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| e.to_string())?;
        for i in 0..zip.len() {
            let mut f = zip.by_index(i).map_err(|e| e.to_string())?;
            if f.name() == wanted {
                let mut out = Vec::with_capacity(f.size() as usize);
                f.read_to_end(&mut out).map_err(|e| e.to_string())?;
                return Ok(out);
            }
        }
    } else {
        let gz = flate2::read::GzDecoder::new(bytes);
        let mut tar = tar::Archive::new(gz);
        for entry in tar.entries().map_err(|e| e.to_string())? {
            let mut e = entry.map_err(|e| e.to_string())?;
            let name = e.path().map_err(|e| e.to_string())?.to_string_lossy().to_string();
            if name == wanted || name.ends_with(&format!("/{}", wanted)) {
                let mut out = Vec::new();
                e.read_to_end(&mut out).map_err(|e| e.to_string())?;
                return Ok(out);
            }
        }
    }
    Err(format!("The archive does not contain {}", wanted))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("reach-binary-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn asset_names_follow_the_release_convention() {
        assert_eq!(asset_name("1.11.4", "windows", "x86_64").unwrap(), "tofu_1.11.4_windows_amd64.zip");
        assert_eq!(asset_name("1.11.4", "macos", "aarch64").unwrap(), "tofu_1.11.4_darwin_arm64.tar.gz");
        assert_eq!(asset_name("1.11.4", "linux", "x86_64").unwrap(), "tofu_1.11.4_linux_amd64.tar.gz");
        assert!(asset_name("1.11.4", "freebsd", "x86_64").is_none());
    }

    #[test]
    fn checksum_lookup_tolerates_binary_mode_markers() {
        let sums = "abc123  tofu_1.11.4_linux_amd64.tar.gz\nDEF456 *tofu_1.11.4_windows_amd64.zip\n";
        assert_eq!(sum_for(sums, "tofu_1.11.4_linux_amd64.tar.gz").as_deref(), Some("abc123"));
        assert_eq!(sum_for(sums, "tofu_1.11.4_windows_amd64.zip").as_deref(), Some("def456"));
        assert!(sum_for(sums, "tofu_1.11.4_darwin_arm64.tar.gz").is_none());
    }

    #[test]
    fn exact_required_version_only() {
        assert_eq!(exact_required_version("terraform {\n  required_version = \"1.9.0\"\n}").as_deref(), Some("1.9.0"));
        assert_eq!(exact_required_version("required_version = \"= 1.9.0\"").as_deref(), Some("1.9.0"));
        assert!(exact_required_version("required_version = \">= 1.6\"").is_none());
        assert!(exact_required_version("required_version = \"~> 1.9\"").is_none());
        assert!(exact_required_version("provider \"aws\" {}").is_none());
    }

    #[test]
    fn the_pin_file_beats_the_hcl() {
        let d = scratch("pin");
        std::fs::write(d.join("main.tf"), "terraform {\n  required_version = \"1.8.0\"\n}\n").unwrap();
        assert_eq!(pinned_version(&d), Some(("1.8.0".into(), "required_version")));
        std::fs::write(d.join(".opentofu-version"), "v1.11.4\n").unwrap();
        assert_eq!(pinned_version(&d), Some(("1.11.4".into(), ".opentofu-version")));
        write_pin(&d, None).unwrap();
        assert_eq!(pinned_version(&d), Some(("1.8.0".into(), "required_version")));
        write_pin(&d, Some("1.10.0")).unwrap();
        assert_eq!(pinned_version(&d).unwrap().0, "1.10.0");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn versions_order_newest_first_and_prereleases_last() {
        let mut v = vec!["1.9.0", "1.11.4", "1.11.0-beta1", "1.10.10", "1.11.10"];
        v.sort_by(|a, b| version_key(b).cmp(&version_key(a)));
        assert_eq!(v, vec!["1.11.10", "1.11.4", "1.11.0-beta1", "1.10.10", "1.9.0"]);
    }

    /// The real thing: GitHub, the release asset, the checksums, the unpack.
    /// Ignored by default because it needs the network and ~30 MB; run with
    /// `cargo test -- --ignored downloads_and_verifies`.
    #[tokio::test]
    #[ignore]
    async fn downloads_and_verifies_the_latest_release() {
        let log = std::sync::Mutex::new(Vec::new());
        let v = install(None, |m| log.lock().unwrap().push(m.to_string())).await.expect("install");
        assert!(managed_binary(&v).is_file(), "binary at {}", managed_binary(&v).display());
        let log = log.into_inner().unwrap();
        assert!(log.iter().any(|l| l.contains("SHA256SUMS")), "verification happened: {:?}", log);
        let r = resolve(None).expect("resolves");
        assert_eq!(r.source, BinarySource::Managed);
        assert_eq!(r.version.as_deref(), Some(v.as_str()));
        let out = std::process::Command::new(&r.path).arg("version").output().expect("runs");
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(text.contains(&v), "tofu version says {} — got {}", v, text);
    }

    #[test]
    fn extracts_tofu_from_a_zip_and_nothing_else() {
        let mut buf = Vec::new();
        {
            let mut w = zip::ZipWriter::new(Cursor::new(&mut buf));
            let o = zip::write::SimpleFileOptions::default();
            w.start_file("LICENSE", o).unwrap();
            std::io::Write::write_all(&mut w, b"mpl").unwrap();
            w.start_file(exe_name(), o).unwrap();
            std::io::Write::write_all(&mut w, b"BINARY").unwrap();
            w.finish().unwrap();
        }
        assert_eq!(extract_tofu(&buf, true).unwrap(), b"BINARY");
    }

    #[test]
    fn extracts_tofu_from_a_tarball() {
        let mut tarbuf = Vec::new();
        {
            let mut t = tar::Builder::new(&mut tarbuf);
            let data = b"BINARY";
            let mut h = tar::Header::new_gnu();
            h.set_size(data.len() as u64);
            h.set_mode(0o755);
            h.set_cksum();
            t.append_data(&mut h, exe_name(), &data[..]).unwrap();
            t.finish().unwrap();
        }
        let mut gz = Vec::new();
        {
            let mut e = flate2::write::GzEncoder::new(&mut gz, flate2::Compression::fast());
            std::io::Write::write_all(&mut e, &tarbuf).unwrap();
            e.finish().unwrap();
        }
        assert_eq!(extract_tofu(&gz, false).unwrap(), b"BINARY");
    }
}
