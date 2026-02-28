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
        "ansible" => {
            install::install_ansible(&app_handle).await?;
        }
        "tofu" => {
            install::install_tofu(&app_handle).await?;
        }
        _ => {
            return Err(format!("Unknown tool: {}", tool));
        }
    }

    // Re-check after install
    Ok(check_tool(&tool))
}
