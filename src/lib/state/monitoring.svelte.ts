export interface SystemStats {
	cpu: number;
	ram: number;
	ramTotal: number;
	ramUsed: number;
	disk: number;
	users: string[];
}

// Use a plain reactive object for better Svelte 5 $derived tracking
let stats = $state<Record<string, SystemStats>>({});

export function getStats(connId: string): SystemStats | undefined {
	return stats[connId];
}

export function getAllStats(): Record<string, SystemStats> {
	return stats;
}

export function updateStats(connId: string, newStats: SystemStats): void {
	stats[connId] = newStats;
}

export function removeStats(connId: string): void {
	delete stats[connId];
}
