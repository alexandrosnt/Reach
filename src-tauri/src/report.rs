//! "Report a problem": a copy of reach.log fit to post in a public issue.
//!
//! The log is what a maintainer needs and what a user cannot be asked to
//! clean by hand: it names the hosts they connect to, the accounts they use
//! and the folders they keep things in. Those are taken out here, before the
//! user sees the copy, and the user still reads it before posting. GitHub
//! does not let an app attach a file to a new issue without the user's
//! token, which Reach does not hold, so the copy is saved next to the log and
//! its folder opened, to be dragged into the issue.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

/// The newest part of the log kept in the copy. A report needs the minutes
/// before the problem, not weeks of history.
const KEEP_BYTES: usize = 400 * 1024;
/// Warnings and errors quoted in the issue itself.
const QUOTED_LINES: usize = 15;

/// Something the user's own data says is private, and what it becomes.
#[derive(Debug, Deserialize)]
pub struct Private {
    pub text: String,
    pub label: String,
}

#[derive(Debug, Serialize)]
pub struct Report {
    /// The cleaned copy, saved next to reach.log.
    pub path: String,
    /// Its contents, for the user to read before posting.
    pub log: String,
    pub version: String,
    pub os: String,
    /// Desktop and display server on Linux, where most reports differ.
    pub desktop: Option<String>,
    /// The newest warnings and errors, cleaned.
    pub problems: Vec<String>,
}

static IPV4: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b(?:\d{1,3}\.){3}\d{1,3}\b").unwrap());
static IPV6: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)(?:[0-9a-f]{0,4}:){2,7}[0-9a-f]{0,4}").unwrap());
static AT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\w.+-]+@[\w-]+(?:\.[\w-]+)*").unwrap());

/// `log` with the private parts replaced: the user's home folder, the hosts,
/// accounts and names in `private`, then any address or user@host left.
pub fn redact(log: &str, home: Option<&str>, private: &[Private]) -> String {
    let mut out = log.to_string();
    if let Some(home) = home.filter(|h| h.len() > 3) {
        // As written, with either slash, and as `{:?}` writes a Windows path:
        // every backslash doubled. The doubled form goes first, as the single
        // one would match only its first half.
        let doubled = home.replace('/', "\\").replace('\\', "\\\\");
        for form in [doubled, home.to_string(), home.replace('\\', "/"), home.replace('/', "\\")] {
            if let Ok(re) = Regex::new(&format!("(?i){}", regex::escape(&form))) {
                out = re.replace_all(&out, "~").into_owned();
            }
        }
    }
    // Longest first, so a host is not half-replaced by a name inside it.
    let mut terms: Vec<&Private> = private.iter().filter(|p| p.text.trim().chars().count() >= 3).collect();
    terms.sort_by_key(|p| std::cmp::Reverse(p.text.len()));
    for p in terms {
        // Whole names only, so a user called "dev" leaves "devops" alone. A
        // name that starts or ends with a symbol has no word edge there.
        let text = p.text.trim();
        let edge = |c: Option<char>| if c.is_some_and(|c| c.is_alphanumeric() || c == '_') { r"\b" } else { "" };
        let pattern = format!("(?i){}{}{}", edge(text.chars().next()), regex::escape(text), edge(text.chars().last()));
        if let Ok(re) = Regex::new(&pattern) {
            out = re.replace_all(&out, format!("[{}]", p.label)).into_owned();
        }
    }
    out = AT.replace_all(&out, "[user]@[host]").into_owned();
    out = IPV4.replace_all(&out, "[ip]").into_owned();
    redact_ipv6(&out)
}

/// IPv6 addresses out of `text`. Times (12:34:56) and Rust paths
/// (reach_lib::ssh) look alike: an address has "::" or at least four colons,
/// and is not glued to a letter, digit or underscore on either side.
fn redact_ipv6(text: &str) -> String {
    let glued = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for m in IPV6.find_iter(text) {
        let s = m.as_str();
        let address = (s.contains("::") || s.matches(':').count() >= 4)
            && s.chars().any(|c| c.is_ascii_hexdigit())
            && !glued(text[..m.start()].chars().next_back())
            && !glued(text[m.end()..].chars().next());
        if address {
            out.push_str(&text[last..m.start()]);
            out.push_str("[ip]");
            last = m.end();
        }
    }
    out.push_str(&text[last..]);
    out
}

/// The end of `log`, at most `KEEP_BYTES`, starting on a whole line.
fn newest(log: &str) -> &str {
    if log.len() <= KEEP_BYTES {
        return log;
    }
    let mut start = log.len() - KEEP_BYTES;
    while !log.is_char_boundary(start) {
        start += 1;
    }
    match log[start..].find('\n') {
        Some(nl) => &log[start + nl + 1..],
        None => &log[start..],
    }
}

/// The newest warning and error lines, oldest first.
fn problems(log: &str) -> Vec<String> {
    let mut lines: Vec<String> = log
        .lines()
        .rev()
        .filter(|l| l.contains(" WARN ") || l.contains(" ERROR "))
        .take(QUOTED_LINES)
        .map(|l| l.chars().take(400).collect())
        .collect();
    lines.reverse();
    lines
}

fn desktop() -> Option<String> {
    if !cfg!(target_os = "linux") {
        return None;
    }
    let de = std::env::var("XDG_CURRENT_DESKTOP").ok().filter(|s| !s.is_empty());
    let session = std::env::var("XDG_SESSION_TYPE").ok().filter(|s| !s.is_empty());
    match (de, session) {
        (None, None) => None,
        (de, session) => Some(format!(
            "{} ({})",
            de.as_deref().unwrap_or("unknown desktop"),
            session.as_deref().unwrap_or("unknown session")
        )),
    }
}

/// Make the cleaned copy of reach.log and what the issue should say.
#[tauri::command]
pub async fn report_prepare(mut private: Vec<Private>) -> Result<Report, String> {
    // The account name turns up outside the home folder too, in paths of
    // other forms and in a shell's prompt.
    if let Some(me) = ["USERNAME", "USER", "LOGNAME"].iter().find_map(|v| std::env::var(v).ok().filter(|s| !s.is_empty())) {
        private.push(Private { text: me, label: "user".into() });
    }
    let dir = crate::app_data_dir().join("logs");
    let raw = std::fs::read(dir.join("reach.log")).unwrap_or_default();
    let raw = String::from_utf8_lossy(&raw);
    let home = dirs::home_dir().map(|h| h.to_string_lossy().into_owned());
    let log = redact(newest(&raw), home.as_deref(), &private);
    let path = dir.join("reach-report.log");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(&path, &log).map_err(|e| format!("Cannot write {}: {e}", path.display()))?;
    Ok(Report {
        path: path.to_string_lossy().into_owned(),
        problems: problems(&log),
        log,
        version: env!("CARGO_PKG_VERSION").to_string(),
        os: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
        desktop: desktop(),
    })
}

/// Show the cleaned copy in the file manager, selected, ready to be dragged
/// into the issue. Only that file: the page cannot name another.
#[tauri::command]
pub fn report_reveal() -> Result<(), String> {
    let path = crate::app_data_dir().join("logs").join("reach-report.log");
    tauri_plugin_opener::reveal_item_in_dir(&path).map_err(|e| e.to_string())
}

#[cfg(test)]
#[path = "report_tests.rs"]
mod tests;
