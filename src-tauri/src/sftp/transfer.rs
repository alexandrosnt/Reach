use std::path::{Path, PathBuf};
use thiserror::Error;
use tauri::Emitter;
use serde::{Deserialize, Serialize};
use crate::ssh::client::{SharedHandle, SshError, exec_on_connection, exec_on_connection_with_exit_code};
use base64::Engine;
pub use tokio_util::sync::CancellationToken;

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
    /// The remote command failed; what it said, e.g. "base64: /x: Permission denied".
    #[error("{0}")]
    Remote(String),
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
    cancel: &CancellationToken,
) -> Result<(), TransferError> {
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

    // Initial progress
    let progress = |bytes: u64, percent: f64| {
        let _ = app_handle.emit(
            &format!("transfer-progress-{}", transfer_id),
            &TransferProgress {
                id: transfer_id.to_string(),
                filename: filename.clone(),
                bytes_transferred: bytes,
                total_bytes,
                percent,
            },
        );
    };
    progress(0, 0.0);

    let received = tokio::select! {
        r = receive_base64(&mut channel, &mut file, DOWNLOAD_IDLE_LIMIT, |bytes| {
            progress(bytes, (bytes as f64 / total_bytes as f64 * 100.0).min(100.0));
        }) => r,
        _ = cancel.cancelled() => Err(TransferError::Cancelled),
    };
    if matches!(received, Err(TransferError::Cancelled)) {
        let _ = channel.close().await;
    }
    let received = received.and_then(|bytes| {
        file.flush().map_err(|e| TransferError::IoError(format!("Flush error: {}", e)))?;
        Ok(bytes)
    });
    let bytes_written = match received {
        Ok(bytes) => bytes,
        Err(e) => {
            // What arrived is not the file, and a file of the right name
            // left behind looks like a finished download (#150).
            drop(file);
            let _ = std::fs::remove_file(local_path);
            tracing::warn!("Download of {} failed: {}", remote_path, e);
            return Err(e);
        }
    };

    progress(bytes_written, 100.0);
    let _ = app_handle.emit(&format!("transfer-complete-{}", transfer_id), ());

    tracing::info!("Download complete: {} ({} bytes)", remote_path, bytes_written);
    Ok(())
}

/// How long a download may go without hearing from the server.
const DOWNLOAD_IDLE_LIMIT: std::time::Duration = std::time::Duration::from_secs(30);

/// The messages of a remote command's channel, one at a time. A trait so
/// that [`receive_base64`] can be tested without a server.
pub(crate) trait ChannelMessages {
    async fn next_message(&mut self) -> Option<russh::ChannelMsg>;
}

impl ChannelMessages for russh::Channel<russh::client::Msg> {
    async fn next_message(&mut self) -> Option<russh::ChannelMsg> {
        self.wait().await
    }
}

/// Read the output of a remote `base64 <file>`, decode it into `out`, and
/// return how many bytes it was.
///
/// The file is only what was sent if the command said so: exit status 0.
/// A command that could not read the file (no permission, gone, a
/// directory) sends nothing on stdout and says why on stderr; taking that
/// as an empty file left an empty download and no error (#150). So any
/// other status is an error carrying what the server said, and so is a
/// channel that closes without a status or goes quiet for `idle`: both are
/// a file cut short.
pub(crate) async fn receive_base64<C: ChannelMessages>(
    channel: &mut C,
    out: &mut impl std::io::Write,
    idle: std::time::Duration,
    mut progress: impl FnMut(u64),
) -> Result<u64, TransferError> {
    use russh::ChannelMsg;

    /// Enough of stderr to say what went wrong.
    const STDERR_LIMIT: usize = 4096;

    let mut b64_buffer = String::new();
    let mut stderr = String::new();
    let mut bytes_written: u64 = 0;
    let mut last_progress_bytes: u64 = 0;
    let mut got_eof = false;
    let mut exit: Option<Result<(), String>> = None;

    let mut write_line = |line: &str, bytes_written: &mut u64| -> Result<(), TransferError> {
        let line: String = line.chars().filter(|c| !c.is_whitespace()).collect();
        if line.is_empty() {
            return Ok(());
        }
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(&line)
            .map_err(|e| TransferError::IoError(format!("Base64 decode error: {}", e)))?;
        out.write_all(&decoded)
            .map_err(|e| TransferError::IoError(format!("Write error: {}", e)))?;
        *bytes_written += decoded.len() as u64;
        Ok(())
    };

    loop {
        let msg = tokio::time::timeout(idle, channel.next_message()).await.map_err(|_| {
            TransferError::IoError(format!(
                "The server sent nothing for {} seconds; the download stopped",
                idle.as_secs()
            ))
        })?;
        match msg {
            Some(ChannelMsg::Data { ref data }) => {
                b64_buffer.push_str(&String::from_utf8_lossy(data));
                // Complete base64 lines (76 chars each = 57 raw bytes)
                while let Some(newline_pos) = b64_buffer.find('\n') {
                    write_line(&b64_buffer[..newline_pos], &mut bytes_written)?;
                    b64_buffer.drain(..=newline_pos);
                }
                // Progress every ~64KB of decoded data
                if bytes_written - last_progress_bytes >= 65536 {
                    last_progress_bytes = bytes_written;
                    progress(bytes_written);
                }
            }
            Some(ChannelMsg::ExtendedData { ref data, .. }) => {
                if stderr.len() < STDERR_LIMIT {
                    stderr.push_str(&String::from_utf8_lossy(data));
                }
            }
            Some(ChannelMsg::Eof) => {
                got_eof = true;
                if exit.is_some() {
                    break;
                }
            }
            Some(ChannelMsg::ExitStatus { exit_status }) => {
                exit = Some(if exit_status == 0 { Ok(()) } else { Err(format!("exit status {exit_status}")) });
                if got_eof {
                    break;
                }
            }
            Some(ChannelMsg::ExitSignal { signal_name, .. }) => {
                exit = Some(Err(format!("stopped by signal {signal_name:?}")));
                if got_eof {
                    break;
                }
            }
            // Closed: whatever status was coming has come or never will.
            Some(ChannelMsg::Close) | None => break,
            Some(_) => {}
        }
    }

    match exit {
        Some(Ok(())) => {}
        Some(Err(status)) => {
            let said = stderr.trim();
            return Err(TransferError::Remote(if said.is_empty() {
                format!("The server could not send the file ({status})")
            } else {
                said.to_string()
            }));
        }
        None => {
            return Err(TransferError::IoError(
                "The connection closed before the whole file arrived".to_string(),
            ))
        }
    }

    // The last partial line
    write_line(&b64_buffer, &mut bytes_written)?;
    Ok(bytes_written)
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
    cancel: &CancellationToken,
) -> Result<(), TransferError> {
    if Path::new(local_path).is_dir() {
        upload_directory(handle, local_path, remote_path, transfer_id, app_handle, cancel).await
    } else {
        upload_file(handle, local_path, remote_path, transfer_id, app_handle, cancel).await
    }
}

/// Upload a file to the remote host using streaming base64 over a single SSH exec.
pub async fn upload_file(
    handle: &SharedHandle,
    local_path: &str,
    remote_path: &str,
    transfer_id: &str,
    app_handle: &tauri::AppHandle,
    cancel: &CancellationToken,
) -> Result<(), TransferError> {
    tracing::info!("Uploading {} to {}", local_path, remote_path);

    let filename = file_name_of(local_path);
    let data = read_local(Path::new(local_path))?;
    let total_bytes = data.len() as u64;
    let emit = |sent: u64| emit_progress(app_handle, transfer_id, &filename, sent, total_bytes);

    emit(0);
    send_bytes(handle, &data, remote_path, cancel, emit).await?;
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
    cancel: &CancellationToken,
) -> Result<(), TransferError> {
    tracing::info!("Uploading directory {} to {}", local_path, remote_path);

    let name = file_name_of(local_path);
    let plan = plan_directory(Path::new(local_path), remote_path)
        .map_err(|e| TransferError::IoError(format!("Failed to read local directory: {}", e)))?;
    let total_bytes = plan.total_bytes;
    let emit = |sent: u64| emit_progress(app_handle, transfer_id, &name, sent, total_bytes);

    emit(0);
    for command in mkdir_commands(&plan.dirs) {
        if cancel.is_cancelled() {
            return Err(TransferError::Cancelled);
        }
        let (_, stderr, code) = exec_on_connection_with_exit_code(handle, &command).await?;
        remote_outcome(i64::from(code), &stderr)?;
    }

    let mut done: u64 = 0;
    for file in &plan.files {
        let data = read_local(&file.local)?;
        send_bytes(handle, &data, &file.remote, cancel, |sent| emit(done + sent)).await?;
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
///
/// The server's answer is read while the data goes out, not after. A shell
/// that cannot create the target (a folder the user may not write, #161)
/// exits at once and reads nothing more; written blind, the upload filled
/// the channel's window and then waited for room that never came. Now its
/// exit and its message end the upload there and then.
///
/// `cancel` stops the upload: the channel is closed and the partial file
/// removed, since `>` has already emptied whatever was there.
async fn send_bytes(
    handle: &SharedHandle,
    data: &[u8],
    remote_path: &str,
    cancel: &CancellationToken,
    mut on_progress: impl FnMut(u64),
) -> Result<(), TransferError> {
    use russh::ChannelMsg;

    if data.is_empty() {
        let command = format!(": > {}", shell_escape(remote_path));
        let (_, stderr, code) = exec_on_connection_with_exit_code(handle, &command).await?;
        return remote_outcome(i64::from(code), &stderr);
    }

    let total_bytes = data.len() as u64;

    let channel = {
        let guard = handle.lock().await;
        guard.channel_open_session().await
            .map_err(|e| SshError::ChannelError(format!("{}", e)))?
    };
    channel.exec(true, format!("base64 -d > {}", shell_escape(remote_path))).await
        .map_err(|e| SshError::ChannelError(format!("{}", e)))?;
    let (mut rx, tx) = channel.split();

    let mut stderr_buf = String::new();
    let mut exit_code: Option<u32> = None;
    let mut sent = false;

    let outcome: Result<(), TransferError> = {
        // 48KB raw → 64KB base64 (a multiple of 3 keeps padding out of the
        // middle of the stream).
        let send = async {
            let mut bytes_sent: u64 = 0;
            let mut last_progress_bytes: u64 = 0;
            for chunk in data.chunks(48 * 1024) {
                let mut b64 = base64::engine::general_purpose::STANDARD.encode(chunk);
                b64.push('\n');
                tx.data_bytes(b64.into_bytes()).await
                    .map_err(|e| TransferError::IoError(format!("Channel write error: {}", e)))?;
                bytes_sent += chunk.len() as u64;
                if bytes_sent - last_progress_bytes >= 65536 || bytes_sent == total_bytes {
                    last_progress_bytes = bytes_sent;
                    on_progress(bytes_sent);
                }
            }
            // Close stdin so base64 -d finishes.
            tx.eof().await
                .map_err(|e| TransferError::IoError(format!("EOF signal error: {}", e)))
        };
        tokio::pin!(send);
        let mut got_eof = false;

        loop {
            // A failed command reads nothing more, so sending stops; its
            // message may still be on the way (sshd can report the exit
            // before the last of stderr), so the replies are read to the end.
            let failed = exit_code.is_some_and(|c| c != 0);
            tokio::select! {
                _ = cancel.cancelled() => break Err(TransferError::Cancelled),
                result = &mut send, if !sent && !failed => {
                    if let Err(e) = result {
                        break Err(e);
                    }
                    sent = true;
                    if got_eof && exit_code.is_some() {
                        break Ok(());
                    }
                }
                msg = next_reply(&mut rx, sent || failed) => match msg {
                    Some(ChannelMsg::ExtendedData { data, .. }) => {
                        stderr_buf.push_str(&String::from_utf8_lossy(&data));
                    }
                    Some(ChannelMsg::Eof) => {
                        got_eof = true;
                        if (sent || failed) && exit_code.is_some() {
                            break Ok(());
                        }
                    }
                    Some(ChannelMsg::ExitStatus { exit_status }) => {
                        exit_code = Some(exit_status);
                        if got_eof && (sent || exit_status != 0) {
                            break Ok(());
                        }
                    }
                    // Closed, or silent for too long after the last byte.
                    None => break Ok(()),
                    _ => {}
                },
            }
        }
    };

    let failed = match outcome {
        Err(e) => Err(e),
        Ok(()) => match exit_code {
            Some(code) if code != 0 => remote_outcome(i64::from(code), &stderr_buf),
            _ if !sent => Err(TransferError::IoError(
                "The server closed the upload before it finished".to_string(),
            )),
            _ => return Ok(()),
        },
    };
    let _ = tx.close().await;
    if matches!(failed, Err(TransferError::Cancelled)) {
        let command = format!("rm -f -- {}", shell_escape(remote_path));
        if let Err(e) = exec_on_connection_with_exit_code(handle, &command).await {
            tracing::warn!("Could not remove the cancelled upload {}: {}", remote_path, e);
        }
    }
    failed
}

/// The next message from the server. While data is still going out there is
/// no limit, since a slow link may keep the window shut for a long time;
/// once all is sent, a server silent for 30 seconds counts as gone.
async fn next_reply(rx: &mut russh::ChannelReadHalf, sent: bool) -> Option<russh::ChannelMsg> {
    if sent {
        tokio::time::timeout(std::time::Duration::from_secs(30), rx.wait()).await.ok().flatten()
    } else {
        rx.wait().await
    }
}

/// A remote command's exit as a result: its own words on failure.
fn remote_outcome(code: i64, stderr: &str) -> Result<(), TransferError> {
    if code == 0 {
        return Ok(());
    }
    let msg = stderr.trim();
    Err(TransferError::Remote(if msg.is_empty() {
        format!("The remote command exited with code {}", code)
    } else {
        msg.to_string()
    }))
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
///
/// A transfer that fails sends its error as the last item, so the drop
/// target fails its copy instead of keeping what arrived as the whole file:
/// an unreadable file used to drop as an empty one (#150).
pub async fn stream_file(
    handle: &SharedHandle,
    remote_path: &str,
    sink: std::sync::mpsc::SyncSender<Result<Vec<u8>, String>>,
) -> Result<u64, TransferError> {
    let channel_error = |e: russh::Error| TransferError::SshError(SshError::ChannelError(format!("{}", e)));
    let result = async {
        let mut channel = {
            let guard = handle.lock().await;
            guard.channel_open_session().await.map_err(channel_error)?
        };
        channel
            .exec(true, format!("base64 {}", shell_escape(remote_path)))
            .await
            .map_err(channel_error)?;
        receive_base64(&mut channel, &mut SinkWriter(&sink), DOWNLOAD_IDLE_LIMIT, |_| {}).await
    }
    .await;
    if let Err(e) = &result {
        let _ = sink.send(Err(e.to_string()));
    }
    result
}

/// Writes go to the drop target's channel, one chunk per write.
struct SinkWriter<'a>(&'a std::sync::mpsc::SyncSender<Result<Vec<u8>, String>>);

impl std::io::Write for SinkWriter<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0
            .send(Ok(buf.to_vec()))
            .map_err(|_| std::io::Error::other("drop cancelled"))?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use russh::ChannelMsg;
    use std::collections::VecDeque;
    use std::time::Duration;

    /// A channel that sends these messages, then closes.
    struct Sends(VecDeque<ChannelMsg>);

    impl ChannelMessages for Sends {
        async fn next_message(&mut self) -> Option<ChannelMsg> {
            self.0.pop_front()
        }
    }

    /// A channel that sends these messages, then nothing, ever.
    struct Stalls(VecDeque<ChannelMsg>);

    impl ChannelMessages for Stalls {
        async fn next_message(&mut self) -> Option<ChannelMsg> {
            match self.0.pop_front() {
                Some(m) => Some(m),
                None => std::future::pending().await,
            }
        }
    }

    fn stdout(s: &str) -> ChannelMsg {
        ChannelMsg::Data { data: s.as_bytes().to_vec().into() }
    }
    fn stderr(s: &str) -> ChannelMsg {
        ChannelMsg::ExtendedData { data: s.as_bytes().to_vec().into(), ext: 1 }
    }
    fn exit(status: u32) -> ChannelMsg {
        ChannelMsg::ExitStatus { exit_status: status }
    }

    async fn receive(channel: &mut impl ChannelMessages) -> (Result<u64, TransferError>, Vec<u8>) {
        let mut out = Vec::new();
        let result = receive_base64(channel, &mut out, Duration::from_millis(50), |_| {}).await;
        (result, out)
    }

    /// `base64 <file>` as the server answers it, for "hello world\n".
    const HELLO_B64: &str = "aGVsbG8gd29ybGQK\n";

    #[tokio::test]
    async fn a_file_that_was_sent_is_received_whole() {
        let mut ch = Sends(VecDeque::from([stdout(HELLO_B64), exit(0), ChannelMsg::Eof, ChannelMsg::Close]));
        let (result, out) = receive(&mut ch).await;
        assert_eq!(result.unwrap(), 12);
        assert_eq!(out, b"hello world\n");
    }

    #[tokio::test]
    async fn the_status_may_come_after_eof_and_lines_may_split_anywhere() {
        let mut ch = Sends(VecDeque::from([stdout("aGVsbG8g"), stdout("d29ybGQK"), ChannelMsg::Eof, exit(0)]));
        let (result, out) = receive(&mut ch).await;
        assert_eq!(result.unwrap(), 12);
        assert_eq!(out, b"hello world\n");
    }

    /// #150: what the server sends for a file the login user cannot read.
    /// It was taken as an empty file: an empty download, and no error.
    #[tokio::test]
    async fn a_file_the_server_cannot_read_is_an_error_saying_why() {
        let mut ch = Sends(VecDeque::from([
            stderr("base64: /home/reach/r150/rootonly.txt: Permission denied\n"),
            exit(1),
            ChannelMsg::Eof,
            ChannelMsg::Close,
        ]));
        let (result, out) = receive(&mut ch).await;
        let err = result.unwrap_err();
        assert_eq!(err.to_string(), "base64: /home/reach/r150/rootonly.txt: Permission denied");
        assert!(out.is_empty());
    }

    #[tokio::test]
    async fn a_failure_with_nothing_on_stderr_still_fails() {
        let mut ch = Sends(VecDeque::from([exit(2), ChannelMsg::Eof]));
        let err = receive(&mut ch).await.0.unwrap_err();
        assert_eq!(err.to_string(), "The server could not send the file (exit status 2)");
    }

    #[tokio::test]
    async fn a_command_stopped_by_a_signal_fails() {
        let mut ch = Sends(VecDeque::from([
            stdout(HELLO_B64),
            ChannelMsg::ExitSignal {
                signal_name: russh::Sig::KILL,
                core_dumped: false,
                error_message: String::new(),
                lang_tag: String::new(),
            },
            ChannelMsg::Eof,
        ]));
        assert!(receive(&mut ch).await.0.is_err());
    }

    /// The connection dropping mid-file was a short file reported complete.
    #[tokio::test]
    async fn a_channel_that_closes_without_a_status_is_a_file_cut_short() {
        let mut ch = Sends(VecDeque::from([stdout(HELLO_B64), ChannelMsg::Eof]));
        let err = receive(&mut ch).await.0.unwrap_err();
        assert_eq!(err.to_string(), "IO error: The connection closed before the whole file arrived");

        let mut ch = Sends(VecDeque::from([stdout(HELLO_B64), ChannelMsg::Close]));
        assert!(receive(&mut ch).await.0.is_err());
    }

    /// A server that went quiet was a short file reported complete too.
    #[tokio::test]
    async fn a_server_that_goes_quiet_stops_the_download_with_an_error() {
        let mut ch = Stalls(VecDeque::from([stdout(HELLO_B64)]));
        let err = receive(&mut ch).await.0.unwrap_err();
        assert!(err.to_string().contains("sent nothing"), "{err}");
    }

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

#[cfg(test)]
#[path = "transfer_live_tests.rs"]
mod live;
