//! Saved container hosts and Kubernetes clusters, in their own internal
//! vault: encrypted at rest and carried to every device byte for byte, like
//! saved database connections. A kubeconfig holds credentials, so it goes
//! into the vault on import and is read back by the backend only; the
//! webview gets the cluster's description, never the file.

use std::collections::HashMap;

use secrecy::{ExposeSecret, SecretBox};
use serde::{Deserialize, Serialize};

use crate::container::Engine;
use crate::db::types::Route;
use crate::logfiles::LogRule;
use crate::vault::manager::DEVOPS_VAULT;
use crate::vault::types::{SecretCategory, VaultType};
use crate::vault::VaultManager;

/// What a target is for. Production is shown in red everywhere, asks for
/// typed confirmation, and starts read-only.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Environment {
    #[default]
    None,
    Development,
    Staging,
    Production,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ContainerHost {
    pub id: String,
    pub name: String,
    pub route: Route,
    pub engine: Engine,
    #[serde(default)]
    pub environment: Environment,
    /// Changes are refused by the backend, not only hidden.
    #[serde(default)]
    pub read_only: bool,
    #[serde(default)]
    pub last_used_at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Cluster {
    pub id: String,
    pub name: String,
    pub kubeconfig: String,
    pub context: String,
    pub route: Route,
    #[serde(default)]
    pub environment: Environment,
    #[serde(default)]
    pub read_only: bool,
    #[serde(default)]
    pub namespace: Option<String>,
    #[serde(default)]
    pub last_used_at: u64,
}

/// A cluster as the webview sees it: everything but the kubeconfig.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ClusterView {
    pub id: String,
    pub name: String,
    pub context: String,
    pub server: String,
    pub user: String,
    pub route: Route,
    pub environment: Environment,
    pub read_only: bool,
    pub namespace: Option<String>,
    pub last_used_at: u64,
}

impl Cluster {
    pub fn view(&self) -> ClusterView {
        let ctx = crate::k8s::contexts(&self.kubeconfig)
            .ok()
            .and_then(|(list, _)| list.into_iter().find(|c| c.name == self.context));
        ClusterView {
            id: self.id.clone(),
            name: self.name.clone(),
            context: self.context.clone(),
            server: ctx.as_ref().map(|c| c.server.clone()).unwrap_or_default(),
            user: ctx.map(|c| c.user).unwrap_or_default(),
            route: self.route.clone(),
            environment: self.environment,
            read_only: self.read_only,
            namespace: self.namespace.clone(),
            last_used_at: self.last_used_at,
        }
    }
}

/// The log files saved for a host's or cluster's containers (#101). An entry
/// of its own, so the host's entry is never rewritten for it, and an older
/// Reach, which skips entries it does not know, leaves it alone.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LogSources {
    pub owner: String,
    pub rules: Vec<LogRule>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum Entry {
    Host(ContainerHost),
    Cluster(Cluster),
    Logs(LogSources),
}

#[derive(Default)]
pub struct DevopsStore {
    hosts: HashMap<String, ContainerHost>,
    clusters: HashMap<String, Cluster>,
    logs: HashMap<String, Vec<LogRule>>,
    loaded: bool,
}

fn logs_id(owner: &str) -> String {
    format!("logs-{owner}")
}

impl DevopsStore {
    /// Load once the vault is unlocked; a locked vault means an empty list.
    pub async fn ensure_loaded(&mut self, vault: &mut VaultManager) -> Result<(), String> {
        if self.loaded || vault.is_locked() {
            return Ok(());
        }
        let vault_id = ensure_vault(vault).await?;
        for secret in vault.list_secrets(&vault_id).await.map_err(|e| e.to_string())? {
            let Ok(plain) = vault.read_secret(&vault_id, &secret.id).await else { continue };
            match serde_json::from_slice::<Entry>(plain.expose_secret()) {
                Ok(Entry::Host(h)) => {
                    self.hosts.insert(h.id.clone(), h);
                }
                Ok(Entry::Cluster(c)) => {
                    self.clusters.insert(c.id.clone(), c);
                }
                Ok(Entry::Logs(l)) => {
                    self.logs.insert(l.owner, l.rules);
                }
                Err(_) => {}
            }
        }
        self.loaded = true;
        Ok(())
    }

    pub fn hosts(&self) -> Vec<ContainerHost> {
        let mut all: Vec<_> = self.hosts.values().cloned().collect();
        all.sort_by(|a, b| b.last_used_at.cmp(&a.last_used_at).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
        all
    }

    pub fn host(&self, id: &str) -> Option<&ContainerHost> {
        self.hosts.get(id)
    }

    pub fn clusters(&self) -> Vec<ClusterView> {
        let mut all: Vec<_> = self.clusters.values().map(Cluster::view).collect();
        all.sort_by(|a, b| b.last_used_at.cmp(&a.last_used_at).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
        all
    }

    pub fn cluster(&self, id: &str) -> Option<&Cluster> {
        self.clusters.get(id)
    }

    pub async fn save_host(&mut self, h: ContainerHost, vault: &mut VaultManager) -> Result<(), String> {
        let name = h.name.clone();
        let id = h.id.clone();
        self.put(&id, &name, "container_host", &Entry::Host(h.clone()), vault).await?;
        self.hosts.insert(id, h);
        Ok(())
    }

    pub async fn save_cluster(&mut self, c: Cluster, vault: &mut VaultManager) -> Result<(), String> {
        let name = c.name.clone();
        let id = c.id.clone();
        self.put(&id, &name, "k8s_cluster", &Entry::Cluster(c.clone()), vault).await?;
        self.clusters.insert(id, c);
        Ok(())
    }

    /// Insert or replace. The vault has no update; delete and re-create, as
    /// the other stores do.
    async fn put(&self, id: &str, name: &str, category: &str, entry: &Entry, vault: &mut VaultManager) -> Result<(), String> {
        let vault_id = ensure_vault(vault).await?;
        let json = serde_json::to_vec(entry).map_err(|e| e.to_string())?;
        let _ = vault.delete_secret(&vault_id, id).await;
        vault
            .create_secret_with_id(&vault_id, id, name, SecretCategory::Custom(category.into()), SecretBox::new(Box::new(json)))
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    pub async fn delete(&mut self, id: &str, vault: &mut VaultManager) -> Result<(), String> {
        let vault_id = ensure_vault(vault).await?;
        vault.delete_secret(&vault_id, id).await.map_err(|e| e.to_string())?;
        // Its saved log files go with it.
        if self.logs.remove(id).is_some() {
            let _ = vault.delete_secret(&vault_id, &logs_id(id)).await;
        }
        self.hosts.remove(id);
        self.clusters.remove(id);
        Ok(())
    }

    /// The log files saved for a host's or cluster's containers.
    pub fn log_rules(&self, owner: &str) -> Vec<LogRule> {
        self.logs.get(owner).cloned().unwrap_or_default()
    }

    /// Replace them; an empty list removes the entry.
    pub async fn save_log_rules(&mut self, owner: &str, rules: Vec<LogRule>, vault: &mut VaultManager) -> Result<(), String> {
        if !self.hosts.contains_key(owner) && !self.clusters.contains_key(owner) {
            return Err("That host is no longer saved".into());
        }
        let id = logs_id(owner);
        if rules.is_empty() {
            let vault_id = ensure_vault(vault).await?;
            let _ = vault.delete_secret(&vault_id, &id).await;
            self.logs.remove(owner);
            return Ok(());
        }
        let entry = Entry::Logs(LogSources { owner: owner.to_string(), rules: rules.clone() });
        self.put(&id, "Log files", "container_logs", &entry, vault).await?;
        self.logs.insert(owner.to_string(), rules);
        Ok(())
    }
}

async fn ensure_vault(vault: &mut VaultManager) -> Result<String, String> {
    if let Some(id) = vault.get_vault_id_by_name(DEVOPS_VAULT) {
        let _ = vault.open_vault(&id, None, None).await;
        vault.unlock_vault(&id).await.map_err(|e| e.to_string())?;
        return Ok(id);
    }
    let created = vault.create_vault(DEVOPS_VAULT, VaultType::Private, None, None).await.map_err(|e| e.to_string())?;
    Ok(created.id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_is_spelled_as_the_frontend_sends_it() {
        let e: Environment = serde_json::from_str("\"production\"").unwrap();
        assert_eq!(e, Environment::Production);
        assert_eq!(Environment::default(), Environment::None);
    }

    #[test]
    fn the_view_never_carries_the_kubeconfig() {
        let c = Cluster {
            id: "1".into(),
            name: "prod".into(),
            kubeconfig: "apiVersion: v1\nkind: Config\nclusters:\n- name: c\n  cluster: {server: 'https://k:6443'}\ncontexts:\n- name: x\n  context: {cluster: c, user: u}\nusers:\n- name: u\n  user: {token: SECRET-TOKEN}\n".into(),
            context: "x".into(),
            route: Route::Direct,
            environment: Environment::Production,
            read_only: true,
            namespace: None,
            last_used_at: 0,
        };
        let v = c.view();
        assert_eq!(v.server, "https://k:6443");
        let json = serde_json::to_string(&v).unwrap();
        assert!(!json.contains("SECRET-TOKEN") && !json.contains("kubeconfig"), "{json}");
    }

    #[test]
    fn saved_entries_round_trip_with_their_type() {
        let h = ContainerHost {
            id: "h".into(),
            name: "web".into(),
            route: Route::Session { session_id: "s".into() },
            engine: Engine::Podman,
            environment: Environment::Staging,
            read_only: false,
            last_used_at: 7,
        };
        let json = serde_json::to_vec(&Entry::Host(h.clone())).unwrap();
        match serde_json::from_slice::<Entry>(&json).unwrap() {
            Entry::Host(back) => assert_eq!(back, h),
            _ => panic!("came back as something else"),
        }
    }

    #[test]
    fn saved_log_files_round_trip_and_an_older_reach_skips_them() {
        use crate::logfiles::Scope;
        let l = LogSources {
            owner: "h".into(),
            rules: vec![LogRule { scope: Scope::Service { project: "shop".into(), service: "api".into() }, paths: vec!["/logs/api.INFO".into()], label: None }],
        };
        let json = serde_json::to_vec(&Entry::Logs(l.clone())).unwrap();
        match serde_json::from_slice::<Entry>(&json).unwrap() {
            Entry::Logs(back) => assert_eq!(back, l),
            _ => panic!("came back as something else"),
        }
        // An older Reach knows only hosts and clusters: the entry fails to
        // parse there, and ensure_loaded skips what fails.
        #[derive(Deserialize)]
        #[serde(tag = "type", rename_all = "lowercase")]
        #[allow(dead_code)]
        enum Older {
            Host(ContainerHost),
            Cluster(Cluster),
        }
        assert!(serde_json::from_slice::<Older>(&json).is_err());
    }
}
