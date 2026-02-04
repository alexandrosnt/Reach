pub mod credentials;
pub mod ipc;
pub mod monitoring;
pub mod playbook;
pub mod pty;
pub mod serial;
pub mod session;
pub mod sftp;
pub mod ssh;
pub mod state;
pub mod tunnel;

use state::AppState;
use tracing_subscriber::EnvFilter;

use ipc::ai_commands::*;
use ipc::credential_commands::*;
use ipc::monitoring_commands::*;
use ipc::playbook_commands::*;
use ipc::pty_commands::*;
use ipc::serial_commands::*;
use ipc::session_commands::*;
use ipc::sftp_commands::*;
use ipc::ssh_commands::*;
use ipc::tunnel_commands::*;

/// Build and run the Tauri application.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    tracing::info!("Starting Reach application");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            // SSH commands
            ssh_connect,
            ssh_disconnect,
            ssh_send,
            ssh_resize,
            ssh_list_connections,
            ssh_detect_os,
            // SFTP commands
            sftp_list_dir,
            sftp_upload,
            sftp_download,
            sftp_delete,
            sftp_rename,
            sftp_mkdir,
            sftp_touch,
            sftp_read_file,
            sftp_write_file,
            // Session commands
            session_list,
            session_get,
            session_create,
            session_update,
            session_delete,
            session_list_folders,
            session_create_folder,
            session_delete_folder,
            // Playbook commands
            playbook_run,
            playbook_get_run,
            playbook_stop,
            playbook_list,
            playbook_save,
            playbook_list_saved,
            playbook_delete,
            // Tunnel commands
            tunnel_create,
            tunnel_start,
            tunnel_stop,
            tunnel_list,
            // PTY commands
            pty_spawn,
            pty_write,
            pty_resize,
            pty_close,
            // Serial commands
            serial_list_ports,
            serial_open,
            serial_close,
            serial_send,
            // Monitoring commands
            monitoring_start,
            monitoring_stop,
            monitoring_get_stats,
            // AI commands
            ai_chat,
            ai_fetch_models,
            // Credential commands
            credential_set_master_password,
            credential_verify_master_password,
            credential_is_locked,
            credential_lock,
            credential_has_master_password,
            credential_save_password,
            credential_get_password,
            credential_has_password,
            credential_delete_password,
        ])
        .setup(|app| {
            use tauri::Manager;
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let app_data_dir = match handle.path().app_data_dir() {
                    Ok(dir) => dir,
                    Err(e) => {
                        tracing::warn!("Failed to get app data dir: {}", e);
                        return;
                    }
                };

                if let Err(e) = std::fs::create_dir_all(&app_data_dir) {
                    tracing::warn!("Failed to create app data dir: {}", e);
                    return;
                }

                match crate::session::storage::load_sessions(&app_data_dir).await {
                    Ok(store) => {
                        let state = handle.state::<AppState>();
                        let mut sessions = state.sessions.write().await;
                        for session in store.sessions {
                            sessions.insert(session.id.clone(), session);
                        }
                        let mut folders = state.folders.write().await;
                        for folder in store.folders {
                            folders.insert(folder.id.clone(), folder);
                        }
                        tracing::info!("Loaded sessions from disk");
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load sessions: {}", e);
                    }
                }

                match crate::playbook::storage::load_playbooks(&app_data_dir).await {
                    Ok(store) => {
                        let state = handle.state::<AppState>();
                        let mut playbooks = state.saved_playbooks.write().await;
                        for pb in store.playbooks {
                            playbooks.insert(pb.id.clone(), pb);
                        }
                        tracing::info!("Loaded playbooks from disk");
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load playbooks: {}", e);
                    }
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Reach application");
}
