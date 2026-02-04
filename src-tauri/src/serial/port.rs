use std::collections::HashMap;
use std::io::Read as IoRead;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::Emitter;
use thiserror::Error;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

#[derive(Debug, Error)]
pub enum SerialError {
    #[error("Port not found: {0}")]
    PortNotFound(String),
    #[error("Port already open: {0}")]
    AlreadyOpen(String),
    #[error("Port not open: {0}")]
    NotOpen(String),
    #[error("IO error: {0}")]
    IoError(String),
    #[error("Enumeration failed: {0}")]
    EnumerationFailed(String),
}

/// Metadata about an available serial port.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerialPortInfo {
    pub name: String,
    pub description: String,
    pub port_type: String,
}

/// An active serial port with a write sender and reader task handle.
struct ActiveSerialPort {
    write_tx: mpsc::UnboundedSender<Vec<u8>>,
    shutdown_tx: mpsc::Sender<()>,
    task: JoinHandle<()>,
}

/// Manages open serial port connections.
pub struct SerialManager {
    ports: HashMap<String, ActiveSerialPort>,
}

impl SerialManager {
    pub fn new() -> Self {
        Self {
            ports: HashMap::new(),
        }
    }

    /// Open a serial port, spawn read/write tasks, emit data events.
    pub fn open_port(
        &mut self,
        port_name: &str,
        baud_rate: u32,
        app_handle: &tauri::AppHandle,
    ) -> Result<(), SerialError> {
        if self.ports.contains_key(port_name) {
            return Err(SerialError::AlreadyOpen(port_name.to_string()));
        }

        tracing::info!("Opening serial port {} at {} baud", port_name, baud_rate);

        // Open the serial port (blocking API)
        let port = serialport::new(port_name, baud_rate)
            .timeout(Duration::from_millis(100))
            .open()
            .map_err(|e| SerialError::IoError(format!("Failed to open {}: {}", port_name, e)))?;

        // Clone the port for writing (serialport supports try_clone)
        let write_port = port
            .try_clone()
            .map_err(|e| SerialError::IoError(format!("Failed to clone port: {}", e)))?;

        let (write_tx, mut write_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);

        let data_event = format!("serial-data-{}", port_name);
        let handle = app_handle.clone();
        let pname = port_name.to_string();

        // Spawn a combined read/write task using blocking operations on a thread
        let task = tokio::spawn(async move {
            let mut read_port = port;
            let mut write_port = write_port;

            // Spawn a blocking reader on a dedicated thread
            let reader_data_event = data_event.clone();
            let reader_handle = handle.clone();
            let reader_pname = pname.clone();
            let (reader_shutdown_tx, reader_shutdown_rx) = std::sync::mpsc::channel::<()>();

            let reader_thread = std::thread::spawn(move || {
                let mut buf = vec![0u8; 1024];
                loop {
                    // Check for shutdown
                    if reader_shutdown_rx.try_recv().is_ok() {
                        break;
                    }
                    match read_port.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            let data = String::from_utf8_lossy(&buf[..n]).to_string();
                            if let Err(e) = reader_handle.emit(&reader_data_event, &data) {
                                tracing::error!(
                                    "Serial {}: failed to emit data: {}",
                                    reader_pname, e
                                );
                                break;
                            }
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                            // Timeout is normal with polling, just continue
                            continue;
                        }
                        Err(e) => {
                            tracing::error!("Serial {}: read error: {}", reader_pname, e);
                            break;
                        }
                    }
                }
                tracing::info!("Serial {}: reader thread exiting", reader_pname);
            });

            // Handle writes and shutdown in the async task
            loop {
                tokio::select! {
                    data = write_rx.recv() => {
                        match data {
                            Some(bytes) => {
                                use std::io::Write;
                                if let Err(e) = write_port.write_all(&bytes) {
                                    tracing::error!("Serial {}: write error: {}", pname, e);
                                    break;
                                }
                                if let Err(e) = write_port.flush() {
                                    tracing::error!("Serial {}: flush error: {}", pname, e);
                                    break;
                                }
                            }
                            None => {
                                // Channel closed
                                break;
                            }
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        tracing::info!("Serial {}: shutdown signal received", pname);
                        break;
                    }
                }
            }

            // Signal reader thread to stop
            let _ = reader_shutdown_tx.send(());
            let _ = reader_thread.join();
            tracing::info!("Serial {}: task exiting", pname);
        });

        self.ports.insert(
            port_name.to_string(),
            ActiveSerialPort {
                write_tx,
                shutdown_tx,
                task,
            },
        );

        tracing::info!("Serial port {} opened successfully", port_name);
        Ok(())
    }

    /// Send data over an open serial port.
    pub fn send_data(&self, port_name: &str, data: &[u8]) -> Result<(), SerialError> {
        let active = self
            .ports
            .get(port_name)
            .ok_or_else(|| SerialError::NotOpen(port_name.to_string()))?;

        active
            .write_tx
            .send(data.to_vec())
            .map_err(|e| SerialError::IoError(format!("Send failed: {}", e)))
    }

    /// Close an open serial port.
    pub async fn close_port(&mut self, port_name: &str) -> Result<(), SerialError> {
        let active = self
            .ports
            .remove(port_name)
            .ok_or_else(|| SerialError::NotOpen(port_name.to_string()))?;

        tracing::info!("Closing serial port {}", port_name);
        let _ = active.shutdown_tx.send(()).await;

        // Wait for task to finish with timeout
        let _ = tokio::time::timeout(Duration::from_secs(3), active.task).await;

        tracing::info!("Serial port {} closed", port_name);
        Ok(())
    }

    /// Check if a port is currently open.
    pub fn is_open(&self, port_name: &str) -> bool {
        self.ports.contains_key(port_name)
    }
}

impl Default for SerialManager {
    fn default() -> Self {
        Self::new()
    }
}

/// List all available serial ports on the system.
pub fn list_ports() -> Result<Vec<SerialPortInfo>, SerialError> {
    tracing::debug!("Enumerating serial ports");
    match serialport::available_ports() {
        Ok(ports) => {
            let infos = ports
                .into_iter()
                .map(|p| {
                    let description = match &p.port_type {
                        serialport::SerialPortType::UsbPort(info) => {
                            format!(
                                "USB - {}",
                                info.product.as_deref().unwrap_or("Unknown")
                            )
                        }
                        serialport::SerialPortType::PciPort => "PCI".to_string(),
                        serialport::SerialPortType::BluetoothPort => "Bluetooth".to_string(),
                        serialport::SerialPortType::Unknown => "Unknown".to_string(),
                    };
                    let port_type = match &p.port_type {
                        serialport::SerialPortType::UsbPort(_) => "usb".to_string(),
                        serialport::SerialPortType::PciPort => "pci".to_string(),
                        serialport::SerialPortType::BluetoothPort => "bluetooth".to_string(),
                        serialport::SerialPortType::Unknown => "unknown".to_string(),
                    };
                    SerialPortInfo {
                        name: p.port_name,
                        description,
                        port_type,
                    }
                })
                .collect();
            Ok(infos)
        }
        Err(e) => Err(SerialError::EnumerationFailed(e.to_string())),
    }
}
