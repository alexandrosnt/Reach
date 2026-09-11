//! Where Ansible runs.
//!
//! Ansible is a Linux program: its control node needs `fork()`, so Windows
//! can never run it directly. What Reach can do is own *where* it runs and
//! make every choice explicit — a native install on Linux or macOS, WSL on
//! Windows (works, unofficial upstream), or a remote Linux host Reach is
//! already connected to. Each is an engine; the UI asks which engines are
//! green, never which OS this is.
//!
//! The remote engine is the one that is identical on every platform, and
//! it is the one that needs the most from Reach: the project has to get
//! there first. `sync_project` streams it over the existing SSH channel as
//! a tarball into a directory Reach owns on the far side.

use std::io::Write;
use std::path::Path;

use serde::Serialize;

use crate::ssh::client::{exec_on_connection, exec_with_stdin, SharedHandle};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EngineKind {
    Native,
    Wsl,
    Container,
    Remote,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineInfo {
    pub kind: EngineKind,
    pub available: bool,
    /// `ansible [core 2.18.1]`, as the tool reports it.
    pub version: Option<String>,
    /// Why it is not available, in a sentence a person can act on.
    pub reason: Option<String>,
    /// The upstream project does not stand behind this path (WSL).
    pub unofficial: bool,
    /// The distro for WSL, the connection for remote.
    pub detail: Option<String>,
}

fn first_line(s: &str) -> Option<String> {
    s.lines().map(str::trim).find(|l| !l.is_empty()).map(str::to_string)
}

fn silent(program: &str) -> std::process::Command {
    let mut cmd = std::process::Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    cmd
}

fn native() -> EngineInfo {
    if cfg!(windows) {
        return EngineInfo {
            kind: EngineKind::Native,
            available: false,
            version: None,
            reason: Some("Ansible cannot run on Windows as a control node — it needs fork(). Use WSL or a remote host.".into()),
            unofficial: false,
            detail: None,
        };
    }
    crate::toolchain::detect::ensure_ansible_in_path();
    match which::which("ansible") {
        Ok(bin) => {
            let version = silent(&bin.to_string_lossy())
                .arg("--version")
                .output()
                .ok()
                .filter(|o| o.status.success())
                .and_then(|o| first_line(&String::from_utf8_lossy(&o.stdout)));
            EngineInfo {
                kind: EngineKind::Native,
                available: version.is_some(),
                reason: version.is_none().then(|| "ansible is on PATH but `ansible --version` failed.".to_string()),
                version,
                unofficial: false,
                detail: Some(bin.to_string_lossy().to_string()),
            }
        }
        Err(_) => EngineInfo {
            kind: EngineKind::Native,
            available: false,
            version: None,
            reason: Some("Ansible is not installed. Install it with pipx, or use a remote host.".into()),
            unofficial: false,
            detail: None,
        },
    }
}

#[cfg(windows)]
fn wsl() -> Option<EngineInfo> {
    let list = silent("wsl.exe").args(["--list", "--quiet"]).output().ok()?;
    if !list.status.success() {
        return Some(EngineInfo {
            kind: EngineKind::Wsl,
            available: false,
            version: None,
            reason: Some("WSL is not installed. Install it with `wsl --install`, or use a remote host.".into()),
            unofficial: true,
            detail: None,
        });
    }
    // wsl.exe prints UTF-16; strip the NULs and take the default distro.
    let text: String = String::from_utf8_lossy(&list.stdout).chars().filter(|c| *c != '\0').collect();
    let distro = first_line(&text);
    if distro.is_none() {
        return Some(EngineInfo {
            kind: EngineKind::Wsl,
            available: false,
            version: None,
            reason: Some("WSL is installed but has no Linux distribution yet.".into()),
            unofficial: true,
            detail: None,
        });
    }
    let version = silent("wsl.exe")
        .args(["--", "bash", "-lc", "ansible --version 2>/dev/null | head -n1"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| first_line(&String::from_utf8_lossy(&o.stdout)));
    Some(EngineInfo {
        kind: EngineKind::Wsl,
        available: version.is_some(),
        reason: version.is_none().then(|| "Ansible is not installed inside WSL. Install it there with pipx.".to_string()),
        version,
        unofficial: true,
        detail: distro,
    })
}

#[cfg(not(windows))]
fn wsl() -> Option<EngineInfo> {
    None
}

/// The execution environment image: ansible-core, the community
/// collections, and the tooling, maintained by the Ansible project. The
/// one engine that is the same on Windows, macOS and Linux.
pub const EE_IMAGE: &str = "ghcr.io/ansible/community-ansible-dev-tools:latest";

/// `podman` first — rootless, daemonless, the runtime the Ansible tooling
/// itself prefers — then `docker`.
pub fn container_runtime() -> Option<String> {
    for name in ["podman", "docker"] {
        if which::which(name).is_ok() {
            return Some(name.to_string());
        }
    }
    None
}

fn container() -> EngineInfo {
    let Some(runtime) = container_runtime() else {
        return EngineInfo {
            kind: EngineKind::Container,
            available: false,
            version: None,
            reason: Some("No container runtime found. Install Podman Desktop or Docker Desktop to run Ansible in an execution environment.".into()),
            unofficial: false,
            detail: None,
        };
    };
    // `info` talks to the daemon (or the podman machine); a runtime that is
    // installed but not running fails here, quickly, with a message.
    let alive = silent(&runtime)
        .args(["info", "--format", "{{.ServerVersion}}{{.Version.Version}}"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    let version = silent(&runtime)
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| first_line(&String::from_utf8_lossy(&o.stdout)));
    EngineInfo {
        kind: EngineKind::Container,
        available: alive,
        reason: (!alive).then(|| format!("{} is installed but not running. Start it and pick this engine again.", runtime)),
        version,
        unofficial: false,
        detail: Some(format!("{} · {}", runtime, EE_IMAGE)),
    }
}

/// The engines this machine can offer on its own. Remote engines are per
/// connection and checked by `remote`.
pub fn detect_local_engines() -> Vec<EngineInfo> {
    let mut out = vec![native()];
    if let Some(w) = wsl() {
        out.push(w);
    }
    out.push(container());
    out
}

/// Whether an open SSH connection can act as a control node.
pub async fn remote(handle: &SharedHandle, label: &str) -> EngineInfo {
    let out = exec_on_connection(handle, "ansible --version 2>/dev/null | head -n1").await;
    let version = out.ok().and_then(|s| first_line(&s));
    EngineInfo {
        kind: EngineKind::Remote,
        available: version.is_some(),
        reason: version.is_none().then(|| format!("Ansible is not installed on {}.", label)),
        version,
        unofficial: false,
        detail: Some(label.to_string()),
    }
}

/// Where a project lives on a remote host. Under the home directory and
/// keyed by the project id, so two projects with the same name never share
/// a directory and nothing of the user's is ever overwritten.
pub fn remote_dir(project_id: &str) -> String {
    format!("$HOME/.reach/ansible/{}", project_id)
}

/// Directories that are never part of a project's payload.
const SKIP_DIRS: &[&str] = &[".git", ".venv", "venv", "__pycache__", "node_modules", ".reach-sync"];

/// A gzip'd tar of the project, in memory. Returns the bytes and the file
/// count. Projects are playbooks and roles — kilobytes, not gigabytes.
pub fn pack_project(dir: &Path) -> Result<(Vec<u8>, usize), String> {
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    let mut count = 0usize;
    {
        let mut tar = tar::Builder::new(&mut gz);
        tar.follow_symlinks(false);
        let mut stack = vec![dir.to_path_buf()];
        while let Some(d) = stack.pop() {
            let entries = std::fs::read_dir(&d).map_err(|e| format!("{}: {}", d.display(), e))?;
            let mut items: Vec<_> = entries.filter_map(|e| e.ok()).collect();
            items.sort_by_key(|e| e.file_name());
            for entry in items {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();
                let rel = path.strip_prefix(dir).map_err(|e| e.to_string())?;
                if path.is_dir() {
                    if SKIP_DIRS.contains(&name.as_str()) {
                        continue;
                    }
                    stack.push(path);
                } else if path.is_file() {
                    let mut f = std::fs::File::open(&path).map_err(|e| e.to_string())?;
                    // Paths inside the archive are always slash-separated.
                    let arch = rel.to_string_lossy().replace('\\', "/");
                    tar.append_file(&arch, &mut f).map_err(|e| e.to_string())?;
                    count += 1;
                }
            }
        }
        tar.finish().map_err(|e| e.to_string())?;
    }
    let bytes = gz.finish().map_err(|e| e.to_string())?;
    Ok((bytes, count))
}

/// Put the project on the remote host, replacing what was there. The
/// tarball travels as the remote command's stdin, so nothing is written
/// anywhere it could be read on the way.
pub async fn sync_project(
    handle: &SharedHandle,
    local_dir: &Path,
    project_id: &str,
) -> Result<(String, usize, usize), String> {
    let (bytes, files) = pack_project(local_dir)?;
    let dir = remote_dir(project_id);
    let cmd = format!(
        "umask 077 && mkdir -p {d} && find {d} -mindepth 1 -maxdepth 1 ! -name .vault-pass -exec rm -rf {{}} + && tar xzf - -C {d}",
        d = dir
    );
    let size = bytes.len();
    let (code, out) = exec_with_stdin(handle, &cmd, bytes).await.map_err(|e| e.to_string())?;
    if code != 0 {
        return Err(format!("Sync failed on the remote host (exit {}): {}", code, out.trim()));
    }
    Ok((dir, files, size))
}

/// Write a secret onto the remote host as a file only its owner can read.
pub async fn write_remote_secret(handle: &SharedHandle, path: &str, content: &str) -> Result<(), String> {
    let cmd = format!("umask 077 && cat > {} && chmod 600 {}", path, path);
    let mut body = content.as_bytes().to_vec();
    if !body.ends_with(b"\n") {
        body.push(b'\n');
    }
    let (code, out) = exec_with_stdin(handle, &cmd, body).await.map_err(|e| e.to_string())?;
    if code != 0 {
        return Err(format!("Could not write {} (exit {}): {}", path, code, out.trim()));
    }
    Ok(())
}

pub async fn remove_remote_file(handle: &SharedHandle, path: &str) {
    let _ = exec_on_connection(handle, &format!("rm -f {}", path)).await;
}

/// A secret as a file the current user alone can read, for
/// `--vault-password-file`. Removed by the caller when the run ends.
pub fn write_local_secret(content: &str) -> Result<std::path::PathBuf, String> {
    let dir = crate::app_data_dir().join("tmp");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("vault-{}.pass", uuid::Uuid::new_v4()));
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(&path).map_err(|e| e.to_string())?;
    f.write_all(content.as_bytes()).map_err(|e| e.to_string())?;
    f.write_all(b"\n").map_err(|e| e.to_string())?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn scratch(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("reach-engine-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn names_in(bytes: &[u8]) -> Vec<String> {
        let gz = flate2::read::GzDecoder::new(bytes);
        let mut tar = tar::Archive::new(gz);
        let mut out = Vec::new();
        for e in tar.entries().unwrap() {
            let mut e = e.unwrap();
            let mut s = String::new();
            e.read_to_string(&mut s).unwrap();
            out.push(e.path().unwrap().to_string_lossy().to_string());
        }
        out.sort();
        out
    }

    #[test]
    fn packs_the_project_and_leaves_the_junk() {
        let d = scratch("pack");
        std::fs::write(d.join("site.yml"), "- hosts: all\n").unwrap();
        std::fs::create_dir_all(d.join("roles").join("common").join("tasks")).unwrap();
        std::fs::write(d.join("roles/common/tasks/main.yml"), "[]\n").unwrap();
        std::fs::create_dir_all(d.join(".git")).unwrap();
        std::fs::write(d.join(".git/HEAD"), "ref\n").unwrap();
        std::fs::create_dir_all(d.join("__pycache__")).unwrap();
        std::fs::write(d.join("__pycache__/x.pyc"), "x").unwrap();
        let (bytes, count) = pack_project(&d).unwrap();
        assert_eq!(count, 2);
        assert_eq!(names_in(&bytes), vec!["roles/common/tasks/main.yml", "site.yml"]);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn remote_dir_is_keyed_by_project_id() {
        assert_eq!(remote_dir("abc"), "$HOME/.reach/ansible/abc");
    }

    #[test]
    fn local_secret_is_a_file_with_a_trailing_newline() {
        let p = write_local_secret("hunter2").unwrap();
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "hunter2\n");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&p).unwrap().permissions().mode() & 0o777, 0o600);
        }
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn native_engine_on_windows_says_why() {
        let e = native();
        if cfg!(windows) {
            assert!(!e.available);
            assert!(e.reason.as_deref().unwrap_or("").contains("fork"));
        }
        assert_eq!(e.kind, EngineKind::Native);
    }
}
