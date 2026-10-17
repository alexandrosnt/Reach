//! wsl.exe, run the same way everywhere Reach needs it: no console window,
//! no stdin, a deadline, and its output decoded whichever encoding it used.

use std::process::Stdio;
use std::time::Duration;

/// Listing distributions can start the WSL service first.
pub const LIST_WITHIN: Duration = Duration::from_secs(15);
/// A command in a stopped distribution boots it first.
pub const RUN_WITHIN: Duration = Duration::from_secs(20);

/// Distributions Docker Desktop installs for itself: there is no shell in
/// them for a person to use.
const NOT_FOR_PEOPLE: [&str; 2] = ["docker-desktop", "docker-desktop-data"];

/// wsl.exe's own messages, `-l` among them, are UTF-16LE; a Linux program's
/// output passes through as it was written, UTF-8. Line ends become `\n`.
pub fn decode(bytes: &[u8]) -> String {
    let utf16 = bytes.starts_with(&[0xFF, 0xFE])
        || (bytes.len() >= 2 && bytes.iter().skip(1).step_by(2).filter(|&&b| b == 0).count() * 2 >= bytes.len() / 2);
    let text = if utf16 {
        let units: Vec<u16> = bytes.as_chunks::<2>().0.iter().map(|&pair| u16::from_le_bytes(pair)).collect();
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    };
    text.trim_start_matches('\u{feff}').replace("\r\n", "\n")
}

/// The names `wsl.exe -l -q` printed, without blanks or Docker's own.
pub fn distros(listing: &str) -> Vec<String> {
    listing
        .lines()
        .map(|l| l.trim().trim_matches('\0'))
        .filter(|l| !l.is_empty() && !NOT_FOR_PEOPLE.iter().any(|d| l.eq_ignore_ascii_case(d)))
        .map(str::to_string)
        .collect()
}

/// Runs wsl.exe with `args` and returns its decoded stdout, or why not.
/// Past `within` the process is killed rather than left behind.
pub async fn run(args: &[&str], within: Duration) -> Result<String, String> {
    let mut cmd = tokio::process::Command::new("wsl.exe");
    cmd.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000);
    let output = tokio::time::timeout(within, cmd.output())
        .await
        .map_err(|_| format!("wsl.exe did not answer within {} seconds", within.as_secs()))?
        .map_err(|e| format!("Failed to execute wsl.exe: {e}"))?;
    if !output.status.success() {
        let why = decode(&output.stderr);
        let why = why.trim();
        return Err(if why.is_empty() { format!("wsl.exe exited with {}", output.status) } else { why.to_string() });
    }
    Ok(decode(&output.stdout))
}

#[cfg(test)]
#[path = "wsl_tests.rs"]
mod tests;
