/**
 * Snippets state module - predefined command snippets for DevOps tools.
 * O(1) lookup by category using Map.
 */

export type SnippetCategory = 'terraform' | 'docker' | 'git' | 'kubernetes' | 'system';

export interface Snippet {
	id: string;
	name: string;
	description: string;
	command: string;
	category: SnippetCategory;
}

/** All snippets indexed by ID for O(1) lookup */
const snippetsById = new Map<string, Snippet>();

/** Snippets grouped by category for O(1) category lookup */
const snippetsByCategory = new Map<SnippetCategory, Snippet[]>();

/** Docker snippets */
const dockerSnippets: Snippet[] = [
	{
		id: 'docker-ps',
		name: 'List Containers',
		description: 'List running containers',
		command: 'docker ps',
		category: 'docker'
	},
	{
		id: 'docker-ps-all',
		name: 'List All Containers',
		description: 'List all containers including stopped',
		command: 'docker ps -a',
		category: 'docker'
	},
	{
		id: 'docker-images',
		name: 'List Images',
		description: 'List local images',
		command: 'docker images',
		category: 'docker'
	},
	{
		id: 'docker-pull',
		name: 'Pull Image',
		description: 'Pull image from registry',
		command: 'docker pull ',
		category: 'docker'
	},
	{
		id: 'docker-build',
		name: 'Build Image',
		description: 'Build image from Dockerfile',
		command: 'docker build -t ',
		category: 'docker'
	},
	{
		id: 'docker-run',
		name: 'Run Container',
		description: 'Run a container',
		command: 'docker run -it ',
		category: 'docker'
	},
	{
		id: 'docker-run-d',
		name: 'Run Detached',
		description: 'Run container in background',
		command: 'docker run -d ',
		category: 'docker'
	},
	{
		id: 'docker-exec',
		name: 'Exec into Container',
		description: 'Execute command in running container',
		command: 'docker exec -it ',
		category: 'docker'
	},
	{
		id: 'docker-logs',
		name: 'View Logs',
		description: 'View container logs',
		command: 'docker logs -f ',
		category: 'docker'
	},
	{
		id: 'docker-stop',
		name: 'Stop Container',
		description: 'Stop a running container',
		command: 'docker stop ',
		category: 'docker'
	},
	{
		id: 'docker-rm',
		name: 'Remove Container',
		description: 'Remove a container',
		command: 'docker rm ',
		category: 'docker'
	},
	{
		id: 'docker-rmi',
		name: 'Remove Image',
		description: 'Remove an image',
		command: 'docker rmi ',
		category: 'docker'
	},
	{
		id: 'docker-compose-up',
		name: 'Compose Up',
		description: 'Start services with compose',
		command: 'docker compose up -d',
		category: 'docker'
	},
	{
		id: 'docker-compose-down',
		name: 'Compose Down',
		description: 'Stop and remove compose services',
		command: 'docker compose down',
		category: 'docker'
	},
	{
		id: 'docker-compose-logs',
		name: 'Compose Logs',
		description: 'View compose service logs',
		command: 'docker compose logs -f',
		category: 'docker'
	},
	{
		id: 'docker-system-prune',
		name: 'System Prune',
		description: 'Remove unused data',
		command: 'docker system prune -f',
		category: 'docker'
	},
	{
		id: 'docker-volume-ls',
		name: 'List Volumes',
		description: 'List volumes',
		command: 'docker volume ls',
		category: 'docker'
	},
	{
		id: 'docker-network-ls',
		name: 'List Networks',
		description: 'List networks',
		command: 'docker network ls',
		category: 'docker'
	}
];

/** Git snippets */
const gitSnippets: Snippet[] = [
	{
		id: 'git-status',
		name: 'Status',
		description: 'Show working tree status',
		command: 'git status',
		category: 'git'
	},
	{
		id: 'git-log',
		name: 'Log',
		description: 'Show commit history',
		command: 'git log --oneline -20',
		category: 'git'
	},
	{
		id: 'git-diff',
		name: 'Diff',
		description: 'Show changes',
		command: 'git diff',
		category: 'git'
	},
	{
		id: 'git-branch',
		name: 'List Branches',
		description: 'List local branches',
		command: 'git branch',
		category: 'git'
	},
	{
		id: 'git-branch-all',
		name: 'All Branches',
		description: 'List all branches',
		command: 'git branch -a',
		category: 'git'
	},
	{
		id: 'git-checkout',
		name: 'Checkout',
		description: 'Switch branches',
		command: 'git checkout ',
		category: 'git'
	},
	{
		id: 'git-checkout-b',
		name: 'New Branch',
		description: 'Create and switch to branch',
		command: 'git checkout -b ',
		category: 'git'
	},
	{
		id: 'git-pull',
		name: 'Pull',
		description: 'Fetch and merge remote changes',
		command: 'git pull',
		category: 'git'
	},
	{
		id: 'git-push',
		name: 'Push',
		description: 'Push local commits to remote',
		command: 'git push',
		category: 'git'
	},
	{
		id: 'git-fetch',
		name: 'Fetch',
		description: 'Download remote refs',
		command: 'git fetch --all',
		category: 'git'
	},
	{
		id: 'git-stash',
		name: 'Stash',
		description: 'Stash changes',
		command: 'git stash',
		category: 'git'
	},
	{
		id: 'git-stash-pop',
		name: 'Stash Pop',
		description: 'Apply and remove stash',
		command: 'git stash pop',
		category: 'git'
	},
	{
		id: 'git-reset-soft',
		name: 'Reset Soft',
		description: 'Undo last commit, keep changes',
		command: 'git reset --soft HEAD~1',
		category: 'git'
	},
	{
		id: 'git-remote',
		name: 'List Remotes',
		description: 'Show remote repositories',
		command: 'git remote -v',
		category: 'git'
	}
];

/** Kubernetes snippets */
const kubernetesSnippets: Snippet[] = [
	{
		id: 'k8s-get-pods',
		name: 'Get Pods',
		description: 'List pods in namespace',
		command: 'kubectl get pods',
		category: 'kubernetes'
	},
	{
		id: 'k8s-get-pods-all',
		name: 'Get All Pods',
		description: 'List pods in all namespaces',
		command: 'kubectl get pods -A',
		category: 'kubernetes'
	},
	{
		id: 'k8s-get-svc',
		name: 'Get Services',
		description: 'List services',
		command: 'kubectl get svc',
		category: 'kubernetes'
	},
	{
		id: 'k8s-get-deploy',
		name: 'Get Deployments',
		description: 'List deployments',
		command: 'kubectl get deployments',
		category: 'kubernetes'
	},
	{
		id: 'k8s-get-nodes',
		name: 'Get Nodes',
		description: 'List cluster nodes',
		command: 'kubectl get nodes',
		category: 'kubernetes'
	},
	{
		id: 'k8s-describe-pod',
		name: 'Describe Pod',
		description: 'Show pod details',
		command: 'kubectl describe pod ',
		category: 'kubernetes'
	},
	{
		id: 'k8s-logs',
		name: 'Pod Logs',
		description: 'View pod logs',
		command: 'kubectl logs -f ',
		category: 'kubernetes'
	},
	{
		id: 'k8s-exec',
		name: 'Exec into Pod',
		description: 'Execute shell in pod',
		command: 'kubectl exec -it ',
		category: 'kubernetes'
	},
	{
		id: 'k8s-apply',
		name: 'Apply',
		description: 'Apply configuration',
		command: 'kubectl apply -f ',
		category: 'kubernetes'
	},
	{
		id: 'k8s-delete',
		name: 'Delete',
		description: 'Delete resource',
		command: 'kubectl delete -f ',
		category: 'kubernetes'
	},
	{
		id: 'k8s-get-ns',
		name: 'Get Namespaces',
		description: 'List namespaces',
		command: 'kubectl get namespaces',
		category: 'kubernetes'
	},
	{
		id: 'k8s-ctx',
		name: 'Current Context',
		description: 'Show current context',
		command: 'kubectl config current-context',
		category: 'kubernetes'
	},
	{
		id: 'k8s-ctx-list',
		name: 'List Contexts',
		description: 'List all contexts',
		command: 'kubectl config get-contexts',
		category: 'kubernetes'
	},
	{
		id: 'k8s-scale',
		name: 'Scale Deployment',
		description: 'Scale replicas',
		command: 'kubectl scale deployment --replicas=',
		category: 'kubernetes'
	},
	{
		id: 'k8s-rollout-status',
		name: 'Rollout Status',
		description: 'Check rollout status',
		command: 'kubectl rollout status deployment/',
		category: 'kubernetes'
	},
	{
		id: 'k8s-rollout-restart',
		name: 'Rollout Restart',
		description: 'Restart deployment',
		command: 'kubectl rollout restart deployment/',
		category: 'kubernetes'
	}
];

/** System snippets */
const systemSnippets: Snippet[] = [
	{
		id: 'sys-df',
		name: 'Disk Usage',
		description: 'Show disk space usage',
		command: 'df -h',
		category: 'system'
	},
	{
		id: 'sys-du',
		name: 'Directory Size',
		description: 'Show directory sizes',
		command: 'du -sh *',
		category: 'system'
	},
	{
		id: 'sys-free',
		name: 'Memory',
		description: 'Show memory usage',
		command: 'free -h',
		category: 'system'
	},
	{
		id: 'sys-top',
		name: 'Top Processes',
		description: 'Interactive process viewer',
		command: 'top',
		category: 'system'
	},
	{
		id: 'sys-htop',
		name: 'Htop',
		description: 'Enhanced process viewer',
		command: 'htop',
		category: 'system'
	},
	{
		id: 'sys-ps',
		name: 'Process List',
		description: 'List running processes',
		command: 'ps aux',
		category: 'system'
	},
	{
		id: 'sys-netstat',
		name: 'Network Stats',
		description: 'Show network connections',
		command: 'netstat -tulpn',
		category: 'system'
	},
	{
		id: 'sys-ss',
		name: 'Socket Stats',
		description: 'Show socket statistics',
		command: 'ss -tulpn',
		category: 'system'
	},
	{
		id: 'sys-uptime',
		name: 'Uptime',
		description: 'Show system uptime',
		command: 'uptime',
		category: 'system'
	},
	{
		id: 'sys-hostname',
		name: 'Hostname',
		description: 'Show hostname',
		command: 'hostname',
		category: 'system'
	},
	{
		id: 'sys-uname',
		name: 'System Info',
		description: 'Show system information',
		command: 'uname -a',
		category: 'system'
	},
	{
		id: 'sys-whoami',
		name: 'Current User',
		description: 'Show current user',
		command: 'whoami',
		category: 'system'
	},
	{
		id: 'sys-tail-syslog',
		name: 'Syslog',
		description: 'Tail system log',
		command: 'tail -f /var/log/syslog',
		category: 'system'
	},
	{
		id: 'sys-journalctl',
		name: 'Journal',
		description: 'View systemd journal',
		command: 'journalctl -f',
		category: 'system'
	},
	{
		id: 'sys-systemctl-status',
		name: 'Service Status',
		description: 'Check service status',
		command: 'systemctl status ',
		category: 'system'
	},
	{
		id: 'sys-systemctl-restart',
		name: 'Restart Service',
		description: 'Restart a service',
		command: 'sudo systemctl restart ',
		category: 'system'
	}
];

/** Terraform snippets */
const terraformSnippets: Snippet[] = [
	{
		id: 'tf-init',
		name: 'Init',
		description: 'Initialize Terraform working directory',
		command: 'terraform init',
		category: 'terraform'
	},
	{
		id: 'tf-plan',
		name: 'Plan',
		description: 'Show execution plan',
		command: 'terraform plan',
		category: 'terraform'
	},
	{
		id: 'tf-plan-out',
		name: 'Plan (save)',
		description: 'Save plan to file for apply',
		command: 'terraform plan -out=tfplan',
		category: 'terraform'
	},
	{
		id: 'tf-apply',
		name: 'Apply',
		description: 'Apply changes interactively',
		command: 'terraform apply',
		category: 'terraform'
	},
	{
		id: 'tf-apply-plan',
		name: 'Apply (saved plan)',
		description: 'Apply a saved plan file',
		command: 'terraform apply tfplan',
		category: 'terraform'
	},
	{
		id: 'tf-apply-auto',
		name: 'Apply (auto-approve)',
		description: 'Apply without confirmation',
		command: 'terraform apply -auto-approve',
		category: 'terraform'
	},
	{
		id: 'tf-destroy',
		name: 'Destroy',
		description: 'Destroy all resources',
		command: 'terraform destroy',
		category: 'terraform'
	},
	{
		id: 'tf-destroy-auto',
		name: 'Destroy (auto-approve)',
		description: 'Destroy without confirmation',
		command: 'terraform destroy -auto-approve',
		category: 'terraform'
	},
	{
		id: 'tf-validate',
		name: 'Validate',
		description: 'Validate configuration files',
		command: 'terraform validate',
		category: 'terraform'
	},
	{
		id: 'tf-fmt',
		name: 'Format',
		description: 'Format configuration files',
		command: 'terraform fmt',
		category: 'terraform'
	},
	{
		id: 'tf-fmt-check',
		name: 'Format (check)',
		description: 'Check formatting without changes',
		command: 'terraform fmt -check',
		category: 'terraform'
	},
	{
		id: 'tf-output',
		name: 'Output',
		description: 'Show output values',
		command: 'terraform output',
		category: 'terraform'
	},
	{
		id: 'tf-output-json',
		name: 'Output (JSON)',
		description: 'Show outputs as JSON',
		command: 'terraform output -json',
		category: 'terraform'
	},
	{
		id: 'tf-state-list',
		name: 'State List',
		description: 'List resources in state',
		command: 'terraform state list',
		category: 'terraform'
	},
	{
		id: 'tf-state-show',
		name: 'State Show',
		description: 'Show resource in state',
		command: 'terraform state show ',
		category: 'terraform'
	},
	{
		id: 'tf-workspace-list',
		name: 'Workspace List',
		description: 'List workspaces',
		command: 'terraform workspace list',
		category: 'terraform'
	},
	{
		id: 'tf-workspace-new',
		name: 'Workspace New',
		description: 'Create new workspace',
		command: 'terraform workspace new ',
		category: 'terraform'
	},
	{
		id: 'tf-workspace-select',
		name: 'Workspace Select',
		description: 'Switch workspace',
		command: 'terraform workspace select ',
		category: 'terraform'
	},
	{
		id: 'tf-refresh',
		name: 'Refresh',
		description: 'Update state with real resources',
		command: 'terraform refresh',
		category: 'terraform'
	},
	{
		id: 'tf-import',
		name: 'Import',
		description: 'Import existing resource',
		command: 'terraform import ',
		category: 'terraform'
	},
	{
		id: 'tf-taint',
		name: 'Taint',
		description: 'Mark resource for recreation',
		command: 'terraform taint ',
		category: 'terraform'
	},
	{
		id: 'tf-untaint',
		name: 'Untaint',
		description: 'Remove taint from resource',
		command: 'terraform untaint ',
		category: 'terraform'
	},
	{
		id: 'tf-graph',
		name: 'Graph',
		description: 'Generate dependency graph',
		command: 'terraform graph',
		category: 'terraform'
	},
	{
		id: 'tf-providers',
		name: 'Providers',
		description: 'Show required providers',
		command: 'terraform providers',
		category: 'terraform'
	},
	{
		id: 'tf-version',
		name: 'Version',
		description: 'Show Terraform version',
		command: 'terraform version',
		category: 'terraform'
	}
];

// Initialize maps
function initSnippets(): void {
	const allSnippets = [
		...terraformSnippets,
		...dockerSnippets,
		...gitSnippets,
		...kubernetesSnippets,
		...systemSnippets
	];

	for (const snippet of allSnippets) {
		snippetsById.set(snippet.id, snippet);

		const categoryList = snippetsByCategory.get(snippet.category) ?? [];
		categoryList.push(snippet);
		snippetsByCategory.set(snippet.category, categoryList);
	}
}

initSnippets();

/** Get snippet by ID - O(1) */
export function getSnippetById(id: string): Snippet | undefined {
	return snippetsById.get(id);
}

/** Get all snippets for a category - O(1) */
export function getSnippetsByCategory(category: SnippetCategory): Snippet[] {
	return snippetsByCategory.get(category) ?? [];
}

/** Get all available categories */
export function getCategories(): SnippetCategory[] {
	return Array.from(snippetsByCategory.keys());
}

/** Get all snippets */
export function getAllSnippets(): Snippet[] {
	return Array.from(snippetsById.values());
}
