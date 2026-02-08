use crate::toolchain::detect::{check_tool, ToolStatus};
use crate::toolchain::install;

#[tauri::command]
pub async fn toolchain_check(tool: String) -> Result<ToolStatus, String> {
    Ok(check_tool(&tool))
}

#[tauri::command]
pub async fn toolchain_install(
    tool: String,
    app_handle: tauri::AppHandle,
) -> Result<ToolStatus, String> {
    match tool.as_str() {
        "terraform" => {
            let data_dir = dirs::data_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("com.reach.app");
            install::install_terraform(&app_handle, &data_dir).await?;
        }
        "ansible" => {
            install::install_ansible(&app_handle).await?;
        }
        _ => {
            return Err(format!("Unknown tool: {}", tool));
        }
    }

    // Re-check after install
    Ok(check_tool(&tool))
}
