use std::collections::HashMap;
use std::path::Path;

use crate::ansible::types::AnsibleProject;
use crate::vault::manager::ANSIBLE_PROJECTS_VAULT;
use crate::vault::types::{SecretCategory, VaultType};
use crate::vault::VaultManager;
use secrecy::{ExposeSecret, SecretBox};

/// Manages Ansible project metadata.
pub struct AnsibleProjectManager {
    /// In-memory cache of projects, loaded from vault on first access.
    projects: HashMap<String, AnsibleProject>,
    loaded: bool,
}

impl AnsibleProjectManager {
    pub fn new() -> Self {
        Self {
            projects: HashMap::new(),
            loaded: false,
        }
    }

    /// Ensure projects are loaded from vault into memory cache.
    pub async fn ensure_loaded(
        &mut self,
        vault_manager: &mut VaultManager,
    ) -> Result<(), String> {
        if self.loaded {
            return Ok(());
        }
        self.load_from_vault(vault_manager).await?;
        self.loaded = true;
        Ok(())
    }

    /// Load all projects from vault into in-memory cache.
    async fn load_from_vault(
        &mut self,
        vault_manager: &mut VaultManager,
    ) -> Result<(), String> {
        if vault_manager.is_locked() {
            return Ok(());
        }

        let vault_id = match ensure_ansible_vault(vault_manager).await {
            Ok(id) => id,
            Err(_) => return Ok(()), // vault not available yet
        };

        let secrets = vault_manager
            .list_secrets(&vault_id)
            .await
            .map_err(|e| e.to_string())?;

        for secret in secrets {
            if let Ok(plaintext) = vault_manager.read_secret(&vault_id, &secret.id).await {
                if let Ok(json) = String::from_utf8(plaintext.expose_secret().clone()) {
                    if let Ok(project) = serde_json::from_str::<AnsibleProject>(&json) {
                        self.projects.insert(project.id.clone(), project);
                    }
                }
            }
        }

        Ok(())
    }

    /// Add a new project to cache and persist to vault.
    pub async fn add_project(
        &mut self,
        project: AnsibleProject,
        vault_manager: &mut VaultManager,
    ) -> Result<(), String> {
        let vault_id = ensure_ansible_vault(vault_manager).await?;
        let json = serde_json::to_string(&project).map_err(|e| e.to_string())?;
        vault_manager
            .create_secret_with_id(
                &vault_id,
                &project.id,
                &project.name,
                SecretCategory::Custom("ansible_project".to_string()),
                SecretBox::new(Box::new(json.into_bytes())),
            )
            .await
            .map_err(|e| e.to_string())?;
        self.projects.insert(project.id.clone(), project);
        Ok(())
    }

    /// Remove a project from cache and vault.
    pub async fn remove_project(
        &mut self,
        project_id: &str,
        vault_manager: &mut VaultManager,
    ) -> Result<(), String> {
        let vault_id = ensure_ansible_vault(vault_manager).await?;
        vault_manager
            .delete_secret(&vault_id, project_id)
            .await
            .map_err(|e| e.to_string())?;
        self.projects.remove(project_id);
        Ok(())
    }

    /// Get a project by ID.
    pub fn get_project(&self, project_id: &str) -> Option<&AnsibleProject> {
        self.projects.get(project_id)
    }

    /// List all projects.
    pub fn list_projects(&self) -> Vec<AnsibleProject> {
        let mut projects: Vec<AnsibleProject> = self.projects.values().cloned().collect();
        projects.sort_by(|a, b| b.last_opened_at.cmp(&a.last_opened_at));
        projects
    }

    /// Update a project entirely in cache and vault (delete + re-create pattern).
    pub async fn update_project(
        &mut self,
        project: AnsibleProject,
        vault_manager: &mut VaultManager,
    ) -> Result<(), String> {
        let vault_id = ensure_ansible_vault(vault_manager).await?;
        let json = serde_json::to_string(&project).map_err(|e| e.to_string())?;
        let _ = vault_manager.delete_secret(&vault_id, &project.id).await;
        vault_manager
            .create_secret_with_id(
                &vault_id,
                &project.id,
                &project.name,
                SecretCategory::Custom("ansible_project".to_string()),
                SecretBox::new(Box::new(json.into_bytes())),
            )
            .await
            .map_err(|e| e.to_string())?;
        self.projects.insert(project.id.clone(), project);
        Ok(())
    }

    /// Update a project's last_opened_at timestamp in cache and vault.
    pub async fn touch_project(
        &mut self,
        project_id: &str,
        vault_manager: &mut VaultManager,
    ) -> Result<(), String> {
        let project = self
            .projects
            .get_mut(project_id)
            .ok_or_else(|| "Project not found".to_string())?;

        project.last_opened_at = now_iso8601();

        let vault_id = ensure_ansible_vault(vault_manager).await?;
        let json = serde_json::to_string(&project).map_err(|e| e.to_string())?;
        // Delete old then re-create (update pattern)
        let _ = vault_manager.delete_secret(&vault_id, project_id).await;
        vault_manager
            .create_secret_with_id(
                &vault_id,
                project_id,
                &project.name,
                SecretCategory::Custom("ansible_project".to_string()),
                SecretBox::new(Box::new(json.into_bytes())),
            )
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Scaffold a new project directory with starter files.
    pub fn scaffold_project(path: &str, name: &str) -> Result<(), String> {
        let dir = Path::new(path);
        std::fs::create_dir_all(dir).map_err(|e| format!("Failed to create directory: {}", e))?;

        let site_yml = dir.join("site.yml");
        if !site_yml.exists() {
            let content = format!(
                r#"---
# Ansible project: {}
# Main playbook

- name: Configure all hosts
  hosts: all
  become: true
  tasks:
    - name: Ping all hosts
      ansible.builtin.ping:
"#,
                name
            );
            std::fs::write(&site_yml, content)
                .map_err(|e| format!("Failed to write site.yml: {}", e))?;
        }

        let inventory = dir.join("inventory.ini");
        if !inventory.exists() {
            let content = r#"# Ansible inventory file
# Add your hosts and groups below

[all]
# host1 ansible_host=192.168.1.10 ansible_user=admin
"#;
            std::fs::write(&inventory, content)
                .map_err(|e| format!("Failed to write inventory.ini: {}", e))?;
        }

        Ok(())
    }
}

/// Get current time as ISO 8601 string.
fn now_iso8601() -> String {
    let dur = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();
    let secs = dur.as_secs();
    // Convert to ISO 8601 manually (UTC)
    let days = secs / 86400;
    let time_secs = secs % 86400;
    let hours = time_secs / 3600;
    let minutes = (time_secs % 3600) / 60;
    let seconds = time_secs % 60;

    // Days since epoch to Y-M-D (simplified Gregorian)
    let mut y = 1970i64;
    let mut remaining = days as i64;
    loop {
        let year_days = if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) { 366 } else { 365 };
        if remaining < year_days { break; }
        remaining -= year_days;
        y += 1;
    }
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let month_days = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut m = 0usize;
    for &md in &month_days {
        if remaining < md { break; }
        remaining -= md;
        m += 1;
    }
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, m + 1, remaining + 1, hours, minutes, seconds)
}

/// Ensure the __ansible_projects__ vault exists, creating if needed.
async fn ensure_ansible_vault(manager: &mut VaultManager) -> Result<String, String> {
    if let Some(vault_id) = manager.get_vault_id_by_name(ANSIBLE_PROJECTS_VAULT) {
        let _ = manager.open_vault(&vault_id, None, None).await;
        manager
            .unlock_vault(&vault_id)
            .await
            .map_err(|e| e.to_string())?;
        Ok(vault_id)
    } else {
        let vault = manager
            .create_vault(ANSIBLE_PROJECTS_VAULT, VaultType::Private, None, None)
            .await
            .map_err(|e| e.to_string())?;
        Ok(vault.id)
    }
}
