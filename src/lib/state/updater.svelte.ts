import { check } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';

let updateAvailable = $state(false);
let updateVersion = $state<string | null>(null);
let updateNotes = $state<string | null>(null);
let downloading = $state(false);
let downloadProgress = $state(0);
let installing = $state(false);
let startupBlocking = $state(false);
let downloadAttempts = $state(0);
let error = $state<string | null>(null);
let dismissed = $state(false);

let periodicInterval: ReturnType<typeof setInterval> | null = null;

export function getUpdaterState() {
	return {
		get updateAvailable() { return updateAvailable; },
		get updateVersion() { return updateVersion; },
		get updateNotes() { return updateNotes; },
		get downloading() { return downloading; },
		get downloadProgress() { return downloadProgress; },
		get installing() { return installing; },
		get startupBlocking() { return startupBlocking; },
		get downloadAttempts() { return downloadAttempts; },
		get error() { return error; },
		get dismissed() { return dismissed; }
	};
}

let cachedUpdate: Awaited<ReturnType<typeof check>> | null = null;

export async function checkForUpdate(): Promise<boolean> {
	try {
		error = null;
		const update = await check();
		if (update) {
			updateAvailable = true;
			updateVersion = update.version;
			updateNotes = update.body ?? null;
			cachedUpdate = update;
			return true;
		}
		updateAvailable = false;
		return false;
	} catch (e) {
		console.error('Update check failed:', e);
		error = e instanceof Error ? e.message : String(e);
		return false;
	}
}

export async function startupUpdateCheck(): Promise<void> {
	const found = await checkForUpdate();
	if (found) {
		startupBlocking = true;
	}
}

export async function downloadAndInstall(): Promise<void> {
	if (!cachedUpdate || downloading || installing) return;

	try {
		error = null;
		downloading = true;
		downloadProgress = 0;

		let contentLength = 0;
		let downloaded = 0;

		await cachedUpdate.downloadAndInstall((event) => {
			if (event.event === 'Started') {
				contentLength = event.data.contentLength ?? 0;
			} else if (event.event === 'Progress') {
				downloaded += event.data.chunkLength;
				if (contentLength > 0) {
					downloadProgress = Math.round((downloaded / contentLength) * 100);
				}
			} else if (event.event === 'Finished') {
				downloading = false;
				installing = true;
			}
		});

		await relaunch();
	} catch (e) {
		downloading = false;
		installing = false;
		downloadAttempts++;
		error = e instanceof Error ? e.message : String(e);
		console.error('Download/install failed:', e);
	}
}

export function dismissUpdate(): void {
	dismissed = true;
}

export function skipStartupUpdate(): void {
	startupBlocking = false;
}

export function startPeriodicChecks(): void {
	stopPeriodicChecks();
	periodicInterval = setInterval(async () => {
		if (!downloading && !installing && !startupBlocking) {
			const found = await checkForUpdate();
			if (found) {
				dismissed = false;
			}
		}
	}, 45 * 60 * 1000);
}

export function stopPeriodicChecks(): void {
	if (periodicInterval) {
		clearInterval(periodicInterval);
		periodicInterval = null;
	}
}
