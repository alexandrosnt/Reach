use std::collections::HashMap;
use std::io::{Read, Write};
use portable_pty::{native_pty_system, CommandBuilder, PtySize, MasterPty, Child};
use tauri::Emitter;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PtyError {
    #[error("PTY spawn failed: {0}")]
    SpawnFailed(String),
    #[error("PTY not found: {0}")]
    NotFound(String),
    #[error("Resize failed: {0}")]
    ResizeFailed(String),
    #[error("Write error: {0}")]
    WriteError(String),
    #[error("Read error: {0}")]
    ReadError(String),
}

struct PtyInstance {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
    cols: u16,
    rows: u16,
}

/// Manages local pseudo-terminal sessions using portable-pty.
pub struct PtyManager {
    instances: HashMap<String, PtyInstance>,
}

impl PtyManager {
    pub fn new() -> Self {
        Self {
            instances: HashMap::new(),
        }
    }

    /// Spawn a new PTY session with the given shell.
    ///
    /// If `shell` is None, auto-detects:
    /// - Windows: `powershell.exe`
    /// - Unix: `$SHELL` env var, fallback to `/bin/bash`
    ///
    /// Starts a background reader thread that emits `pty-data-{id}` events
    /// with output data and `pty-exit-{id}` when the process exits.
    pub fn spawn(
        &mut self,
        id: &str,
        shell: Option<String>,
        cols: u16,
        rows: u16,
        app_handle: tauri::AppHandle,
    ) -> Result<String, PtyError> {
        let shell_path = match shell {
            Some(s) => s,
            None => detect_default_shell(),
        };

        tracing::info!(
            "Spawning PTY '{}' with shell '{}' ({}x{})",
            id, shell_path, cols, rows,
        );

        let pty_system = native_pty_system();

        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| PtyError::SpawnFailed(format!("Failed to open PTY pair: {}", e)))?;

        let mut cmd = CommandBuilder::new(&shell_path);

        // Set TERM so that programs like clear, htop, vim work correctly.
        // xterm-256color matches the xterm.js frontend.
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");

        // On Windows, pass -NoLogo to PowerShell for a cleaner experience.
        #[cfg(target_os = "windows")]
        {
            let lower = shell_path.to_lowercase();
            if lower.contains("powershell") || lower.contains("pwsh") {
                cmd.arg("-NoLogo");
            }
        }

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| PtyError::SpawnFailed(format!("Failed to spawn '{}': {}", shell_path, e)))?;

        // We need a separate reader handle for the background thread.
        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| PtyError::SpawnFailed(format!("Failed to clone PTY reader: {}", e)))?;

        let writer = pair
            .master
            .take_writer()
            .map_err(|e| PtyError::SpawnFailed(format!("Failed to take PTY writer: {}", e)))?;

        // Store the instance.
        let instance = PtyInstance {
            master: pair.master,
            writer,
            child,
            cols,
            rows,
        };
        let id_owned = id.to_string();
        self.instances.insert(id_owned.clone(), instance);

        // Spawn a blocking reader thread (NOT a tokio task) because
        // portable-pty's read is blocking I/O.
        let reader_id = id_owned.clone();
        let handle = app_handle.clone();
        std::thread::Builder::new()
            .name(format!("pty-reader-{}", reader_id))
            .spawn(move || {
                pty_reader_loop(reader, &reader_id, &handle);
            })
            .map_err(|e| PtyError::SpawnFailed(format!("Failed to spawn reader thread: {}", e)))?;

        tracing::info!("PTY '{}' spawned successfully", id);
        Ok(id.to_string())
    }

    /// Write data (user keystrokes) to the PTY.
    pub fn write(&mut self, id: &str, data: &[u8]) -> Result<(), PtyError> {
        let instance = self
            .instances
            .get_mut(id)
            .ok_or_else(|| PtyError::NotFound(id.to_string()))?;

        instance
            .writer
            .write_all(data)
            .map_err(|e| PtyError::WriteError(format!("{}", e)))?;

        instance
            .writer
            .flush()
            .map_err(|e| PtyError::WriteError(format!("flush: {}", e)))?;

        Ok(())
    }

    /// Resize an existing PTY session.
    pub fn resize(&mut self, id: &str, cols: u16, rows: u16) -> Result<(), PtyError> {
        let instance = self
            .instances
            .get_mut(id)
            .ok_or_else(|| PtyError::NotFound(id.to_string()))?;

        instance
            .master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| PtyError::ResizeFailed(format!("{}", e)))?;

        instance.cols = cols;
        instance.rows = rows;

        tracing::debug!("Resized PTY '{}' to {}x{}", id, cols, rows);
        Ok(())
    }

    /// Kill the child process and remove the PTY instance.
    pub fn close(&mut self, id: &str) -> Result<(), PtyError> {
        let mut instance = self
            .instances
            .remove(id)
            .ok_or_else(|| PtyError::NotFound(id.to_string()))?;

        // Attempt to kill the child process. If it already exited, that's fine.
        if let Err(e) = instance.child.kill() {
            tracing::warn!("Failed to kill PTY child '{}': {} (may have already exited)", id, e);
        }

        // Wait for the child to fully exit so we don't leave zombies.
        let _ = instance.child.wait();

        tracing::info!("Closed PTY '{}'", id);
        Ok(())
    }
}

impl Default for PtyManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Blocking read loop that runs on a dedicated OS thread.
///
/// Reads chunks from the PTY master reader and emits them as
/// `pty-data-{id}` events. When the read returns 0 or errors,
/// emits `pty-exit-{id}` and exits.
fn pty_reader_loop(
    mut reader: Box<dyn Read + Send>,
    id: &str,
    app_handle: &tauri::AppHandle,
) {
    let data_event = format!("pty-data-{}", id);
    let exit_event = format!("pty-exit-{}", id);
    let mut buf = [0u8; 4096];

    loop {
        match reader.read(&mut buf) {
            Ok(0) => {
                // EOF — the child process has exited.
                tracing::debug!("PTY '{}' reader got EOF", id);
                break;
            }
            Ok(n) => {
                let payload = String::from_utf8_lossy(&buf[..n]).to_string();
                if let Err(e) = app_handle.emit(&data_event, payload) {
                    tracing::error!("Failed to emit '{}': {}", data_event, e);
                    break;
                }
            }
            Err(e) => {
                tracing::debug!("PTY '{}' reader error: {}", id, e);
                break;
            }
        }
    }

    // Notify frontend that the PTY has exited.
    if let Err(e) = app_handle.emit(&exit_event, ()) {
        tracing::error!("Failed to emit '{}': {}", exit_event, e);
    }

    tracing::info!("PTY '{}' reader thread exiting", id);
}

/// Detect the default shell for the current platform.
fn detect_default_shell() -> String {
    #[cfg(target_os = "windows")]
    {
        "powershell.exe".to_string()
    }

    #[cfg(not(target_os = "windows"))]
    {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string())
    }
}
