//! Log files inside a container or pod (issue #101).
//!
//! Many services write their real log to files and keep stdout for errors
//! only (glog's `/logs/<service>.INFO`, log4j appenders, nginx's access log).
//! Reach shows those files the way an engineer would look at them:
//! `tail -n N -F -- file…`, which keeps following through rotation and a
//! glog symlink moving to its next file, run through the engine's own exec
//! API as an argument list, never through a shell, so no path can become a
//! command. It only reads, so a read-only or Production host allows it.
//!
//! Where it cannot follow, it still shows something true: an image without
//! `tail` (distroless, scratch) or a stopped container gets the file as it is
//! now, through Docker's file copy, with a line saying why it is not live.
//! Finding the files is a fixed script, the same everywhere, listing what the
//! container's processes hold open and what lies in the usual log folders.

use serde::{Deserialize, Serialize};

/// At most this many files are followed together.
pub const MAX_FILES: usize = 8;
/// A snapshot reads at most this much of a file.
pub const SNAPSHOT_LIMIT: usize = 16 * 1024 * 1024;

/// Marks a line as Reach's own note, not the log's: the viewer shows it
/// apart and leaves it out of a download. A control character, so no log
/// line can start with it by accident (the viewer strips them from logs).
pub const NOTE: char = '\u{1}';

/// `text` as a note line for the log viewer.
pub fn note(text: &str) -> String {
    format!("\r\n{NOTE}{text}\r\n")
}

/// Log data, with the note mark taken out: what a container prints can never
/// pass for Reach's own words.
pub fn data(text: String) -> String {
    if text.contains(NOTE) {
        text.replace(NOTE, "")
    } else {
        text
    }
}

/// The paths a user asked for, checked: absolute, printable, not too many.
pub fn check_paths(paths: &[String]) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    for p in paths {
        let p = p.trim();
        if p.is_empty() {
            continue;
        }
        if !p.starts_with('/') {
            return Err(format!("{p}: give the full path inside the container, starting with /"));
        }
        if p.len() > 4096 || p.chars().any(|c| c.is_control()) {
            return Err("That path cannot be a file name".into());
        }
        if !out.iter().any(|o| o == p) {
            out.push(p.to_string());
        }
    }
    if out.is_empty() {
        return Err("Choose at least one file".into());
    }
    if out.len() > MAX_FILES {
        return Err(format!("Follow at most {MAX_FILES} files at once"));
    }
    Ok(out)
}

/// `tail -n <lines> -F -- <paths>`: the newest lines, then follow by name,
/// so rotation and a moved symlink are followed. GNU and BusyBox both take it.
pub fn tail_command(paths: &[String], lines: u32) -> Vec<String> {
    let mut cmd = vec!["tail".to_string(), "-n".to_string(), lines.to_string(), "-F".to_string(), "--".to_string()];
    cmd.extend(paths.iter().cloned());
    cmd
}

/// Lists log files: those a process holds open, then those in the usual
/// folders. One line each: `<kind> <size> <mtime> <path>`, kind `o` (open),
/// `f` (file) or `l` (symlink, size and time of its target). POSIX sh, GNU
/// and BusyBox `find` and `stat`; nothing in it comes from the user.
pub const DISCOVER_SCRIPT: &str = r#"for p in /proc/[0-9]*/fd/*; do t=$(readlink "$p" 2>/dev/null) || continue; case "$t" in /dev/*|/proc/*|/sys/*|socket:*|pipe:*|anon_inode:*|*' (deleted)') continue;; esac; [ -f "$t" ] && stat -c 'o %s %Y %n' "$t" 2>/dev/null; done | head -n 200
for d in /logs /log /var/log /app/log /app/logs /opt /srv /data/log /data/logs /home /tmp; do [ -d "$d" ] || continue; find "$d" -maxdepth 3 -type f \( -name '*.log' -o -name '*.log.*' -o -name '*.INFO' -o -name '*.WARN*' -o -name '*.ERROR' -o -name '*.FATAL' -o -name '*.out' -o -name '*.err' \) -exec stat -c 'f %s %Y %n' {} + 2>/dev/null; find "$d" -maxdepth 3 -type l \( -name '*.log' -o -name '*.INFO' -o -name '*.WARN*' -o -name '*.ERROR' -o -name '*.FATAL' \) -exec stat -L -c 'l %s %Y %n' {} + 2>/dev/null; done | head -n 400"#;

/// A log file found in a container.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FoundFile {
    pub path: String,
    pub size: u64,
    /// Seconds since the epoch.
    pub modified: i64,
    /// A process has it open: it is being written to.
    pub open: bool,
    /// A symlink, as glog's `<service>.INFO`: its name stays, its file moves.
    pub link: bool,
}

/// [`DISCOVER_SCRIPT`]'s output, one entry per path: written-to first, then
/// the newest.
pub fn parse_discovery(out: &str) -> Vec<FoundFile> {
    let mut files: Vec<FoundFile> = Vec::new();
    for line in out.lines() {
        let mut it = line.splitn(4, ' ');
        let (Some(kind), Some(size), Some(mtime), Some(path)) = (it.next(), it.next(), it.next(), it.next()) else { continue };
        let (Ok(size), Ok(modified)) = (size.parse::<u64>(), mtime.parse::<i64>()) else { continue };
        if !path.starts_with('/') {
            continue;
        }
        match files.iter_mut().find(|f| f.path == path) {
            Some(f) => {
                f.open |= kind == "o";
                f.link |= kind == "l";
            }
            None => files.push(FoundFile { path: path.to_string(), size, modified, open: kind == "o", link: kind == "l" }),
        }
    }
    files.sort_by(|a, b| b.open.cmp(&a.open).then(b.modified.cmp(&a.modified)).then(a.path.cmp(&b.path)));
    files.truncate(200);
    files
}

/// Why following did not work, read from what the engine or `tail` said.
#[derive(Debug, PartialEq)]
pub enum Refusal {
    /// The image has no `tail` (or no shell, for the listing).
    NoTool,
    /// The container is stopped or paused.
    NotRunning,
}

pub fn refusal(message: &str) -> Option<Refusal> {
    let m = message.to_lowercase();
    if m.contains("is not running") || m.contains("is paused") || m.contains("container not running") {
        return Some(Refusal::NotRunning);
    }
    if m.contains("executable file not found") || (m.contains("no such file or directory") && m.contains("exec")) {
        return Some(Refusal::NoTool);
    }
    None
}

/// The last `lines` lines of `bytes`, as text.
pub fn last_lines(bytes: &[u8], lines: usize) -> String {
    let text = String::from_utf8_lossy(bytes);
    let trimmed = text.trim_end_matches('\n');
    let mut start = trimmed.len();
    for _ in 0..lines {
        match trimmed[..start].rfind('\n') {
            Some(i) => start = i,
            None => {
                start = 0;
                break;
            }
        }
    }
    let body = trimmed[start..].trim_start_matches('\n');
    if body.is_empty() {
        String::new()
    } else {
        format!("{body}\n")
    }
}

/// What a file-copy archive of one path holds.
#[derive(Debug, PartialEq)]
pub enum Archived {
    File(Vec<u8>),
    /// A symlink; its target, to fetch next.
    Link(String),
    Missing,
}

/// The single entry of the tar Docker returns for one path.
pub fn read_archive(tar_bytes: &[u8]) -> Result<Archived, String> {
    let mut ar = tar::Archive::new(tar_bytes);
    for entry in ar.entries().map_err(|e| e.to_string())? {
        let mut entry = entry.map_err(|e| e.to_string())?;
        let kind = entry.header().entry_type();
        if kind.is_symlink() {
            let target = entry.link_name().map_err(|e| e.to_string())?.map(|p| p.to_string_lossy().into_owned());
            return Ok(target.map(Archived::Link).unwrap_or(Archived::Missing));
        }
        if kind.is_file() {
            let mut buf = Vec::new();
            use std::io::Read;
            (&mut entry).take(SNAPSHOT_LIMIT as u64).read_to_end(&mut buf).map_err(|e| e.to_string())?;
            return Ok(Archived::File(buf));
        }
        if kind.is_dir() {
            return Err("That is a folder, not a file".into());
        }
    }
    Ok(Archived::Missing)
}

/// A symlink's target as a full path: relative targets are next to the link.
pub fn resolve_link(link: &str, target: &str) -> String {
    if target.starts_with('/') {
        return target.to_string();
    }
    let dir = link.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
    format!("{dir}/{target}")
}

/// Which containers a saved set of files is for.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Scope {
    /// One Compose service, whatever its containers are called this time.
    #[serde(rename_all = "camelCase")]
    Service { project: String, service: String },
    /// A container by its name.
    Container { name: String },
    /// A Kubernetes container name in a namespace, across the pods it runs in.
    #[serde(rename_all = "camelCase")]
    PodContainer { namespace: String, container: String },
    /// Every container on the host; paths usually use `{service}`.
    Host,
}

/// Files saved for a scope, with an optional name for the source picker.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LogRule {
    pub scope: Scope,
    pub paths: Vec<String>,
    #[serde(default)]
    pub label: Option<String>,
}

#[cfg(test)]
#[path = "logfiles_tests.rs"]
mod tests;
