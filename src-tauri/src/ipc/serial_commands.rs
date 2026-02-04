use crate::serial::port::{self, SerialPortInfo};
use crate::state::AppState;

/// List available serial ports on the system.
#[tauri::command]
pub async fn serial_list_ports(
    _state: tauri::State<'_, AppState>,
) -> Result<Vec<SerialPortInfo>, String> {
    port::list_ports().map_err(|e| e.to_string())
}

/// Open a serial port connection.
#[tauri::command]
pub async fn serial_open(
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
    port_name: String,
    baud_rate: u32,
) -> Result<(), String> {
    let mut serial_manager = state.serial_manager.lock().await;
    serial_manager
        .open_port(&port_name, baud_rate, &app_handle)
        .map_err(|e| e.to_string())
}

/// Close an open serial port.
#[tauri::command]
pub async fn serial_close(
    state: tauri::State<'_, AppState>,
    port_name: String,
) -> Result<(), String> {
    let mut serial_manager = state.serial_manager.lock().await;
    serial_manager
        .close_port(&port_name)
        .await
        .map_err(|e| e.to_string())
}

/// Send data over an open serial port.
#[tauri::command]
pub async fn serial_send(
    state: tauri::State<'_, AppState>,
    port_name: String,
    data: String,
) -> Result<(), String> {
    let serial_manager = state.serial_manager.lock().await;
    serial_manager
        .send_data(&port_name, data.as_bytes())
        .map_err(|e| e.to_string())
}
