use std::path::Path;
use thiserror::Error;
use tauri::Emitter;
use serde::{Deserialize, Serialize};
use crate::ssh::client::{SharedHandle, SshError, exec_on_connection};
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

    // Create/truncate the local file
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

/// Upload a file to the remote host using streaming base64 over a single SSH exec.
///
/// Opens one channel running `base64 -d > <file>`, streams base64-encoded data
/// into stdin, then closes the channel. No per-chunk SSH roundtrips — mirrors
/// the download approach for maximum throughput.
pub async fn upload_file(
    handle: &SharedHandle,
    local_path: &str,
    remote_path: &str,
    transfer_id: &str,
    app_handle: &tauri::AppHandle,
) -> Result<(), TransferError> {
    use russh::ChannelMsg;

    tracing::info!("Uploading {} to {}", local_path, remote_path);

    let filename = Path::new(local_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| local_path.to_string());

    // Read local file
    let data = std::fs::read(local_path)
        .map_err(|e| TransferError::IoError(format!("Failed to read local file: {}", e)))?;

    let total_bytes = data.len() as u64;

    // Handle empty files
    if total_bytes == 0 {
        let _ = exec_on_connection(
            handle,
            &format!(": > {}", shell_escape(remote_path)),
        ).await?;
        let _ = app_handle.emit(&format!("transfer-complete-{}", transfer_id), ());
        tracing::info!("Upload complete: {} (empty file)", remote_path);
        return Ok(());
    }

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
            let percent = (bytes_sent as f64 / total_bytes as f64 * 100.0).min(100.0);
            let _ = app_handle.emit(
                &format!("transfer-progress-{}", transfer_id),
                &TransferProgress {
                    id: transfer_id.to_string(),
                    filename: filename.clone(),
                    bytes_transferred: bytes_sent,
                    total_bytes,
                    percent,
                },
            );
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

    // Final progress + completion
    let _ = app_handle.emit(
        &format!("transfer-progress-{}", transfer_id),
        &TransferProgress {
            id: transfer_id.to_string(),
            filename: filename.clone(),
            bytes_transferred: total_bytes,
            total_bytes,
            percent: 100.0,
        },
    );
    let _ = app_handle.emit(&format!("transfer-complete-{}", transfer_id), ());

    tracing::info!("Upload complete: {} ({} bytes)", remote_path, total_bytes);
    Ok(())
}

/// Escape a string for safe use in a shell command using single quotes.
fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}
