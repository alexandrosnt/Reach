use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use mlua::Lua;
use tauri::Emitter;

use crate::plugin::host_api::{inject_host_api, PluginAppState};
use crate::plugin::sandbox::create_sandbox;
use crate::plugin::schema::*;
use crate::ssh::client::SshManager;
use crate::tunnel::manager::TunnelManager;
use crate::vault::VaultManager;

/// A loaded plugin instance with its Lua VM.
pub struct PluginInstance {
    pub manifest: PluginManifest,
    pub config: PluginConfig,
    pub status: PluginStatus,
    pub lua: Option<Lua>,
    pub ui_state: Option<PluginUiState>,
    pub plugin_dir: PathBuf,
}

/// Manages all plugins: discovery, lifecycle, and hook dispatch.
pub struct PluginManager {
    pub plugins_dir: PathBuf,
    pub plugins: HashMap<String, PluginInstance>,
    pub hook_registry: HashMap<String, Vec<String>>,
}

impl PluginManager {
    pub fn new(plugins_dir: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&plugins_dir);
        Self {
            plugins_dir,
            plugins: HashMap::new(),
            hook_registry: HashMap::new(),
        }
    }

    /// Scan the plugins directory for plugin.toml manifests.
    pub fn discover_plugins(&self) -> Result<Vec<PluginManifest>, String> {
        let mut manifests = Vec::new();

        let entries = std::fs::read_dir(&self.plugins_dir)
            .map_err(|e| format!("Cannot read plugins dir: {}", e))?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let manifest_path = path.join("plugin.toml");
                if manifest_path.exists() {
                    match std::fs::read_to_string(&manifest_path) {
                        Ok(content) => match toml::from_str::<PluginManifest>(&content) {
                            Ok(manifest) => manifests.push(manifest),
                            Err(e) => {
                                tracing::warn!(
                                    "Invalid plugin.toml in {:?}: {}",
                                    path,
                                    e
                                );
                            }
                        },
                        Err(e) => {
                            tracing::warn!("Cannot read {:?}: {}", manifest_path, e);
                        }
                    }
                }
            }
        }

        Ok(manifests)
    }

    /// Load a plugin by ID, creating its sandboxed VM and calling on_init().
    pub fn load_plugin(
        &mut self,
        plugin_id: &str,
        config: PluginConfig,
        ssh_manager: Arc<tokio::sync::Mutex<SshManager>>,
        tunnel_manager: Arc<tokio::sync::Mutex<TunnelManager>>,
        vault_manager: Arc<tokio::sync::Mutex<VaultManager>>,
        app_handle: Option<tauri::AppHandle>,
    ) -> Result<PluginInfo, String> {
        let plugin_dir = self.plugins_dir.join(plugin_id);
        let manifest_path = plugin_dir.join("plugin.toml");

        let manifest_content =
            std::fs::read_to_string(&manifest_path).map_err(|e| e.to_string())?;
        let manifest: PluginManifest =
            toml::from_str(&manifest_content).map_err(|e| e.to_string())?;

        if !config.enabled {
            let instance = PluginInstance {
                manifest: manifest.clone(),
                config: config.clone(),
                status: PluginStatus::Disabled,
                lua: None,
                ui_state: None,
                plugin_dir,
            };
            let info = make_plugin_info(&instance);
            self.plugins.insert(plugin_id.to_string(), instance);
            return Ok(info);
        }

        // Create sandboxed VM
        let lua = create_sandbox().map_err(|e| format!("Sandbox creation failed: {}", e))?;

        // Build granted permissions set
        let granted: HashSet<PluginPermission> =
            config.granted_permissions.iter().cloned().collect();

        // Inject host API
        let plugin_app_state = PluginAppState {
            ssh_manager,
            tunnel_manager,
            vault_manager,
            app_handle,
            plugin_id: plugin_id.to_string(),
        };
        inject_host_api(&lua, &granted, plugin_app_state)
            .map_err(|e| format!("Failed to inject host API: {}", e))?;

        // Load and execute the entry file
        let entry_path = plugin_dir.join(&manifest.entry);
        let entry_code = std::fs::read_to_string(&entry_path)
            .map_err(|e| format!("Cannot read {}: {}", manifest.entry, e))?;

        let status = match lua.load(&entry_code).set_name(&manifest.entry).exec() {
            Ok(()) => {
                // Try calling on_init if it exists
                match lua.globals().get::<mlua::Function>("on_init") {
                    Ok(func) => match func.call::<()>(()) {
                        Ok(()) => PluginStatus::Running,
                        Err(e) => PluginStatus::Error {
                            message: format!("on_init error: {}", e),
                        },
                    },
                    Err(_) => PluginStatus::Running,
                }
            }
            Err(e) => PluginStatus::Error {
                message: format!("Load error: {}", e),
            },
        };

        // Register hooks
        for hook_name in &manifest.hooks {
            self.hook_registry
                .entry(hook_name.clone())
                .or_default()
                .push(plugin_id.to_string());
        }

        // Check if plugin has UI (has on_ui or initial render)
        let has_ui = lua.globals().get::<mlua::Function>("on_ui").is_ok();

        // Try to get initial UI state
        let ui_state = if has_ui {
            get_ui_from_lua(&lua, plugin_id)
        } else {
            None
        };

        let instance = PluginInstance {
            manifest: manifest.clone(),
            config: config.clone(),
            status,
            lua: Some(lua),
            ui_state,
            plugin_dir,
        };

        let info = make_plugin_info(&instance);
        self.plugins.insert(plugin_id.to_string(), instance);

        Ok(info)
    }

    /// Unload a plugin, calling on_unload() first.
    pub fn unload_plugin(&mut self, plugin_id: &str) -> Result<(), String> {
        if let Some(instance) = self.plugins.get(plugin_id) {
            if let Some(ref lua) = instance.lua {
                if let Ok(func) = lua.globals().get::<mlua::Function>("on_unload") {
                    let _ = func.call::<()>(());
                }
            }
        }

        // Remove from hook registry
        for entries in self.hook_registry.values_mut() {
            entries.retain(|id| id != plugin_id);
        }

        self.plugins.remove(plugin_id);
        Ok(())
    }

    /// Reload a plugin.
    pub fn reload_plugin(
        &mut self,
        plugin_id: &str,
        config: PluginConfig,
        ssh_manager: Arc<tokio::sync::Mutex<SshManager>>,
        tunnel_manager: Arc<tokio::sync::Mutex<TunnelManager>>,
        vault_manager: Arc<tokio::sync::Mutex<VaultManager>>,
        app_handle: Option<tauri::AppHandle>,
    ) -> Result<PluginInfo, String> {
        self.unload_plugin(plugin_id)?;
        self.load_plugin(
            plugin_id,
            config,
            ssh_manager,
            tunnel_manager,
            vault_manager,
            app_handle,
        )
    }

    /// Call an action function in a plugin's Lua VM.
    pub fn call_action(
        &mut self,
        plugin_id: &str,
        action: &str,
        params: serde_json::Value,
    ) -> Result<Option<PluginUiState>, String> {
        let instance = self
            .plugins
            .get_mut(plugin_id)
            .ok_or_else(|| format!("Plugin not found: {}", plugin_id))?;

        let lua = instance
            .lua
            .as_ref()
            .ok_or_else(|| "Plugin VM not loaded".to_string())?;

        let func_name = format!("on_action_{}", action);
        match lua.globals().get::<mlua::Function>(&*func_name) {
            Ok(func) => {
                let lua_val = mlua::LuaSerdeExt::to_value(lua, &params)
                    .map_err(|e| e.to_string())?;
                match func.call::<mlua::Value>(lua_val) {
                    Ok(result) => {
                        // Check if result is a table (UI elements)
                        if let mlua::Value::Table(_) = &result {
                            let json: serde_json::Value =
                                mlua::LuaSerdeExt::from_value(lua, result)
                                    .map_err(|e| e.to_string())?;
                            if let Ok(elements) =
                                serde_json::from_value::<Vec<UiElement>>(json)
                            {
                                let ui_state = PluginUiState {
                                    plugin_id: plugin_id.to_string(),
                                    title: instance.manifest.name.clone(),
                                    elements,
                                };
                                instance.ui_state = Some(ui_state.clone());
                                return Ok(Some(ui_state));
                            }
                        }
                        // Also check if on_ui function exists and get updated UI
                        let ui = get_ui_from_lua(lua, plugin_id);
                        if ui.is_some() {
                            instance.ui_state = ui.clone();
                        }
                        Ok(ui)
                    }
                    Err(e) => Err(format!("Action '{}' error: {}", action, e)),
                }
            }
            Err(_) => Err(format!("Action '{}' not found in plugin", action)),
        }
    }

    /// Dispatch a hook event to all registered plugins.
    pub fn dispatch_hook(
        &mut self,
        event: &HookEvent,
        app_handle: Option<&tauri::AppHandle>,
    ) {
        let plugin_ids: Vec<String> = self
            .hook_registry
            .get(&event.event_name)
            .cloned()
            .unwrap_or_default();

        for plugin_id in plugin_ids {
            if let Some(instance) = self.plugins.get_mut(&plugin_id) {
                if let Some(ref lua) = instance.lua {
                    let func_name = format!(
                        "on_{}",
                        event.event_name.replace(':', "_")
                    );
                    if let Ok(func) = lua.globals().get::<mlua::Function>(&*func_name) {
                        let lua_val = mlua::LuaSerdeExt::to_value(lua, &event.data).ok();
                        if let Some(val) = lua_val {
                            if let Err(e) = func.call::<()>(val) {
                                tracing::warn!(
                                    "Plugin '{}' hook '{}' error: {}",
                                    plugin_id,
                                    event.event_name,
                                    e
                                );
                            }
                        }
                    }

                    // Update UI state after hook
                    let ui = get_ui_from_lua(lua, &plugin_id);
                    if ui.is_some() {
                        instance.ui_state = ui.clone();
                        if let Some(handle) = app_handle {
                            let _ = handle.emit("plugin-ui-update", &ui);
                        }
                    }
                }
            }
        }
    }

    /// Get the current UI state for a plugin.
    pub fn get_ui_state(&self, plugin_id: &str) -> Option<PluginUiState> {
        self.plugins
            .get(plugin_id)
            .and_then(|i| i.ui_state.clone())
    }

    /// List all loaded plugins with their info.
    pub fn list_plugins(&self) -> Vec<PluginInfo> {
        self.plugins.values().map(make_plugin_info).collect()
    }

    /// Get config for a plugin.
    pub fn get_plugin_config(&self, plugin_id: &str) -> Option<PluginConfig> {
        self.plugins.get(plugin_id).map(|i| i.config.clone())
    }

    /// Get plugins directory.
    pub fn get_plugins_dir(&self) -> PathBuf {
        self.plugins_dir.clone()
    }

    /// Set plugins directory.
    pub fn set_plugins_dir(&mut self, dir: PathBuf) {
        let _ = std::fs::create_dir_all(&dir);
        self.plugins_dir = dir;
    }
}

fn make_plugin_info(instance: &PluginInstance) -> PluginInfo {
    let has_ui = instance
        .lua
        .as_ref()
        .map(|lua| lua.globals().get::<mlua::Function>("on_ui").is_ok())
        .unwrap_or(false);

    PluginInfo {
        manifest: instance.manifest.clone(),
        status: instance.status.clone(),
        granted_permissions: instance.config.granted_permissions.clone(),
        has_ui,
    }
}

/// Try to call on_ui() in the Lua VM and parse the result as UI elements.
fn get_ui_from_lua(lua: &Lua, plugin_id: &str) -> Option<PluginUiState> {
    let func = lua.globals().get::<mlua::Function>("on_ui").ok()?;
    let result = func.call::<mlua::Value>(()).ok()?;

    if let mlua::Value::Table(ref tbl) = result {
        // Check for title and elements keys
        let title: String = tbl.get("title").unwrap_or_default();
        let elements_val: mlua::Value = tbl.get("elements").ok()?;
        let json: serde_json::Value =
            mlua::LuaSerdeExt::from_value(lua, elements_val).ok()?;
        let elements: Vec<UiElement> = serde_json::from_value(json).ok()?;

        Some(PluginUiState {
            plugin_id: plugin_id.to_string(),
            title,
            elements,
        })
    } else {
        None
    }
}
