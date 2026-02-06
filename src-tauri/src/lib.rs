pub mod ipc;
pub mod monitoring;
pub mod playbook;
#[cfg(desktop)]
pub mod pty;
#[cfg(desktop)]
pub mod serial;
pub mod session;
pub mod sftp;
pub mod ssh;
pub mod state;
pub mod tunnel;
pub mod vault;

use state::AppState;
use tracing_subscriber::EnvFilter;

use ipc::ai_commands::*;
use ipc::credential_commands::*;
use ipc::settings_commands::*;
use ipc::monitoring_commands::*;
use ipc::playbook_commands::*;
#[cfg(desktop)]
use ipc::pty_commands::*;
#[cfg(desktop)]
use ipc::serial_commands::*;
use ipc::session_commands::*;
use ipc::sftp_commands::*;
use ipc::ssh_commands::*;
use ipc::tunnel_commands::*;
use ipc::vault_commands::*;

/// Build and run the Tauri application.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    tracing::info!("Starting Reach application");

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .manage(AppState::new());

    #[cfg(desktop)]
    {
        builder = builder.invoke_handler(tauri::generate_handler![
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
            session_share,
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
            // Settings commands
            settings_get_all,
            settings_get,
            settings_set,
            settings_delete,
            settings_save_all,
            // Vault commands
            vault_init_identity,
            vault_unlock,
            vault_auto_unlock,
            vault_reset,
            vault_export_identity,
            vault_import_identity,
            vault_lock,
            vault_is_locked,
            vault_has_identity,
            vault_get_public_key,
            vault_get_user_uuid,
            vault_create,
            vault_open,
            vault_close,
            vault_list,
            vault_unlock_vault,
            vault_lock_vault,
            vault_sync,
            vault_secret_create,
            vault_secret_read,
            vault_secret_update,
            vault_secret_delete,
            vault_secret_list,
            vault_invite_member,
            vault_accept_invite,
            vault_remove_member,
            vault_list_members,
            vault_delete,
            // Vault sharing individual items
            vault_share_item,
            vault_list_shared_items,
            vault_revoke_shared_item,
            vault_accept_shared_item,
            vault_list_received_shares,
            // Vault settings
            vault_get_settings,
            vault_save_settings,
            vault_get_turso_config,
            vault_set_turso_config,
            // Turso Platform API
            turso_create_database,
            turso_create_database_token,
            // Personal sync config
            vault_set_personal_sync,
            vault_get_personal_sync,
            // Full backup
            vault_export_backup,
            vault_preview_backup,
            vault_import_backup,
        ]);
    }

    #[cfg(not(desktop))]
    {
        builder = builder.invoke_handler(tauri::generate_handler![
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
            session_share,
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
            // Settings commands
            settings_get_all,
            settings_get,
            settings_set,
            settings_delete,
            settings_save_all,
            // Vault commands
            vault_init_identity,
            vault_unlock,
            vault_auto_unlock,
            vault_reset,
            vault_export_identity,
            vault_import_identity,
            vault_lock,
            vault_is_locked,
            vault_has_identity,
            vault_get_public_key,
            vault_get_user_uuid,
            vault_create,
            vault_open,
            vault_close,
            vault_list,
            vault_unlock_vault,
            vault_lock_vault,
            vault_sync,
            vault_secret_create,
            vault_secret_read,
            vault_secret_update,
            vault_secret_delete,
            vault_secret_list,
            vault_invite_member,
            vault_accept_invite,
            vault_remove_member,
            vault_list_members,
            vault_delete,
            // Vault sharing individual items
            vault_share_item,
            vault_list_shared_items,
            vault_revoke_shared_item,
            vault_accept_shared_item,
            vault_list_received_shares,
            // Vault settings
            vault_get_settings,
            vault_save_settings,
            vault_get_turso_config,
            vault_set_turso_config,
            // Turso Platform API
            turso_create_database,
            turso_create_database_token,
            // Personal sync config
            vault_set_personal_sync,
            vault_get_personal_sync,
            // Full backup
            vault_export_backup,
            vault_preview_backup,
            vault_import_backup,
        ]);
    }

    builder
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

                // All data (sessions, playbooks, credentials, settings) now loaded
                // from encrypted vault on-demand after user unlocks with master password.
                // No JSON file loading needed.
                tracing::info!("App data directory ready: {:?}", app_data_dir);
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Reach application");
}
