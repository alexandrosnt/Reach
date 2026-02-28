pub mod ai_commands;
pub mod plugin_commands;
pub mod credential_commands;
pub mod settings_commands;
pub mod monitoring_commands;
#[cfg(desktop)]
pub mod pty_commands;
#[cfg(desktop)]
pub mod serial_commands;
pub mod session_commands;
pub mod sftp_commands;
pub mod ssh_commands;
pub mod sshconfig_commands;
pub mod ansible_commands;
pub mod tofu_commands;
pub mod toolchain_commands;
pub mod tunnel_commands;
pub mod vault_commands;
