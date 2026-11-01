use serde::{Deserialize, Serialize};
use thiserror::Error;
use crate::ssh::client::{SharedHandle, exec_on_connection, exec_on_connection_with_exit_code, SshError};

#[derive(Debug, Error)]
pub enum SftpBrowserError {
    #[error("Not connected")]
    NotConnected,
    #[error("SSH error: {0}")]
    SshError(#[from] SshError),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("Path not found: {0}")]
    PathNotFound(String),
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    /// A remote command failed; what it said, e.g. "base64: /x: Permission denied".
    #[error("{0}")]
    Remote(String),
}

/// Metadata for a remote file or directory entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteEntry {
    pub name: String,
    pub path: String,
    #[serde(rename = "isDirectory")]
    pub is_dir: bool,
    pub size: u64,
    pub modified: u64,
    pub permissions: String,
}

/// List the contents of a remote directory via SSH exec.
pub async fn list_directory(
    handle: &SharedHandle,
    path: &str,
) -> Result<Vec<RemoteEntry>, SftpBrowserError> {
    // Use ls -lA --time-style=+%s to get machine-parseable output with timestamps
    let command = format!(
        "ls -lA --time-style=+%s {} 2>/dev/null || ls -lA {}",
        shell_escape(path),
        shell_escape(path)
    );
    let (stdout, stderr, code) = exec_on_connection_with_exit_code(handle, &command).await?;
    parse_ls_output(&listing_output(stdout, &stderr, code)?, path)
}

/// Create a directory on the remote host.
pub async fn make_directory(
    handle: &SharedHandle,
    path: &str,
) -> Result<(), SftpBrowserError> {
    let command = format!("mkdir -p {}", shell_escape(path));
    run_checked(handle, &command).await?;
    Ok(())
}

/// Delete a file or directory on the remote host.
pub async fn delete_entry(
    handle: &SharedHandle,
    path: &str,
) -> Result<(), SftpBrowserError> {
    let command = format!("rm -rf {}", shell_escape(path));
    run_checked(handle, &command).await?;
    Ok(())
}

/// Create an empty file on the remote host.
pub async fn touch_file(
    handle: &SharedHandle,
    path: &str,
) -> Result<(), SftpBrowserError> {
    let command = format!("touch {}", shell_escape(path));
    run_checked(handle, &command).await?;
    Ok(())
}

/// Rename or move a remote file or directory.
pub async fn rename_entry(
    handle: &SharedHandle,
    old_path: &str,
    new_path: &str,
) -> Result<(), SftpBrowserError> {
    let command = format!("mv {} {}", shell_escape(old_path), shell_escape(new_path));
    run_checked(handle, &command).await?;
    Ok(())
}

/// Maximum file size for text editing (5 MB).
const MAX_EDIT_SIZE: u64 = 5 * 1024 * 1024;

/// Read a text file's content from the remote host via base64 encoding.
pub async fn read_text_file(
    handle: &SharedHandle,
    path: &str,
) -> Result<String, SftpBrowserError> {
    // Check file size first
    let stat_cmd = format!("stat -c %s {} 2>/dev/null || stat -f %z {}", shell_escape(path), shell_escape(path));
    let size_output = exec_on_connection(handle, &stat_cmd).await?;
    let size: u64 = size_output
        .trim()
        .parse()
        .map_err(|_| SftpBrowserError::ParseError(format!("Cannot determine file size: {}", path)))?;

    if size > MAX_EDIT_SIZE {
        return Err(SftpBrowserError::ParseError(format!(
            "File too large to edit ({:.1} MB, max {:.0} MB)",
            size as f64 / (1024.0 * 1024.0),
            MAX_EDIT_SIZE as f64 / (1024.0 * 1024.0)
        )));
    }

    // Read file via base64 to handle binary-safe transport
    let cmd = format!("base64 {}", shell_escape(path));
    let (stdout, stderr, code) = exec_on_connection_with_exit_code(handle, &cmd).await?;
    let b64_output = command_output(stdout, &stderr, code)?;

    // Remove all whitespace from base64 output (line breaks etc.)
    let b64_clean: String = b64_output.chars().filter(|c| !c.is_whitespace()).collect();

    if b64_clean.is_empty() {
        return Ok(String::new());
    }

    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&b64_clean)
        .map_err(|e| SftpBrowserError::ParseError(format!("Base64 decode failed: {}", e)))?;

    String::from_utf8(bytes).map_err(|_| {
        SftpBrowserError::ParseError("File is not valid UTF-8 text".to_string())
    })
}

/// What `ls` listed. It also fails for an entry it could not stat (exit 1,
/// the rest still listed), so only a failure that listed nothing is an
/// error: a folder the login user cannot read, which showed as empty (#150).
fn listing_output(stdout: String, stderr: &str, code: i32) -> Result<String, SftpBrowserError> {
    let listed = stdout.lines().any(|l| !l.trim().is_empty() && !l.starts_with("total "));
    if listed { Ok(stdout) } else { command_output(stdout, stderr, code) }
}

/// Run a remote command; its stdout, or an error with what it said.
///
/// The commands here (mkdir, rm, touch, mv) print nothing when they work,
/// and when they fail only stderr and the exit status say so: a delete the
/// login user may not do reported success, and the file came back on the
/// next refresh (#150).
async fn run_checked(handle: &SharedHandle, command: &str) -> Result<String, SftpBrowserError> {
    let (stdout, stderr, code) = exec_on_connection_with_exit_code(handle, command).await?;
    command_output(stdout, &stderr, code)
}

/// A remote command's stdout, if it succeeded. A file the login user cannot
/// read gives an empty stdout and a reason on stderr; taken as output, it
/// opened as an empty file, and saving that would have emptied the real one
/// (#150). `-1` is no exit status at all: the channel closed or timed out.
fn command_output(stdout: String, stderr: &str, code: i32) -> Result<String, SftpBrowserError> {
    if code == 0 {
        return Ok(stdout);
    }
    let said = stderr.trim();
    Err(SftpBrowserError::Remote(match (code, said.is_empty()) {
        (-1, _) => "The connection closed before the server answered".to_string(),
        (_, false) => said.to_string(),
        (_, true) => format!("The server reported a failure (exit status {code})"),
    }))
}

/// Write text content to a remote file via streaming base64 over a single SSH channel.
pub async fn write_text_file(
    handle: &SharedHandle,
    path: &str,
    content: &str,
) -> Result<(), SftpBrowserError> {
    use base64::Engine;
    use russh::ChannelMsg;

    let data = content.as_bytes();

    // Empty file: simple truncate
    if data.is_empty() {
        let cmd = format!(": > {}", shell_escape(path));
        run_checked(handle, &cmd).await?;
        return Ok(());
    }

    // Open a single channel: pipe base64 stdin into decoder, write to file
    let mut channel = {
        let guard = handle.lock().await;
        guard.channel_open_session().await
            .map_err(|e| SshError::ChannelError(format!("{}", e)))?
    };
    channel.exec(true, format!("base64 -d > {}", shell_escape(path))).await
        .map_err(|e| SshError::ChannelError(format!("{}", e)))?;

    // Stream base64 in 48KB raw chunks (multiple of 3 → clean base64, no mid-stream padding)
    let chunk_size: usize = 48 * 1024;

    for chunk in data.chunks(chunk_size) {
        let mut b64 = base64::engine::general_purpose::STANDARD.encode(chunk);
        b64.push('\n');

        channel.data(b64.as_bytes()).await
            .map_err(|e| SftpBrowserError::ParseError(format!("Channel write error: {}", e)))?;
    }

    // Close stdin to signal EOF
    channel.eof().await
        .map_err(|e| SftpBrowserError::ParseError(format!("EOF signal error: {}", e)))?;

    // Wait for exit
    let mut got_eof = false;
    let mut got_exit = false;
    let mut exit_code: Option<u32> = None;
    let mut stderr_buf = String::new();

    loop {
        let msg = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            channel.wait(),
        ).await;

        match msg {
            Ok(Some(ChannelMsg::ExtendedData { ref data, .. })) => {
                stderr_buf.push_str(&String::from_utf8_lossy(data));
            }
            Ok(Some(ChannelMsg::Eof)) => {
                got_eof = true;
                if got_exit { break; }
            }
            Ok(Some(ChannelMsg::ExitStatus { exit_status })) => {
                exit_code = Some(exit_status);
                got_exit = true;
                if got_eof { break; }
            }
            Ok(None) | Err(_) => break,
            _ => {}
        }
    }

    if let Some(code) = exit_code {
        if code != 0 {
            let msg = stderr_buf.trim();
            if msg.to_lowercase().contains("permission denied") {
                return Err(SftpBrowserError::PermissionDenied(path.to_string()));
            }
            return Err(SftpBrowserError::ParseError(
                if msg.is_empty() { format!("Write failed with exit code {}", code) } else { msg.to_string() }
            ));
        }
    }

    Ok(())
}

fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn parse_ls_output(output: &str, base_path: &str) -> Result<Vec<RemoteEntry>, SftpBrowserError> {
    let mut entries = Vec::new();
    let base = if base_path.ends_with('/') {
        base_path.to_string()
    } else {
        format!("{}/", base_path)
    };

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("total") {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 7 {
            continue;
        }

        let permissions = parts[0].to_string();
        // Skip non-entry lines (permissions must start with d, -, l, c, b, p, s)
        if !permissions.starts_with(|c: char| "d-lcbps".contains(c)) {
            continue;
        }
        let is_dir = permissions.starts_with('d');
        let size: u64 = parts[4].parse().unwrap_or(0);

        // Detect format: --time-style=+%s gives epoch in parts[5], standard ls gives month
        let (modified, name) = if parts[5].parse::<u64>().is_ok() {
            // Epoch format: perms links owner group size epoch name...
            // parts: [0]=perms [1]=links [2]=owner [3]=group [4]=size [5]=epoch [6..]=name
            let ts = parts[5].parse::<u64>().unwrap_or(0);
            let name = parts[6..].join(" ");
            (ts, name)
        } else {
            // Standard format: perms links owner group size month day time/year name...
            // parts: [0]=perms [1]=links [2]=owner [3]=group [4]=size [5]=month [6]=day [7]=time [8..]=name
            if parts.len() < 9 {
                // Might be a short format, take last field as name
                let name = parts[parts.len() - 1].to_string();
                (0u64, name)
            } else {
                let name = parts[8..].join(" ");
                (0u64, name)
            }
        };

        if name == "." || name == ".." || name.is_empty() {
            continue;
        }

        // Handle symlinks: name -> target
        let clean_name = if let Some(idx) = name.find(" -> ") {
            name[..idx].to_string()
        } else {
            name
        };

        entries.push(RemoteEntry {
            path: format!("{}{}", base, clean_name),
            name: clean_name,
            is_dir,
            size,
            modified,
            permissions,
        });
    }

    // Sort: directories first, then by name
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(entries)
}

#[cfg(test)]
mod command_output_tests {
    use super::*;

    #[test]
    fn a_command_that_succeeded_gives_its_output() {
        assert_eq!(command_output("aGkK\n".into(), "", 0).unwrap(), "aGkK\n");
        // An empty file is a success with nothing on stdout.
        assert_eq!(command_output(String::new(), "", 0).unwrap(), "");
    }

    /// #150: a file the login user cannot read opened as an empty file.
    #[test]
    fn a_file_the_server_cannot_read_is_an_error_saying_why() {
        let err = command_output(String::new(), "base64: /root/x: Permission denied\n", 1).unwrap_err();
        assert_eq!(err.to_string(), "base64: /root/x: Permission denied");
    }

    #[test]
    fn a_failure_with_nothing_on_stderr_still_fails() {
        let err = command_output(String::new(), "", 1).unwrap_err();
        assert_eq!(err.to_string(), "The server reported a failure (exit status 1)");
    }

    #[test]
    fn a_folder_the_server_cannot_list_is_an_error_saying_why() {
        let err = listing_output(String::new(), "ls: cannot open directory '/root': Permission denied
", 2).unwrap_err();
        assert_eq!(err.to_string(), "ls: cannot open directory '/root': Permission denied");
    }

    #[test]
    fn a_folder_listed_in_part_still_lists() {
        let out = "total 4
-rw-r--r-- 1 a a 3 1700000000 ok.txt
".to_string();
        let listed = listing_output(out.clone(), "ls: cannot access 'gone': No such file or directory
", 1).unwrap();
        assert_eq!(listed, out);
    }

    #[test]
    fn an_empty_folder_is_not_an_error() {
        assert_eq!(listing_output("total 0
".into(), "", 0).unwrap(), "total 0
");
    }

    #[test]
    fn no_exit_status_is_a_file_cut_short() {
        let err = command_output("aGVsbG8g".into(), "", -1).unwrap_err();
        assert_eq!(err.to_string(), "The connection closed before the server answered");
    }
}
