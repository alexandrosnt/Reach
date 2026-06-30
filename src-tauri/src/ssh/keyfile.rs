//! Inspect SSH key files so the UI (and connect-time errors) can tell a user
//! when they've selected the *wrong* file — most commonly an OpenSSH **public**
//! key where a **private** key is required — and suggest the right key from the
//! same folder.
//!
//! Classification is content-based (not extension-based): we read a bounded
//! prefix of the file and look at the PEM/OpenSSH markers. This is both more
//! reliable than trusting the name and lets us surface the algorithm + comment.

use serde::Serialize;
use std::path::{Path, PathBuf};

use crate::ssh::client::expand_tilde;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyFileKind {
    /// A usable private key (PEM / PKCS#8 / OpenSSH container).
    PrivateKey,
    /// An OpenSSH public key (`ssh-ed25519 AAAA... comment`) — not a secret.
    PublicKey,
    /// The path exists but isn't a recognizable key.
    NotAKey,
    /// No file at the given path.
    NotFound,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyCandidate {
    /// Absolute path on disk.
    pub path: String,
    /// File name only (for display).
    pub name: String,
    /// Detected algorithm, when known (`ed25519`, `rsa`, `ecdsa`, `dsa`).
    pub algo: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyFileInfo {
    /// The expanded path that was inspected.
    pub path: String,
    pub kind: KeyFileKind,
    /// Algorithm for the inspected file, when detectable.
    pub algo: Option<String>,
    /// Trailing comment on a public key, when present.
    pub comment: Option<String>,
    /// Best-effort: whether a private key is passphrase-protected.
    pub encrypted: bool,
    /// When `kind == PublicKey`: the matching private key (path without `.pub`),
    /// if it exists on disk and really is a private key.
    pub suggested_private_key: Option<KeyCandidate>,
    /// Other private keys found in the same directory (deduped against the
    /// suggestion and the inspected file).
    pub sibling_private_keys: Vec<KeyCandidate>,
}

/// OpenSSH public-key line prefixes mapped to a friendly algorithm name.
const PUBLIC_KEY_TYPES: &[(&str, &str)] = &[
    ("ssh-ed25519", "ed25519"),
    ("ssh-rsa", "rsa"),
    ("ssh-dss", "dsa"),
    ("ecdsa-sha2-nistp256", "ecdsa"),
    ("ecdsa-sha2-nistp384", "ecdsa"),
    ("ecdsa-sha2-nistp521", "ecdsa"),
    ("sk-ssh-ed25519@openssh.com", "ed25519-sk"),
    ("sk-ecdsa-sha2-nistp256@openssh.com", "ecdsa-sk"),
];

/// Inspect the key file at `raw` (which may contain a leading `~`).
pub fn classify_path(raw: &str) -> KeyFileInfo {
    let expanded = expand_tilde(raw);
    let path_str = expanded.to_string_lossy().into_owned();

    if !expanded.exists() {
        // Even when missing, surface private keys from the parent directory so
        // the UI can still offer a correction (e.g. a typo'd file name).
        let siblings = expanded
            .parent()
            .map(|d| scan_dir_for_private_keys(d, None))
            .unwrap_or_default();
        return KeyFileInfo {
            path: path_str,
            kind: KeyFileKind::NotFound,
            algo: None,
            comment: None,
            encrypted: false,
            suggested_private_key: None,
            sibling_private_keys: siblings,
        };
    }

    // Read a bounded prefix — key files are tiny; never slurp a huge file that
    // was selected by mistake.
    let content = read_head(&expanded, 64 * 1024).unwrap_or_default();
    let (kind, algo, comment, encrypted) = sniff(&content);

    let suggested_private_key = if kind == KeyFileKind::PublicKey {
        suggest_private_for_public(&expanded)
    } else {
        None
    };

    let suggested_path = suggested_private_key.as_ref().map(|c| PathBuf::from(&c.path));
    let sibling_private_keys = expanded
        .parent()
        .map(|d| {
            let mut v = scan_dir_for_private_keys(d, Some(&expanded));
            if let Some(sp) = &suggested_path {
                v.retain(|c| Path::new(&c.path) != sp.as_path());
            }
            v
        })
        .unwrap_or_default();

    KeyFileInfo {
        path: path_str,
        kind,
        algo,
        comment,
        encrypted,
        suggested_private_key,
        sibling_private_keys,
    }
}

fn read_head(path: &Path, max: u64) -> std::io::Result<String> {
    use std::io::Read;
    let f = std::fs::File::open(path)?;
    let mut buf = Vec::new();
    f.take(max).read_to_end(&mut buf)?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

/// Classify file *content*. Returns `(kind, algo, comment, encrypted)`.
fn sniff(content: &str) -> (KeyFileKind, Option<String>, Option<String>, bool) {
    let trimmed = content.trim_start();
    let first_line = trimmed.lines().next().unwrap_or("").trim();

    // --- Private key (PEM / PKCS#8 / OpenSSH) ---
    if first_line.starts_with("-----BEGIN ") && first_line.contains("PRIVATE KEY") {
        let algo = if first_line.contains("RSA") {
            Some("rsa".to_string())
        } else if first_line.contains("EC PRIVATE") {
            Some("ecdsa".to_string())
        } else if first_line.contains("DSA") {
            Some("dsa".to_string())
        } else {
            // OpenSSH / PKCS#8 containers hide the algorithm.
            None
        };
        return (KeyFileKind::PrivateKey, algo, None, detect_encrypted(trimmed));
    }

    // --- OpenSSH public key (single line: "<type> <base64> [comment]") ---
    let mut parts = first_line.splitn(3, char::is_whitespace);
    let typ = parts.next().unwrap_or("");
    for (prefix, algo) in PUBLIC_KEY_TYPES {
        if typ == *prefix {
            let b64 = parts.next().unwrap_or("");
            if b64.is_empty() {
                break;
            }
            let comment = parts
                .next()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            return (KeyFileKind::PublicKey, Some((*algo).to_string()), comment, false);
        }
    }

    (KeyFileKind::NotAKey, None, None, false)
}

/// Best-effort detection of whether a private key is passphrase-protected.
fn detect_encrypted(content: &str) -> bool {
    // Classic PEM (RSA/EC/DSA) and PKCS#8 markers.
    if content.contains("Proc-Type: 4,ENCRYPTED")
        || content.contains("DEK-Info:")
        || content.contains("ENCRYPTED PRIVATE KEY")
    {
        return true;
    }
    // OpenSSH new format: decode the container and read the cipher name.
    if content.contains("BEGIN OPENSSH PRIVATE KEY") {
        if let Some(cipher) = openssh_cipher_name(content) {
            return cipher != "none";
        }
    }
    false
}

/// Parse the cipher name out of an `openssh-key-v1` container.
fn openssh_cipher_name(content: &str) -> Option<String> {
    use base64::Engine;
    let body: String = content
        .lines()
        .skip_while(|l| !l.contains("BEGIN OPENSSH PRIVATE KEY"))
        .skip(1)
        .take_while(|l| !l.contains("END OPENSSH PRIVATE KEY"))
        .collect();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(body.trim())
        .ok()?;
    const MAGIC: &[u8] = b"openssh-key-v1\0";
    let rest = bytes.strip_prefix(MAGIC)?;
    let len = u32::from_be_bytes([*rest.first()?, *rest.get(1)?, *rest.get(2)?, *rest.get(3)?])
        as usize;
    let name = rest.get(4..4 + len)?;
    Some(String::from_utf8_lossy(name).into_owned())
}

/// For a `*.pub` path, return the matching private key (path without `.pub`)
/// when it exists and really is a private key.
fn suggest_private_for_public(pub_path: &Path) -> Option<KeyCandidate> {
    let s = pub_path.to_string_lossy();
    let stripped = s.strip_suffix(".pub")?;
    let priv_path = PathBuf::from(stripped);
    if !priv_path.is_file() {
        return None;
    }
    let content = read_head(&priv_path, 8 * 1024).unwrap_or_default();
    let (kind, algo, _, _) = sniff(&content);
    if kind == KeyFileKind::PrivateKey {
        Some(KeyCandidate {
            path: priv_path.to_string_lossy().into_owned(),
            name: file_name(&priv_path),
            algo,
        })
    } else {
        None
    }
}

/// Scan a directory for files whose content sniffs as a private key.
fn scan_dir_for_private_keys(dir: &Path, exclude: Option<&Path>) -> Vec<KeyCandidate> {
    let mut out: Vec<KeyCandidate> = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return out;
    };
    let mut scanned = 0;
    for entry in rd.flatten() {
        if scanned >= 200 || out.len() >= 12 {
            break;
        }
        let path = entry.path();
        if !path.is_file() || Some(path.as_path()) == exclude {
            continue;
        }
        let name = file_name(&path);
        if should_skip(&name) {
            continue;
        }
        scanned += 1;
        let content = read_head(&path, 4 * 1024).unwrap_or_default();
        let (kind, algo, _, _) = sniff(&content);
        if kind == KeyFileKind::PrivateKey {
            out.push(KeyCandidate {
                path: path.to_string_lossy().into_owned(),
                name,
                algo,
            });
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Skip files that are obviously not loadable private keys.
fn should_skip(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".pub")
        || lower.ends_with(".ppk") // PuTTY format, not russh-loadable
        || lower.ends_with(".crt")
        || lower.ends_with(".csr")
        || lower.ends_with(".pem.pub")
        || lower == "config"
        || lower == "authorized_keys"
        || lower.starts_with("known_hosts")
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_ed25519_public_key_with_comment() {
        let (kind, algo, comment, enc) =
            sniff("ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIabc123 user@host\n");
        assert_eq!(kind, KeyFileKind::PublicKey);
        assert_eq!(algo.as_deref(), Some("ed25519"));
        assert_eq!(comment.as_deref(), Some("user@host"));
        assert!(!enc);
    }

    #[test]
    fn detects_rsa_public_key_without_comment() {
        let (kind, algo, comment, _) = sniff("ssh-rsa AAAAB3NzaC1yc2EAAAADAQAB");
        assert_eq!(kind, KeyFileKind::PublicKey);
        assert_eq!(algo.as_deref(), Some("rsa"));
        assert_eq!(comment, None);
    }

    #[test]
    fn detects_ecdsa_public_key() {
        let (kind, algo, ..) = sniff("ecdsa-sha2-nistp256 AAAAE2VjZHNh base\n");
        assert_eq!(kind, KeyFileKind::PublicKey);
        assert_eq!(algo.as_deref(), Some("ecdsa"));
    }

    #[test]
    fn detects_openssh_private_key() {
        let (kind, ..) =
            sniff("-----BEGIN OPENSSH PRIVATE KEY-----\nb3Blbn...\n-----END OPENSSH PRIVATE KEY-----\n");
        assert_eq!(kind, KeyFileKind::PrivateKey);
    }

    #[test]
    fn detects_rsa_pem_private_key() {
        let (kind, algo, _, _) =
            sniff("-----BEGIN RSA PRIVATE KEY-----\nMIIE...\n-----END RSA PRIVATE KEY-----\n");
        assert_eq!(kind, KeyFileKind::PrivateKey);
        assert_eq!(algo.as_deref(), Some("rsa"));
    }

    #[test]
    fn detects_encrypted_classic_pem() {
        let (kind, _, _, enc) = sniff(
            "-----BEGIN RSA PRIVATE KEY-----\nProc-Type: 4,ENCRYPTED\nDEK-Info: AES-128-CBC,AB\n\nMIIE...\n-----END RSA PRIVATE KEY-----\n",
        );
        assert_eq!(kind, KeyFileKind::PrivateKey);
        assert!(enc);
    }

    #[test]
    fn leading_whitespace_does_not_break_detection() {
        let (kind, algo, ..) = sniff("\n  ssh-ed25519 AAAAC3xyz comment\n");
        assert_eq!(kind, KeyFileKind::PublicKey);
        assert_eq!(algo.as_deref(), Some("ed25519"));
    }

    #[test]
    fn random_text_is_not_a_key() {
        let (kind, ..) = sniff("hello world\nthis is not a key at all");
        assert_eq!(kind, KeyFileKind::NotAKey);
    }

    #[test]
    fn bare_type_without_base64_is_not_public_key() {
        // A lone algorithm token with no key body must not be misread.
        let (kind, ..) = sniff("ssh-ed25519\n");
        assert_eq!(kind, KeyFileKind::NotAKey);
    }
}
