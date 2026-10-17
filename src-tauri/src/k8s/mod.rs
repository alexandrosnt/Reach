//! Kubernetes clusters, from a kubeconfig, directly or through SSH.
//!
//! Wherever there is a choice, this does what `kubectl` does, so a cluster
//! behaves the same here as in the terminal:
//! - TLS: the kubeconfig's CA, and only it, when it names one. Through SSH
//!   the API server is reached over a tunnel and the certificate is still
//!   checked against the server's real name (`tls_server_name`).
//! - Pod status: worked out as `kubectl get pods` prints it (waiting and
//!   terminated reasons, Terminating), not the bare phase.
//! - Restart: the annotation `kubectl rollout restart` sets.
//! - Editing YAML: a replace carrying the object's resourceVersion, as
//!   `kubectl edit` sends, so a change made meanwhile is refused, not lost.
//!
//! The cluster's own RBAC decides what the user may do; Reach never asks for
//! more than the kubeconfig's identity has.

pub mod files;
pub mod helm;

use std::collections::HashMap;
use std::sync::Arc;

use futures_util::{AsyncBufReadExt, StreamExt};
use k8s_openapi::api::apps::v1::{DaemonSet, Deployment, StatefulSet};
use k8s_openapi::api::batch::v1::{CronJob, Job};
use k8s_openapi::api::core::v1::{ConfigMap, Event, Namespace, Node, PersistentVolumeClaim, Pod, Secret, Service};
use k8s_openapi::api::networking::v1::Ingress;
use kube::api::{Api, ApiResource, DeleteParams, DynamicObject, ListParams, LogParams, Patch, PatchParams, PostParams};
use kube::config::{KubeConfigOptions, Kubeconfig};
use kube::Client;
use serde::{Deserialize, Serialize};
use tokio::task::JoinHandle;

use crate::db::conn::Forward;

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ContextInfo {
    pub name: String,
    pub cluster: String,
    pub server: String,
    pub namespace: Option<String>,
    pub user: String,
}

/// The contexts a kubeconfig offers, and which one it uses by default.
pub fn contexts(yaml: &str) -> Result<(Vec<ContextInfo>, Option<String>), String> {
    let kc = Kubeconfig::from_yaml(yaml).map_err(|e| format!("Not a kubeconfig: {e}"))?;
    let servers: HashMap<String, String> = kc
        .clusters
        .iter()
        .map(|c| (c.name.clone(), c.cluster.as_ref().and_then(|c| c.server.clone()).unwrap_or_default()))
        .collect();
    let list = kc
        .contexts
        .iter()
        .filter_map(|c| {
            let ctx = c.context.as_ref()?;
            Some(ContextInfo {
                name: c.name.clone(),
                cluster: ctx.cluster.clone(),
                server: servers.get(&ctx.cluster).cloned().unwrap_or_default(),
                namespace: ctx.namespace.clone(),
                user: ctx.user.clone().unwrap_or_default(),
            })
        })
        .collect();
    Ok((list, kc.current_context))
}

/// Host and port of a context's API server, for an SSH tunnel to it.
pub fn server_addr(server: &str) -> Result<(String, u16), String> {
    let uri: http::Uri = server.parse().map_err(|_| format!("Not a server address: {server}"))?;
    let host = uri.host().ok_or_else(|| format!("No host in {server}"))?.trim_matches(['[', ']']).to_string();
    let port = uri.port_u16().unwrap_or(if uri.scheme_str() == Some("http") { 80 } else { 443 });
    Ok((host, port))
}

pub struct Cluster {
    pub client: Client,
    pub context: String,
    pub namespace: String,
    pub version: String,
    /// Set from the saved cluster; every change refuses while it is on.
    pub read_only: bool,
    /// The SSH tunnel to the API server, when there is one; kept as long
    /// as the cluster is open.
    _forward: Option<Forward>,
}

/// Connect to `context` (or the kubeconfig's current one). With `forward`,
/// requests go through its loopback port to the API server.
pub async fn connect(yaml: &str, context: Option<&str>, forward: Option<Forward>) -> Result<Cluster, String> {
    let kc = Kubeconfig::from_yaml(yaml).map_err(|e| format!("Not a kubeconfig: {e}"))?;
    let opts = KubeConfigOptions { context: context.map(String::from), ..Default::default() };
    let mut config = kube::Config::from_custom_kubeconfig(kc, &opts).await.map_err(|e| e.to_string())?;
    let kc_server = config.cluster_url.to_string();
    if let Some(f) = &forward {
        let real = config.cluster_url.clone();
        let host = real.host().unwrap_or_default().trim_matches(['[', ']']).to_string();
        // The certificate names the real server, not 127.0.0.1.
        if config.tls_server_name.is_none() {
            config.tls_server_name = Some(host);
        }
        let path = real.path_and_query().map(|p| p.as_str()).unwrap_or("/");
        config.cluster_url = format!("{}://127.0.0.1:{}{}", real.scheme_str().unwrap_or("https"), f.port, path)
            .parse()
            .map_err(|e| format!("{e}"))?;
    }
    let namespace = config.default_namespace.clone();
    let context_name = context.map(String::from).unwrap_or_default();
    // Named by the kubeconfig's address, not the tunnel's loopback one.
    let server = kc_server.clone();
    let client = Client::try_from(config).map_err(|e| e.to_string())?;
    let version = client.apiserver_version().await.map_err(|e| match e {
        // kube says "ServiceError: client error (Connect)"; say what failed.
        kube::Error::Service(_) | kube::Error::HyperError(_) => format!("Could not reach the API server at {server}: {e}"),
        other => explain(other),
    })?;
    Ok(Cluster { client, context: context_name, namespace, version: version.git_version, read_only: false, _forward: forward })
}

/// The API server's error, in its own words.
fn explain(e: kube::Error) -> String {
    match e {
        kube::Error::Api(s) => format!("{} ({})", s.message, s.code),
        other => other.to_string(),
    }
}

// ---------------------------------------------------------------------------
// What the pages show

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PodRow {
    pub name: String,
    pub namespace: String,
    pub ready: String,
    pub status: String,
    pub restarts: i32,
    pub created: Option<String>,
    pub node: String,
    pub containers: Vec<String>,
    pub owner: Option<String>,
}

/// The STATUS column of `kubectl get pods`, from the same fields in the
/// same order (kubectl's `printPod`): the pod's reason, then init
/// containers, then each container's waiting or terminated reason, then
/// Terminating if it is being deleted.
pub fn pod_status(p: &Pod) -> String {
    let st = p.status.as_ref();
    let mut reason = st.and_then(|s| s.reason.clone()).or_else(|| st.and_then(|s| s.phase.clone())).unwrap_or_else(|| "Unknown".into());

    let inits = st.and_then(|s| s.init_container_statuses.as_ref());
    let init_total = p.spec.as_ref().and_then(|s| s.init_containers.as_ref()).map_or(0, |v| v.len());
    let mut initializing = false;
    for (i, c) in inits.into_iter().flatten().enumerate() {
        let state = c.state.as_ref();
        if let Some(t) = state.and_then(|s| s.terminated.as_ref()) {
            if t.exit_code == 0 {
                continue;
            }
            reason = match (&t.reason, t.signal) {
                (Some(r), _) if !r.is_empty() => format!("Init:{r}"),
                (_, Some(sig)) if sig != 0 => format!("Init:Signal:{sig}"),
                _ => format!("Init:ExitCode:{}", t.exit_code),
            };
        } else if let Some(w) = state.and_then(|s| s.waiting.as_ref()).filter(|w| w.reason.as_deref().is_some_and(|r| !r.is_empty() && r != "PodInitializing")) {
            reason = format!("Init:{}", w.reason.clone().unwrap_or_default());
        } else {
            reason = format!("Init:{i}/{init_total}");
        }
        initializing = true;
        break;
    }

    if !initializing {
        let mut has_running = false;
        for c in st.and_then(|s| s.container_statuses.as_ref()).into_iter().flatten().rev() {
            let state = c.state.as_ref();
            if let Some(w) = state.and_then(|s| s.waiting.as_ref()).and_then(|w| w.reason.clone()).filter(|r| !r.is_empty()) {
                reason = w;
            } else if let Some(t) = state.and_then(|s| s.terminated.as_ref()) {
                reason = match (&t.reason, t.signal) {
                    (Some(r), _) if !r.is_empty() => r.clone(),
                    (_, Some(sig)) if sig != 0 => format!("Signal:{sig}"),
                    _ => format!("ExitCode:{}", t.exit_code),
                };
            } else if c.ready && state.is_some_and(|s| s.running.is_some()) {
                has_running = true;
            }
        }
        if reason == "Completed" && has_running {
            reason = "Running".into();
        }
    }

    if p.metadata.deletion_timestamp.is_some() {
        reason = if st.and_then(|s| s.reason.as_deref()) == Some("NodeLost") { "Unknown".into() } else { "Terminating".into() };
    }
    reason
}

pub fn pod_row(p: Pod) -> PodRow {
    let statuses = p.status.as_ref().and_then(|s| s.container_statuses.clone()).unwrap_or_default();
    let total = p.spec.as_ref().map_or(0, |s| s.containers.len());
    let ready = statuses.iter().filter(|c| c.ready).count();
    PodRow {
        status: pod_status(&p),
        name: p.metadata.name.clone().unwrap_or_default(),
        namespace: p.metadata.namespace.clone().unwrap_or_default(),
        ready: format!("{ready}/{total}"),
        restarts: statuses.iter().map(|c| c.restart_count).sum(),
        created: p.metadata.creation_timestamp.as_ref().map(|t| t.0.to_string()),
        node: p.spec.as_ref().and_then(|s| s.node_name.clone()).unwrap_or_default(),
        containers: p.spec.as_ref().map(|s| s.containers.iter().map(|c| c.name.clone()).collect()).unwrap_or_default(),
        owner: p.metadata.owner_references.as_ref().and_then(|o| o.first()).map(|o| format!("{}/{}", o.kind, o.name)),
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkloadRow {
    pub kind: String,
    pub name: String,
    pub namespace: String,
    pub desired: i32,
    pub ready: i32,
    pub available: i32,
    pub created: Option<String>,
    pub images: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectRow {
    pub kind: String,
    pub name: String,
    pub namespace: String,
    pub created: Option<String>,
    /// One line on what it is: a service's type and ports, a node's
    /// readiness, a job's completions.
    pub summary: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventRow {
    pub kind: String,
    pub reason: String,
    pub object: String,
    pub message: String,
    pub count: i32,
    pub last: Option<String>,
    pub namespace: String,
}

/// The kinds the workspace lists. Anything else is still reachable as YAML.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum Kind {
    Pod,
    Deployment,
    StatefulSet,
    DaemonSet,
    Job,
    CronJob,
    Service,
    Ingress,
    ConfigMap,
    Secret,
    PersistentVolumeClaim,
    Node,
    Namespace,
}

impl Kind {
    fn resource(self) -> ApiResource {
        match self {
            Kind::Pod => ApiResource::erase::<Pod>(&()),
            Kind::Deployment => ApiResource::erase::<Deployment>(&()),
            Kind::StatefulSet => ApiResource::erase::<StatefulSet>(&()),
            Kind::DaemonSet => ApiResource::erase::<DaemonSet>(&()),
            Kind::Job => ApiResource::erase::<Job>(&()),
            Kind::CronJob => ApiResource::erase::<CronJob>(&()),
            Kind::Service => ApiResource::erase::<Service>(&()),
            Kind::Ingress => ApiResource::erase::<Ingress>(&()),
            Kind::ConfigMap => ApiResource::erase::<ConfigMap>(&()),
            Kind::Secret => ApiResource::erase::<Secret>(&()),
            Kind::PersistentVolumeClaim => ApiResource::erase::<PersistentVolumeClaim>(&()),
            Kind::Node => ApiResource::erase::<Node>(&()),
            Kind::Namespace => ApiResource::erase::<Namespace>(&()),
        }
    }

    fn namespaced(self) -> bool {
        !matches!(self, Kind::Node | Kind::Namespace)
    }

    /// Kinds with replicas, which can be scaled.
    pub fn scalable(self) -> bool {
        matches!(self, Kind::Deployment | Kind::StatefulSet)
    }

    /// Kinds `kubectl rollout restart` works on.
    pub fn restartable(self) -> bool {
        matches!(self, Kind::Deployment | Kind::StatefulSet | Kind::DaemonSet)
    }
}

fn created<K: kube::Resource>(o: &K) -> Option<String> {
    o.meta().creation_timestamp.as_ref().map(|t| t.0.to_string())
}

impl Cluster {
    /// Refuse a change on a read-only cluster, in the backend.
    pub fn writable(&self) -> Result<(), String> {
        if self.read_only {
            Err("This cluster is read-only. Turn that off in its settings to make changes.".into())
        } else {
            Ok(())
        }
    }

    fn api<K>(&self, ns: Option<&str>) -> Api<K>
    where
        K: kube::Resource<Scope = k8s_openapi::NamespaceResourceScope> + Clone + serde::de::DeserializeOwned + std::fmt::Debug,
        K::DynamicType: Default,
    {
        match ns {
            Some(ns) => Api::namespaced(self.client.clone(), ns),
            None => Api::all(self.client.clone()),
        }
    }

    fn dynamic(&self, kind: Kind, ns: Option<&str>) -> Api<DynamicObject> {
        let ar = kind.resource();
        match (kind.namespaced(), ns) {
            (true, Some(ns)) => Api::namespaced_with(self.client.clone(), ns, &ar),
            _ => Api::all_with(self.client.clone(), &ar),
        }
    }

    pub async fn namespaces(&self) -> Result<Vec<String>, String> {
        let list = Api::<Namespace>::all(self.client.clone()).list(&ListParams::default()).await.map_err(explain)?;
        let mut names: Vec<String> = list.items.into_iter().filter_map(|n| n.metadata.name).collect();
        names.sort();
        Ok(names)
    }

    pub async fn pods(&self, ns: Option<&str>) -> Result<Vec<PodRow>, String> {
        let list = self.api::<Pod>(ns).list(&ListParams::default()).await.map_err(explain)?;
        Ok(list.items.into_iter().map(pod_row).collect())
    }

    pub async fn workloads(&self, ns: Option<&str>) -> Result<Vec<WorkloadRow>, String> {
        let lp = ListParams::default();
        let mut rows = Vec::new();
        let images = |s: Option<&k8s_openapi::api::core::v1::PodSpec>| -> Vec<String> {
            s.map(|s| s.containers.iter().filter_map(|c| c.image.clone()).collect()).unwrap_or_default()
        };
        for d in self.api::<Deployment>(ns).list(&lp).await.map_err(explain)?.items {
            let st = d.status.clone().unwrap_or_default();
            rows.push(WorkloadRow {
                kind: "Deployment".into(),
                created: created(&d),
                name: d.metadata.name.clone().unwrap_or_default(),
                namespace: d.metadata.namespace.clone().unwrap_or_default(),
                desired: d.spec.as_ref().and_then(|s| s.replicas).unwrap_or(1),
                ready: st.ready_replicas.unwrap_or(0),
                available: st.available_replicas.unwrap_or(0),
                images: images(d.spec.as_ref().and_then(|s| s.template.spec.as_ref())),
            });
        }
        for s in self.api::<StatefulSet>(ns).list(&lp).await.map_err(explain)?.items {
            let st = s.status.clone().unwrap_or_default();
            rows.push(WorkloadRow {
                kind: "StatefulSet".into(),
                created: created(&s),
                name: s.metadata.name.clone().unwrap_or_default(),
                namespace: s.metadata.namespace.clone().unwrap_or_default(),
                desired: s.spec.as_ref().and_then(|s| s.replicas).unwrap_or(1),
                ready: st.ready_replicas.unwrap_or(0),
                available: st.available_replicas.unwrap_or(0),
                images: images(s.spec.as_ref().and_then(|s| s.template.spec.as_ref())),
            });
        }
        for d in self.api::<DaemonSet>(ns).list(&lp).await.map_err(explain)?.items {
            let st = d.status.clone().unwrap_or_default();
            rows.push(WorkloadRow {
                kind: "DaemonSet".into(),
                created: created(&d),
                name: d.metadata.name.clone().unwrap_or_default(),
                namespace: d.metadata.namespace.clone().unwrap_or_default(),
                desired: st.desired_number_scheduled,
                ready: st.number_ready,
                available: st.number_available.unwrap_or(0),
                images: images(d.spec.as_ref().and_then(|s| s.template.spec.as_ref())),
            });
        }
        Ok(rows)
    }

    /// Any listed kind as name, namespace, age and a one-line summary.
    pub async fn objects(&self, kind: Kind, ns: Option<&str>) -> Result<Vec<ObjectRow>, String> {
        let lp = ListParams::default();
        let row = |kind: &str, m: &kube::api::ObjectMeta, summary: String| ObjectRow {
            kind: kind.into(),
            name: m.name.clone().unwrap_or_default(),
            namespace: m.namespace.clone().unwrap_or_default(),
            created: m.creation_timestamp.as_ref().map(|t| t.0.to_string()),
            summary,
        };
        Ok(match kind {
            Kind::Service => self
                .api::<Service>(ns)
                .list(&lp)
                .await
                .map_err(explain)?
                .items
                .iter()
                .map(|s| {
                    let spec = s.spec.clone().unwrap_or_default();
                    let ports: Vec<String> = spec
                        .ports
                        .unwrap_or_default()
                        .iter()
                        .map(|p| match p.node_port {
                            Some(np) => format!("{}:{}/{}", p.port, np, p.protocol.clone().unwrap_or_else(|| "TCP".into())),
                            None => format!("{}/{}", p.port, p.protocol.clone().unwrap_or_else(|| "TCP".into())),
                        })
                        .collect();
                    let summary = format!(
                        "{} {} {}",
                        spec.type_.unwrap_or_else(|| "ClusterIP".into()),
                        spec.cluster_ip.unwrap_or_default(),
                        ports.join(",")
                    );
                    row("Service", &s.metadata, summary.trim().to_string())
                })
                .collect(),
            Kind::Node => Api::<Node>::all(self.client.clone())
                .list(&lp)
                .await
                .map_err(explain)?
                .items
                .iter()
                .map(|n| {
                    let st = n.status.clone().unwrap_or_default();
                    let ready = st
                        .conditions
                        .unwrap_or_default()
                        .iter()
                        .find(|c| c.type_ == "Ready")
                        .map(|c| if c.status == "True" { "Ready" } else { "NotReady" })
                        .unwrap_or("Unknown");
                    let version = st.node_info.map(|i| i.kubelet_version).unwrap_or_default();
                    row("Node", &n.metadata, format!("{ready} {version}"))
                })
                .collect(),
            Kind::Job => self
                .api::<Job>(ns)
                .list(&lp)
                .await
                .map_err(explain)?
                .items
                .iter()
                .map(|j| {
                    let done = j.status.as_ref().and_then(|s| s.succeeded).unwrap_or(0);
                    let want = j.spec.as_ref().and_then(|s| s.completions).unwrap_or(1);
                    row("Job", &j.metadata, format!("{done}/{want} complete"))
                })
                .collect(),
            Kind::CronJob => self
                .api::<CronJob>(ns)
                .list(&lp)
                .await
                .map_err(explain)?
                .items
                .iter()
                .map(|c| {
                    let spec = c.spec.clone().unwrap_or_default();
                    let paused = if spec.suspend == Some(true) { " (suspended)" } else { "" };
                    row("CronJob", &c.metadata, format!("{}{paused}", spec.schedule))
                })
                .collect(),
            other => {
                // Secrets are listed by name only; their values are never read
                // for a list.
                let api = self.dynamic(other, ns);
                let list = api.list_metadata(&lp).await.map_err(explain)?;
                let name = format!("{other:?}");
                list.items.iter().map(|o| row(&name, &o.metadata, String::new())).collect()
            }
        })
    }

    pub async fn events(&self, ns: Option<&str>) -> Result<Vec<EventRow>, String> {
        let list = self.api::<Event>(ns).list(&ListParams::default()).await.map_err(explain)?;
        let mut rows: Vec<EventRow> = list
            .items
            .into_iter()
            .map(|e| EventRow {
                kind: e.type_.unwrap_or_default(),
                reason: e.reason.unwrap_or_default(),
                object: format!("{}/{}", e.involved_object.kind.unwrap_or_default(), e.involved_object.name.unwrap_or_default()),
                message: e.message.unwrap_or_default(),
                count: e.count.unwrap_or(1),
                last: e.last_timestamp.map(|t| t.0.to_string()).or_else(|| e.event_time.map(|t| t.0.to_string())),
                namespace: e.metadata.namespace.unwrap_or_default(),
            })
            .collect();
        rows.sort_by(|a, b| b.last.cmp(&a.last));
        Ok(rows)
    }

    /// An object as YAML, without the server's bookkeeping (managedFields).
    pub async fn get_yaml(&self, kind: Kind, ns: Option<&str>, name: &str) -> Result<String, String> {
        let mut obj = self.dynamic(kind, ns).get(name).await.map_err(explain)?;
        obj.metadata.managed_fields = None;
        serde_saphyr::to_string(&obj).map_err(|e| e.to_string())
    }

    /// Save edited YAML: a replace carrying the resourceVersion it was read
    /// with, so it is refused if the object changed in the meantime.
    pub async fn replace_yaml(&self, kind: Kind, ns: Option<&str>, name: &str, yaml: &str) -> Result<(), String> {
        self.writable()?;
        let obj: DynamicObject = serde_saphyr::from_str(yaml).map_err(|e| format!("Not valid YAML: {e}"))?;
        if obj.metadata.name.as_deref() != Some(name) {
            return Err("The name cannot be changed here; that would be a different object.".into());
        }
        if obj.metadata.resource_version.is_none() {
            return Err("metadata.resourceVersion is missing; it is how a change made meanwhile is detected.".into());
        }
        self.dynamic(kind, ns).replace(name, &PostParams::default(), &obj).await.map(|_| ()).map_err(explain)
    }

    pub async fn delete(&self, kind: Kind, ns: Option<&str>, name: &str) -> Result<(), String> {
        self.writable()?;
        self.dynamic(kind, ns).delete(name, &DeleteParams::default()).await.map(|_| ()).map_err(explain)
    }

    pub async fn scale(&self, kind: Kind, ns: &str, name: &str, replicas: i32) -> Result<(), String> {
        self.writable()?;
        if !kind.scalable() {
            return Err(format!("A {kind:?} cannot be scaled"));
        }
        if replicas < 0 {
            return Err("Replicas cannot be negative".into());
        }
        let patch = serde_json::json!({ "spec": { "replicas": replicas } });
        self.dynamic(kind, Some(ns)).patch(name, &PatchParams::default(), &Patch::Merge(&patch)).await.map(|_| ()).map_err(explain)
    }

    /// `kubectl rollout restart`: a new template annotation, so the
    /// controller replaces the pods one by one under its own rollout rules.
    pub async fn restart(&self, kind: Kind, ns: &str, name: &str) -> Result<(), String> {
        self.writable()?;
        if !kind.restartable() {
            return Err(format!("A {kind:?} cannot be restarted"));
        }
        let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let patch = serde_json::json!({
            "spec": { "template": { "metadata": { "annotations": { "kubectl.kubernetes.io/restartedAt": now } } } }
        });
        self.dynamic(kind, Some(ns)).patch(name, &PatchParams::default(), &Patch::Strategic(&patch)).await.map(|_| ()).map_err(explain)
    }
}

/// Open clusters and running log streams.
#[derive(Default)]
pub struct K8sManager {
    clusters: HashMap<String, Arc<Cluster>>,
    streams: HashMap<String, JoinHandle<()>>,
}

impl K8sManager {
    pub fn get(&self, key: &str) -> Result<Arc<Cluster>, String> {
        self.clusters.get(key).cloned().ok_or_else(|| "Not connected to that cluster; open it again.".to_string())
    }

    pub fn insert(&mut self, key: String, c: Cluster) {
        self.clusters.insert(key, Arc::new(c));
    }

    pub fn close(&mut self, key: &str) {
        self.clusters.remove(key);
    }

    pub fn close_all(&mut self) {
        self.clusters.clear();
        for (_, t) in self.streams.drain() {
            t.abort();
        }
    }

    /// Follow a pod container's log into `out`, as `kubectl logs -f`.
    #[allow(clippy::too_many_arguments)]
    pub fn follow_logs(
        &mut self,
        stream_id: String,
        cluster: Arc<Cluster>,
        ns: String,
        pod: String,
        container: Option<String>,
        view: crate::container::LogView,
        out: tauri::ipc::Channel<String>,
    ) {
        if let Some(old) = self.streams.remove(&stream_id) {
            old.abort();
        }
        let task = tokio::spawn(async move {
            let api: Api<Pod> = Api::namespaced(cluster.client.clone(), &ns);
            // With --since, everything since then: a count would cut it.
            let lp = LogParams {
                follow: true,
                container,
                tail_lines: if view.since_secs.is_some() { None } else { Some(view.tail as i64) },
                since_seconds: view.since_secs.map(i64::from),
                timestamps: view.timestamps,
                ..Default::default()
            };
            match api.log_stream(&pod, &lp).await {
                Ok(stream) => {
                    let mut lines = stream.lines();
                    while let Some(line) = lines.next().await {
                        match line {
                            Ok(l) => {
                                if out.send(crate::logfiles::data(format!("{l}\r\n"))).is_err() {
                                    break;
                                }
                            }
                            Err(e) => {
                                let _ = out.send(crate::logfiles::note(&e.to_string()));
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    let _ = out.send(crate::logfiles::note(&explain(e)));
                }
            }
        });
        self.streams.insert(stream_id, task);
    }

    pub fn stop_stream(&mut self, stream_id: &str) {
        if let Some(t) = self.streams.remove(stream_id) {
            t.abort();
        }
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
