pub mod playbook;
pub mod plugin;
pub mod ipc;
pub mod monitoring;
#[cfg(desktop)]
pub mod pty;
#[cfg(desktop)]
pub mod serial;
pub mod session;
pub mod sftp;
pub mod ssh;
pub mod state;
pub mod terraform;
pub mod toolchain;
pub mod tunnel;
pub mod vault;

use std::sync::atomic::Ordering;

use state::AppState;
use tracing_subscriber::EnvFilter;

use ipc::ai_commands::*;
use ipc::playbook_commands::*;
use ipc::plugin_commands::*;
use ipc::credential_commands::*;
use ipc::settings_commands::*;
use ipc::monitoring_commands::*;
#[cfg(desktop)]
use ipc::pty_commands::*;
#[cfg(desktop)]
use ipc::serial_commands::*;
use ipc::session_commands::*;
use ipc::sftp_commands::*;
use ipc::ssh_commands::*;
use ipc::sshconfig_commands::*;
use ipc::terraform_commands::*;
use ipc::toolchain_commands::*;
use ipc::tunnel_commands::*;
use ipc::vault_commands::*;

#[tauri::command]
fn set_close_to_tray(state: tauri::State<'_, AppState>, enabled: bool) {
    state.close_to_tray.store(enabled, Ordering::Relaxed);
}

#[tauri::command]
fn get_close_to_tray(state: tauri::State<'_, AppState>) -> bool {
    state.close_to_tray.load(Ordering::Relaxed)
}

/// Build and run the Tauri application.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    tracing::info!("Starting Reach application");

    // Work around WebKitGTK DMA-BUF protocol errors on Wayland (KDE Plasma, etc.)
    #[cfg(target_os = "linux")]
    {
        if std::env::var("WEBKIT_DISABLE_DMABUF_RENDERER").is_err() {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
            tracing::info!("Set WEBKIT_DISABLE_DMABUF_RENDERER=1 for Wayland compatibility");
        }
    }

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .manage(AppState::new());

    #[cfg(desktop)]
    {
        builder = builder
            .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
                use tauri::Manager;
                // When a second instance is launched, focus the existing window
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.unminimize();
                    let _ = window.set_focus();
                }
            }))
            .plugin(tauri_plugin_updater::Builder::new().build())
            .plugin(tauri_plugin_autostart::init(
                tauri_plugin_autostart::MacosLauncher::LaunchAgent,
                None,
            ));
    }

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
            // SSH Config commands
            sshconfig_list_hosts,
            sshconfig_resolve_host,
            sshconfig_exists,
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
            playbook_cancel,
            playbook_get_run,
            playbook_validate,
            playbook_save_project,
            playbook_list_projects,
            playbook_delete_project,
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
            // Terraform commands
            terraform_run,
            terraform_cancel,
            terraform_get_run,
            terraform_state_list,
            terraform_state_show,
            terraform_output,
            terraform_check,
            terraform_save_workspace,
            terraform_list_workspaces,
            terraform_delete_workspace,
            // Toolchain commands
            toolchain_check,
            toolchain_install,
            // Plugin commands
            plugin_discover,
            plugin_load,
            plugin_unload,
            plugin_reload,
            plugin_list,
            plugin_call_action,
            plugin_get_ui,
            plugin_get_config,
            plugin_set_config,
            plugin_get_dir,
            plugin_set_dir,
            plugin_dispatch_hook,
            // Tray commands
            set_close_to_tray,
            get_close_to_tray,
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
            // SSH Config commands
            sshconfig_list_hosts,
            sshconfig_resolve_host,
            sshconfig_exists,
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
            playbook_cancel,
            playbook_get_run,
            playbook_validate,
            playbook_save_project,
            playbook_list_projects,
            playbook_delete_project,
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
            // Terraform commands
            terraform_run,
            terraform_cancel,
            terraform_get_run,
            terraform_state_list,
            terraform_state_show,
            terraform_output,
            terraform_check,
            terraform_save_workspace,
            terraform_list_workspaces,
            terraform_delete_workspace,
            // Toolchain commands
            toolchain_check,
            toolchain_install,
            // Plugin commands
            plugin_discover,
            plugin_load,
            plugin_unload,
            plugin_reload,
            plugin_list,
            plugin_call_action,
            plugin_get_ui,
            plugin_get_config,
            plugin_set_config,
            plugin_get_dir,
            plugin_set_dir,
            plugin_dispatch_hook,
            // Tray commands
            set_close_to_tray,
            get_close_to_tray,
        ]);
    }

    builder
        .setup(|app| {
            use tauri::Manager;

            // Build system tray
            #[cfg(desktop)]
            {
                use tauri::menu::{MenuBuilder, MenuItemBuilder};
                use tauri::tray::TrayIconBuilder;
                use tauri::image::Image;

                let show_item = MenuItemBuilder::with_id("show", "Show").build(app)?;
                let quit_item = MenuItemBuilder::with_id("quit", "Quit").build(app)?;
                let tray_menu = MenuBuilder::new(app)
                    .item(&show_item)
                    .separator()
                    .item(&quit_item)
                    .build()?;

                let icon = Image::from_bytes(include_bytes!("../icons/32x32.png"))
                    .expect("failed to load tray icon");

                TrayIconBuilder::new()
                    .icon(icon)
                    .tooltip("Reach")
                    .menu(&tray_menu)
                    .on_menu_event(|app_handle, event| {
                        match event.id().as_ref() {
                            "show" => {
                                if let Some(window) = app_handle.get_webview_window("main") {
                                    let _ = window.show();
                                    let _ = window.unminimize();
                                    let _ = window.set_focus();
                                }
                            }
                            "quit" => {
                                app_handle.exit(0);
                            }
                            _ => {}
                        }
                    })
                    .on_tray_icon_event(|tray, event| {
                        if let tauri::tray::TrayIconEvent::Click { button: tauri::tray::MouseButton::Left, .. } = event {
                            let app_handle = tray.app_handle();
                            if let Some(window) = app_handle.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.unminimize();
                                let _ = window.set_focus();
                            }
                        }
                    })
                    .build(app)?;
            }

            // Prepend tools dir to PATH so installed tools are found
            {
                let data_dir = dirs::data_dir()
                    .unwrap_or_else(|| std::path::PathBuf::from("."))
                    .join("com.reach.app");
                let tools_dir = data_dir.join("tools");
                let _ = std::fs::create_dir_all(&tools_dir);
                let current_path = std::env::var("PATH").unwrap_or_default();
                let sep = if cfg!(windows) { ";" } else { ":" };
                std::env::set_var(
                    "PATH",
                    format!("{}{}{}", tools_dir.display(), sep, current_path),
                );
                tracing::info!("Tools directory added to PATH: {:?}", tools_dir);
            }

            let handle = app.handle().clone();

            // Clone state arcs for plugin auto-loading
            let app_state = app.state::<AppState>();
            let ssh_mgr = app_state.ssh_manager.clone();
            let tunnel_mgr = app_state.tunnel_manager.clone();
            let vault_mgr = app_state.vault_manager.clone();
            let plugin_mgr = app_state.plugin_manager.clone();

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

                tracing::info!("App data directory ready: {:?}", app_data_dir);

                // Auto-load plugins on startup
                let saved_configs = {
                    let vm = vault_mgr.lock().await;
                    plugin::storage::load_plugin_configs(&vm)
                        .await
                        .unwrap_or_default()
                };

                let config_map: std::collections::HashMap<String, plugin::schema::PluginConfig> =
                    saved_configs
                        .into_iter()
                        .map(|c| (c.id.clone(), c))
                        .collect();

                let mut pm = plugin_mgr.lock().await;
                if let Ok(manifests) = pm.discover_plugins() {
                    for manifest in manifests {
                        let config =
                            config_map.get(&manifest.id).cloned().unwrap_or_else(|| {
                                plugin::schema::PluginConfig {
                                    id: manifest.id.clone(),
                                    enabled: true,
                                    granted_permissions: manifest.permissions.clone(),
                                }
                            });
                        if config.enabled {
                            match pm.load_plugin(
                                &manifest.id,
                                config,
                                ssh_mgr.clone(),
                                tunnel_mgr.clone(),
                                vault_mgr.clone(),
                                Some(handle.clone()),
                            ) {
                                Ok(info) => {
                                    tracing::info!(
                                        "Auto-loaded plugin: {} ({}) [{:?}]",
                                        info.manifest.name,
                                        info.manifest.id,
                                        info.status,
                                    );
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        "Failed to auto-load plugin {}: {}",
                                        manifest.id,
                                        e,
                                    );
                                }
                            }
                        }
                    }
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                use tauri::Manager;
                let app_state = window.state::<AppState>();
                if app_state.close_to_tray.load(Ordering::Relaxed) {
                    api.prevent_close();
                    #[cfg(desktop)]
                    let _ = window.hide();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Reach application");
}
