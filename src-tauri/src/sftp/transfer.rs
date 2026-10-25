use std::path::{Path, PathBuf};
use thiserror::Error;
use tauri::Emitter;
use serde::{Deserialize, Serialize};
use crate::ssh::client::{SharedHandle, SshError, exec_on_connection, exec_on_connection_with_exit_code};
use base64::Engine;

#[derive(Debug, Error)]
pub enum TransferError {
    #[error("Not connected")]
    NotConnected,
    #[error("SSH error: {0}")]
    SshError(#[from] SshError),
    #[error("File not found: {0}")]
    FileNotFound(String),
    #[error("IO error: {0}")]
    IoError(String),
    #[error("Transfer cancelled")]
    Cancelled,
}

/// Progress information for an active file transfer.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferProgress {
    pub id: String,
    pub filename: String,
    pub bytes_transferred: u64,
    pub total_bytes: u64,
    pub percent: f64,
}

/// Download a file from the remote host using streaming base64 over a single SSH exec.
///
/// Runs `base64 <file>` once, streams the channel output, decodes line-by-line
/// and writes to the local file incrementally. Progress events are emitted as
/// data arrives — no per-chunk SSH roundtrips.
pub async fn download_file(
    handle: &SharedHandle,
    remote_path: &str,
    local_path: &str,
    transfer_id: &str,
    app_handle: &tauri::AppHandle,
) -> Result<(), TransferError> {
    use russh::ChannelMsg;
    use std::io::Write;

    tracing::info!("Downloading {} to {}", remote_path, local_path);

    let filename = Path::new(remote_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| remote_path.to_string());

    // Get file size first (try GNU stat, then BSD stat)
    let size_output = exec_on_connection(
        handle,
        &format!(
            "stat -c%s {} 2>/dev/null || stat -f%z {} 2>/dev/null",
            shell_escape(remote_path),
            shell_escape(remote_path)
        ),
    )
    .await?;
    let total_bytes: u64 = size_output.trim().parse().unwrap_or(0);

    if total_bytes == 0 {
        // Check if the file exists but is empty, or doesn't exist
        let exists_check = exec_on_connection(
            handle,
            &format!("test -f {} && echo EXISTS", shell_escape(remote_path)),
        )
        .await?;
        if !exists_check.trim().contains("EXISTS") {
            return Err(TransferError::FileNotFound(remote_path.to_string()));
        }
        // File exists but is empty -- write an empty file
        std::fs::write(local_path, b"")
            .map_err(|e| TransferError::IoError(format!("Failed to write local file: {}", e)))?;

        let _ = app_handle.emit(&format!("transfer-complete-{}", transfer_id), ());
        tracing::info!("Download complete: {} (empty file)", remote_path);
        return Ok(());
    }

    // Open a dedicated channel for streaming the base64 output
    let mut channel = {
        let guard = handle.lock().await;
        guard.channel_open_session().await
            .map_err(|e| SshError::ChannelError(format!("{}", e)))?
    };
    channel.exec(true, format!("base64 {}", shell_escape(remote_path))).await
        .map_err(|e| SshError::ChannelError(format!("{}", e)))?;

    // Create/truncate the local file. The parent has to exist first: a
    // download aimed at a path that has not been made yet would otherwise
    // fail with a bare "No such file or directory", which says nothing about
    // which part was missing.
    if let Some(parent) = std::path::Path::new(local_path).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| {
                TransferError::IoError(format!("Failed to create {}: {}", parent.display(), e))
            })?;
        }
    }
    let mut file = std::fs::File::create(local_path)
        .map_err(|e| TransferError::IoError(format!("Failed to create local file: {}", e)))?;

    let mut b64_buffer = String::new();
    let mut bytes_written: u64 = 0;
    let mut last_progress_bytes: u64 = 0;
    let mut got_eof = false;
    let mut got_exit = false;

    // Emit initial progress
    let _ = app_handle.emit(
        &format!("transfer-progress-{}", transfer_id),
        &TransferProgress {
            id: transfer_id.to_string(),
            filename: filename.clone(),
            bytes_transferred: 0,
            total_bytes,
            percent: 0.0,
        },
    );

    loop {
        let msg = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            channel.wait(),
        ).await;

        match msg {
            Ok(Some(ChannelMsg::Data { ref data })) => {
                b64_buffer.push_str(&String::from_utf8_lossy(data));

                // Process complete base64 lines (76 chars each = 57 raw bytes)
                while let Some(newline_pos) = b64_buffer.find('\n') {
                    let line: String = b64_buffer[..newline_pos]
                        .chars()
                        .filter(|c| !c.is_whitespace())
                        .collect();
                    b64_buffer = b64_buffer[newline_pos + 1..].to_string();

                    if line.is_empty() {
                        continue;
                    }

                    let decoded = base64::engine::general_purpose::STANDARD
                        .decode(&line)
                        .map_err(|e| TransferError::IoError(format!("Base64 decode error: {}", e)))?;

                    file.write_all(&decoded)
                        .map_err(|e| TransferError::IoError(format!("Write error: {}", e)))?;

                    bytes_written += decoded.len() as u64;
                }

                // Emit progress every ~64KB of decoded data
                if bytes_written - last_progress_bytes >= 65536 {
                    last_progress_bytes = bytes_written;
                    let percent = (bytes_written as f64 / total_bytes as f64 * 100.0).min(100.0);
                    let _ = app_handle.emit(
                        &format!("transfer-progress-{}", transfer_id),
                        &TransferProgress {
                            id: transfer_id.to_string(),
                            filename: filename.clone(),
                            bytes_transferred: bytes_written,
                            total_bytes,
                            percent,
                        },
                    );
                }
            }
            Ok(Some(ChannelMsg::ExtendedData { .. })) => {
                // stderr — ignore
            }
            Ok(Some(ChannelMsg::Eof)) => {
                got_eof = true;
                if got_exit { break; }
            }
            Ok(Some(ChannelMsg::ExitStatus { .. })) => {
                got_exit = true;
                if got_eof { break; }
            }
            Ok(None) | Err(_) => break,
            _ => {}
        }
    }

    // Decode any remaining data in the buffer (last partial line)
    let remaining: String = b64_buffer.chars().filter(|c| !c.is_whitespace()).collect();
    if !remaining.is_empty() {
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(&remaining)
            .map_err(|e| TransferError::IoError(format!("Base64 decode error (tail): {}", e)))?;
        file.write_all(&decoded)
            .map_err(|e| TransferError::IoError(format!("Write error: {}", e)))?;
        bytes_written += decoded.len() as u64;
    }

    file.flush()
        .map_err(|e| TransferError::IoError(format!("Flush error: {}", e)))?;

    // Final progress + completion
    let _ = app_handle.emit(
        &format!("transfer-progress-{}", transfer_id),
        &TransferProgress {
            id: transfer_id.to_string(),
            filename: filename.clone(),
            bytes_transferred: bytes_written,
            total_bytes,
            percent: 100.0,
        },
    );
    let _ = app_handle.emit(&format!("transfer-complete-{}", transfer_id), ());

    tracing::info!("Download complete: {} ({} bytes)", remote_path, bytes_written);
    Ok(())
}

/// Upload a local file or directory to the remote host.
///
/// A directory is walked here, recreated on the remote side with `mkdir -p`,
/// and its files sent one at a time the same way a single file is. It all
/// reports under the one transfer id, counting bytes across the whole tree,
/// so one dropped folder shows as one transfer.
pub async fn upload_path(
    handle: &SharedHandle,
    local_path: &str,
    remote_path: &str,
    transfer_id: &str,
    app_handle: &tauri::AppHandle,
) -> Result<(), TransferError> {
    if Path::new(local_path).is_dir() {
        upload_directory(handle, local_path, remote_path, transfer_id, app_handle).await
    } else {
        upload_file(handle, local_path, remote_path, transfer_id, app_handle).await
    }
}

/// Upload a file to the remote host using streaming base64 over a single SSH exec.
pub async fn upload_file(
    handle: &SharedHandle,
    local_path: &str,
    remote_path: &str,
    transfer_id: &str,
    app_handle: &tauri::AppHandle,
) -> Result<(), TransferError> {
    tracing::info!("Uploading {} to {}", local_path, remote_path);

    let filename = file_name_of(local_path);
    let data = read_local(Path::new(local_path))?;
    let total_bytes = data.len() as u64;
    let emit = |sent: u64| emit_progress(app_handle, transfer_id, &filename, sent, total_bytes);

    emit(0);
    send_bytes(handle, &data, remote_path, emit).await?;
    emit(total_bytes);
    let _ = app_handle.emit(&format!("transfer-complete-{}", transfer_id), ());

    tracing::info!("Upload complete: {} ({} bytes)", remote_path, total_bytes);
    Ok(())
}

async fn upload_directory(
    handle: &SharedHandle,
    local_path: &str,
    remote_path: &str,
    transfer_id: &str,
    app_handle: &tauri::AppHandle,
) -> Result<(), TransferError> {
    tracing::info!("Uploading directory {} to {}", local_path, remote_path);

    let name = file_name_of(local_path);
    let plan = plan_directory(Path::new(local_path), remote_path)
        .map_err(|e| TransferError::IoError(format!("Failed to read local directory: {}", e)))?;
    let total_bytes = plan.total_bytes;
    let emit = |sent: u64| emit_progress(app_handle, transfer_id, &name, sent, total_bytes);

    emit(0);
    for command in mkdir_commands(&plan.dirs) {
        let (_, stderr, code) = exec_on_connection_with_exit_code(handle, &command).await?;
        if code != 0 {
            let msg = if stderr.trim().is_empty() {
                format!("Remote mkdir exited with code {}", code)
            } else {
                stderr.trim().to_string()
            };
            return Err(TransferError::IoError(msg));
        }
    }

    let mut done: u64 = 0;
    for file in &plan.files {
        let data = read_local(&file.local)?;
        send_bytes(handle, &data, &file.remote, |sent| emit(done + sent)).await?;
        done += data.len() as u64;
    }

    emit(total_bytes);
    let _ = app_handle.emit(&format!("transfer-complete-{}", transfer_id), ());

    tracing::info!(
        "Upload complete: {} ({} files, {} bytes)",
        remote_path,
        plan.files.len(),
        done
    );
    Ok(())
}

/// What uploading a local directory takes: the remote directories to create,
/// parents before children, and every file with the place it goes.
#[derive(Debug, Default)]
struct DirectoryPlan {
    dirs: Vec<String>,
    files: Vec<PlannedFile>,
    total_bytes: u64,
}

#[derive(Debug)]
struct PlannedFile {
    local: PathBuf,
    remote: String,
}

/// Walk `local` and map it onto `remote`.
///
/// Remote paths are joined with `/` whatever the local separator is. A
/// symbolic link to a file is uploaded as the file it points to; a link to a
/// directory is left out, since following it could walk in a circle, and so
/// is a link that points nowhere. Entries are taken in name order.
fn plan_directory(local: &Path, remote: &str) -> std::io::Result<DirectoryPlan> {
    let mut plan = DirectoryPlan::default();
    let mut pending = vec![(local.to_path_buf(), remote.to_string())];

    while let Some((dir, remote_dir)) = pending.pop() {
        let mut entries = std::fs::read_dir(&dir)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(|e| e.file_name());

        let mut subdirs = Vec::new();
        for entry in entries {
            let path = entry.path();
            let remote_child = format!(
                "{}/{}",
                remote_dir.trim_end_matches('/'),
                entry.file_name().to_string_lossy()
            );
            let file_type = entry.file_type()?;
            if file_type.is_dir() {
                subdirs.push((path, remote_child));
                continue;
            }
            match std::fs::metadata(&path) {
                Ok(meta) if meta.is_file() => {
                    plan.total_bytes += meta.len();
                    plan.files.push(PlannedFile { local: path, remote: remote_child });
                }
                Ok(_) => {}
                Err(_) if file_type.is_symlink() => {}
                Err(e) => return Err(e),
            }
        }

        plan.dirs.push(remote_dir);
        // Reversed, so the stack hands them back in name order.
        pending.extend(subdirs.into_iter().rev());
    }

    Ok(plan)
}

/// `mkdir -p` commands for `dirs`, many directories to a command but none
/// longer than 16KB, which the small shells on embedded devices still take.
fn mkdir_commands(dirs: &[String]) -> Vec<String> {
    const LIMIT: usize = 16 * 1024;
    let mut commands = Vec::new();
    let mut current = String::new();

    for dir in dirs {
        let arg = shell_escape(dir);
        if !current.is_empty() && current.len() + 1 + arg.len() > LIMIT {
            commands.push(std::mem::take(&mut current));
        }
        if current.is_empty() {
            current.push_str("mkdir -p --");
        }
        current.push(' ');
        current.push_str(&arg);
    }
    if !current.is_empty() {
        commands.push(current);
    }
    commands
}

fn file_name_of(local_path: &str) -> String {
    Path::new(local_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| local_path.to_string())
}

fn read_local(path: &Path) -> Result<Vec<u8>, TransferError> {
    std::fs::read(path).map_err(|e| {
        TransferError::IoError(format!("Failed to read local file {}: {}", path.display(), e))
    })
}

fn emit_progress(
    app_handle: &tauri::AppHandle,
    transfer_id: &str,
    filename: &str,
    bytes_transferred: u64,
    total_bytes: u64,
) {
    let percent = if total_bytes == 0 {
        100.0
    } else {
        (bytes_transferred as f64 / total_bytes as f64 * 100.0).min(100.0)
    };
    let _ = app_handle.emit(
        &format!("transfer-progress-{}", transfer_id),
        &TransferProgress {
            id: transfer_id.to_string(),
            filename: filename.to_string(),
            bytes_transferred,
            total_bytes,
            percent,
        },
    );
}

/// Write `data` to `remote_path` through one `base64 -d` channel.
///
/// Streams base64-encoded data into the channel's stdin and closes it. No
/// per-chunk SSH roundtrips — mirrors the download approach for maximum
/// throughput. `on_progress` hears how many bytes of `data` have gone out,
/// about every 64KB.
async fn send_bytes(
    handle: &SharedHandle,
    data: &[u8],
    remote_path: &str,
    mut on_progress: impl FnMut(u64),
) -> Result<(), TransferError> {
    use russh::ChannelMsg;

    // Handle empty files
    if data.is_empty() {
        let _ = exec_on_connection(
            handle,
            &format!(": > {}", shell_escape(remote_path)),
        ).await?;
        return Ok(());
    }

    let total_bytes = data.len() as u64;

    // Open a single channel: pipe base64 stdin into decoder, write to file
    let mut channel = {
        let guard = handle.lock().await;
        guard.channel_open_session().await
            .map_err(|e| SshError::ChannelError(format!("{}", e)))?
    };
    channel.exec(true, format!("base64 -d > {}", shell_escape(remote_path))).await
        .map_err(|e| SshError::ChannelError(format!("{}", e)))?;

    // Stream base64-encoded data in chunks through the channel's stdin.
    // 48KB raw → 64KB base64 (multiple of 3 avoids padding mid-stream).
    let chunk_size: usize = 48 * 1024;
    let mut bytes_sent: u64 = 0;
    let mut last_progress_bytes: u64 = 0;

    for chunk in data.chunks(chunk_size) {
        let mut b64 = base64::engine::general_purpose::STANDARD.encode(chunk);
        b64.push('\n');

        channel.data(b64.as_bytes()).await
            .map_err(|e| TransferError::IoError(format!("Channel write error: {}", e)))?;

        bytes_sent += chunk.len() as u64;

        // Emit progress every ~64KB of raw data
        if bytes_sent - last_progress_bytes >= 65536 || bytes_sent == total_bytes {
            last_progress_bytes = bytes_sent;
            on_progress(bytes_sent);
        }
    }

    // Close stdin to signal EOF to base64 -d
    channel.eof().await
        .map_err(|e| TransferError::IoError(format!("EOF signal error: {}", e)))?;

    // Wait for the remote command to finish
    let mut got_eof = false;
    let mut got_exit = false;
    let mut exit_code: Option<u32> = None;
    let mut stderr_buf = String::new();

    loop {
        let msg = tokio::time::timeout(
            std::time::Duration::from_secs(30),
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

    // Check for errors
    if let Some(code) = exit_code {
        if code != 0 {
            let msg = if stderr_buf.trim().is_empty() {
                format!("Remote base64 -d exited with code {}", code)
            } else {
                stderr_buf.trim().to_string()
            };
            return Err(TransferError::IoError(msg));
        }
    }

    Ok(())
}

/// Escape a string for safe use in a shell command using single quotes.
fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Read a remote file straight into a channel, without touching local disk.
///
/// Used by the promise drag: the drop target pulls bytes through an `IStream`
/// and writes them wherever the user dropped, so there is no local copy to
/// make and nothing should exist until the drop happens.
///
/// The channel is bounded, which is what applies back-pressure: if the
/// receiver stops reading, this stops reading from the network rather than
/// buffering a whole file in memory. Sending fails once the receiver is gone,
/// which is how a cancelled drop stops the transfer.
pub async fn stream_file(
    handle: &SharedHandle,
    remote_path: &str,
    sink: std::sync::mpsc::SyncSender<Vec<u8>>,
) -> Result<u64, TransferError> {
    use russh::ChannelMsg;

    let mut channel = {
        let guard = handle.lock().await;
        guard
            .channel_open_session()
            .await
            .map_err(|e| TransferError::SshError(crate::ssh::client::SshError::ChannelError(format!("{}", e))))?
    };
    channel
        .exec(true, format!("base64 {}", shell_escape(remote_path)))
        .await
        .map_err(|e| TransferError::SshError(crate::ssh::client::SshError::ChannelError(format!("{}", e))))?;

    let mut b64 = String::new();
    let mut sent: u64 = 0;
    let mut got_eof = false;
    let mut got_exit = false;

    let mut emit = |bytes: Vec<u8>| -> Result<(), TransferError> {
        sent += bytes.len() as u64;
        sink.send(bytes)
            .map_err(|_| TransferError::IoError("drop cancelled".into()))
    };

    loop {
        let msg = tokio::time::timeout(std::time::Duration::from_secs(30), channel.wait()).await;
        match msg {
            Ok(Some(ChannelMsg::Data { ref data })) => {
                b64.push_str(&String::from_utf8_lossy(data));
                while let Some(nl) = b64.find('\n') {
                    let line: String = b64[..nl].chars().filter(|c| !c.is_whitespace()).collect();
                    b64 = b64[nl + 1..].to_string();
                    if line.is_empty() {
                        continue;
                    }
                    let decoded = base64::engine::general_purpose::STANDARD
                        .decode(&line)
                        .map_err(|e| TransferError::IoError(format!("Base64 decode error: {}", e)))?;
                    emit(decoded)?;
                }
            }
            Ok(Some(ChannelMsg::Eof)) => {
                got_eof = true;
                if got_exit {
                    break;
                }
            }
            Ok(Some(ChannelMsg::ExitStatus { .. })) => {
                got_exit = true;
                if got_eof {
                    break;
                }
            }
            Ok(None) | Err(_) => break,
            _ => {}
        }
    }

    let tail: String = b64.chars().filter(|c| !c.is_whitespace()).collect();
    if !tail.is_empty() {
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(&tail)
            .map_err(|e| TransferError::IoError(format!("Base64 decode error (tail): {}", e)))?;
        emit(decoded)?;
    }

    Ok(sent)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("reach-upload-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_directory_maps_onto_the_remote_side_parents_first() {
        let root = scratch("tree");
        std::fs::create_dir_all(root.join("b/deep")).unwrap();
        std::fs::create_dir_all(root.join("empty")).unwrap();
        std::fs::write(root.join("z.txt"), b"12345").unwrap();
        std::fs::write(root.join("a.txt"), b"").unwrap();
        std::fs::write(root.join("b/deep/c.bin"), b"abc").unwrap();

        let plan = plan_directory(&root, "/home/me/site").unwrap();

        assert_eq!(
            plan.dirs,
            ["/home/me/site", "/home/me/site/b", "/home/me/site/b/deep", "/home/me/site/empty"]
        );
        let remote: Vec<&str> = plan.files.iter().map(|f| f.remote.as_str()).collect();
        assert_eq!(remote, ["/home/me/site/a.txt", "/home/me/site/z.txt", "/home/me/site/b/deep/c.bin"]);
        assert_eq!(plan.files[2].local, root.join("b").join("deep").join("c.bin"));
        assert_eq!(plan.total_bytes, 8);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_trailing_slash_on_the_target_does_not_double() {
        let root = scratch("slash");
        std::fs::write(root.join("f"), b"x").unwrap();

        let plan = plan_directory(&root, "/srv/").unwrap();

        assert_eq!(plan.files[0].remote, "/srv/f");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn mkdir_quotes_each_directory() {
        let dirs = vec!["/tmp/it's here".to_string(), "/tmp/-x".to_string()];
        assert_eq!(mkdir_commands(&dirs), [r"mkdir -p -- '/tmp/it'\''s here' '/tmp/-x'"]);
        assert!(mkdir_commands(&[]).is_empty());
    }

    #[test]
    fn mkdir_splits_long_lists() {
        let dirs: Vec<String> = (0..2000).map(|i| format!("/data/some/fairly/long/path/{i:05}")).collect();
        let commands = mkdir_commands(&dirs);

        assert!(commands.len() > 1);
        assert!(commands.iter().all(|c| c.len() <= 16 * 1024 && c.starts_with("mkdir -p -- ")));
        let count: usize = commands.iter().map(|c| c.matches("'/data/").count()).sum();
        assert_eq!(count, dirs.len());
    }
}
