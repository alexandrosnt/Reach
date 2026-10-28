/**
 * The Kubernetes workspace's backend: saved clusters (their kubeconfigs stay
 * in the vault and never come back to the page), workloads, logs, YAML and
 * Helm. See src-tauri/src/ipc/k8s_commands.rs.
 */

import { invoke, Channel } from '@tauri-apps/api/core';
import { explainUntrustedRefs } from './sessions';
import type { Environment, Route } from '#lib/ipc/containers.js';

export interface ContextInfo {
	name: string;
	cluster: string;
	server: string;
	namespace: string | null;
	user: string;
}

export interface ClusterView {
	id: string;
	name: string;
	context: string;
	server: string;
	user: string;
	route: Route;
	environment: Environment;
	readOnly: boolean;
	namespace: string | null;
	lastUsedAt: number;
}

export interface OpenedCluster {
	key: string;
	version: string;
	namespace: string;
	readOnly: boolean;
}

export type Kind =
	| 'Pod'
	| 'Deployment'
	| 'StatefulSet'
	| 'DaemonSet'
	| 'Job'
	| 'CronJob'
	| 'Service'
	| 'Ingress'
	| 'ConfigMap'
	| 'Secret'
	| 'PersistentVolumeClaim'
	| 'Node'
	| 'Namespace';

export interface PodRow {
	name: string;
	namespace: string;
	ready: string;
	status: string;
	restarts: number;
	created: string | null;
	node: string;
	containers: string[];
	owner: string | null;
}

export interface WorkloadRow {
	kind: string;
	name: string;
	namespace: string;
	desired: number;
	ready: number;
	available: number;
	created: string | null;
	images: string[];
}

export interface ObjectRow {
	kind: string;
	name: string;
	namespace: string;
	created: string | null;
	summary: string;
}

export interface EventRow {
	kind: string;
	reason: string;
	object: string;
	message: string;
	count: number;
	last: string | null;
	namespace: string;
}

export interface Revision {
	name: string;
	namespace: string;
	revision: number;
	status: string;
	chart: string;
	chartVersion: string;
	appVersion: string;
	updated: string;
	description: string;
}

export interface ReleaseDetail {
	revision: Revision;
	values: string;
	chartValues: string;
	manifest: string;
	notes: string;
}

/** An empty namespace means all of them. */
type Ns = string | null;

export const k8sContexts = (yaml: string) => invoke<{ contexts: ContextInfo[]; current: string | null }>('k8s_contexts', { yaml });
export const k8sClusters = () => invoke<ClusterView[]>('k8s_clusters');
export const k8sClusterSave = (c: {
	id: string;
	name: string;
	yaml: string | null;
	context: string;
	route: Route;
	environment: Environment;
	readOnly: boolean;
	namespace: string | null;
}) => invoke<ClusterView>('k8s_cluster_save', c);
export const k8sClusterDelete = (id: string) => invoke<void>('k8s_cluster_delete', { id });

export const k8sOpen = (clusterId: string) => explainUntrustedRefs(invoke<OpenedCluster>('k8s_open', { clusterId }));
export const k8sClose = (key: string) => invoke<void>('k8s_close', { key });

export const k8sNamespaces = (key: string) => invoke<string[]>('k8s_namespaces', { key });
export const k8sPods = (key: string, namespace: Ns) => invoke<PodRow[]>('k8s_pods', { key, namespace });
export const k8sWorkloads = (key: string, namespace: Ns) => invoke<WorkloadRow[]>('k8s_workloads', { key, namespace });
export const k8sObjects = (key: string, kind: Kind, namespace: Ns) => invoke<ObjectRow[]>('k8s_objects', { key, kind, namespace });
export const k8sEvents = (key: string, namespace: Ns) => invoke<EventRow[]>('k8s_events', { key, namespace });

export const k8sGetYaml = (key: string, kind: Kind, namespace: Ns, name: string) => invoke<string>('k8s_get_yaml', { key, kind, namespace, name });
export const k8sReplaceYaml = (key: string, kind: Kind, namespace: Ns, name: string, yaml: string) =>
	invoke<void>('k8s_replace_yaml', { key, kind, namespace, name, yaml });
export const k8sDelete = (key: string, kind: Kind, namespace: Ns, name: string) => invoke<void>('k8s_delete', { key, kind, namespace, name });
export const k8sScale = (key: string, kind: Kind, namespace: string, name: string, replicas: number) =>
	invoke<void>('k8s_scale', { key, kind, namespace, name, replicas });
export const k8sRestart = (key: string, kind: Kind, namespace: string, name: string) => invoke<void>('k8s_restart', { key, kind, namespace, name });

/** Follow a pod's log; returns a function that stops it. */
export async function k8sLogs(
	key: string,
	namespace: string,
	pod: string,
	container: string | null,
	view: import('#lib/ipc/containers.js').LogView,
	onData: (chunk: string) => void
): Promise<() => void> {
	const streamId = crypto.randomUUID();
	const channel = new Channel<string>();
	channel.onmessage = onData;
	await invoke('k8s_logs', { key, namespace, pod, container, streamId, view, onData: channel });
	return () => void invoke('k8s_logs_stop', { streamId }).catch(() => {});
}

/** Follow files inside a pod's container, as `tail -F` (#101). */
export async function k8sLogsFiles(
	key: string,
	namespace: string,
	pod: string,
	container: string | null,
	paths: string[],
	lines: number,
	onData: (chunk: string) => void
): Promise<() => void> {
	const streamId = crypto.randomUUID();
	const channel = new Channel<string>();
	channel.onmessage = onData;
	await invoke('k8s_logs_files', { key, namespace, pod, container, streamId, paths, lines, onData: channel });
	return () => void invoke('k8s_logs_stop', { streamId }).catch(() => {});
}

export const k8sLogFiles = (key: string, namespace: string, pod: string, container: string | null) =>
	invoke<import('#lib/ipc/containers.js').FoundFile[]>('k8s_log_files', { key, namespace, pod, container });

export const k8sHelmReleases = (key: string, namespace: Ns) => invoke<Revision[]>('k8s_helm_releases', { key, namespace });
export const k8sHelmHistory = (key: string, namespace: string, name: string) => invoke<Revision[]>('k8s_helm_history', { key, namespace, name });
export const k8sHelmDetail = (key: string, namespace: string, name: string, revision: number) =>
	invoke<ReleaseDetail>('k8s_helm_detail', { key, namespace, name, revision });
export const k8sHelmRun = (clusterId: string, action: 'rollback' | 'uninstall', namespace: string, name: string, revision: number | null) =>
	invoke<string>('k8s_helm_run', { clusterId, action, namespace, name, revision });
