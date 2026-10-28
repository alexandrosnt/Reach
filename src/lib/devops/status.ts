/**
 * Status in plain words, for containers and pods.
 *
 * A state is always shown as a shape and words together, never colour alone
 * (colour-blind users, and screenshots in grey). The engine's or cluster's
 * own term stays one hover away, because that is what people search for.
 */

import { t } from '#lib/state/i18n.svelte.js';

export type Tone = 'ok' | 'busy' | 'warn' | 'bad' | 'idle';

export interface Status {
	tone: Tone;
	/** What the user reads. */
	label: string;
	/** The engine's or cluster's own term, for the tooltip. */
	raw: string;
}

/** Docker's container states: created, running, paused, restarting, removing, exited, dead. */
export function containerStatus(state: string, status: string): Status {
	const raw = status || state;
	switch (state) {
		case 'running':
			return { tone: /\(unhealthy\)/.test(status) ? 'warn' : 'ok', label: /\(unhealthy\)/.test(status) ? t('status.unhealthy') : t('status.running'), raw };
		case 'paused':
			return { tone: 'idle', label: t('status.paused'), raw };
		case 'restarting':
			return { tone: 'warn', label: t('status.restarting'), raw };
		case 'created':
			return { tone: 'idle', label: t('status.created'), raw };
		case 'removing':
			return { tone: 'busy', label: t('status.removing'), raw };
		case 'dead':
			return { tone: 'bad', label: t('status.dead'), raw };
		case 'exited': {
			const code = /Exited \((\d+)\)/.exec(status)?.[1];
			if (code === '0' || code === undefined) return { tone: 'idle', label: t('status.stopped'), raw };
			// 137 is SIGKILL: most often the kernel's out-of-memory killer, or `kill`.
			if (code === '137') return { tone: 'bad', label: t('status.killed'), raw };
			return { tone: 'bad', label: t('status.failed', { code }), raw };
		}
		default:
			return { tone: 'idle', label: state || '?', raw };
	}
}

/** kubectl's STATUS column, put in words. */
export function podStatus(status: string, restarts: number): Status {
	const raw = status;
	switch (status) {
		case 'Running':
			return { tone: 'ok', label: t('status.running'), raw };
		case 'Completed':
		case 'Succeeded':
			return { tone: 'idle', label: t('status.completed'), raw };
		case 'Pending':
		case 'ContainerCreating':
		case 'PodInitializing':
			return { tone: 'busy', label: t('status.starting'), raw };
		case 'Terminating':
			return { tone: 'busy', label: t('status.stopping'), raw };
		case 'CrashLoopBackOff':
			return { tone: 'bad', label: t('status.crash_loop', { count: restarts }), raw };
		case 'ImagePullBackOff':
		case 'ErrImagePull':
		case 'InvalidImageName':
			return { tone: 'bad', label: t('status.image_pull'), raw };
		case 'OOMKilled':
			return { tone: 'bad', label: t('status.out_of_memory'), raw };
		case 'Error':
		case 'Failed':
			return { tone: 'bad', label: restarts > 0 ? t('status.crash_loop', { count: restarts }) : t('status.error'), raw };
		case 'CreateContainerConfigError':
		case 'CreateContainerError':
			return { tone: 'bad', label: t('status.config_error'), raw };
		case 'Evicted':
			return { tone: 'bad', label: t('status.evicted'), raw };
		default:
			if (status.startsWith('Init:')) return { tone: 'busy', label: t('status.initialising'), raw };
			return { tone: 'warn', label: status, raw };
	}
}

/** A workload's replicas: all ready, some ready, none. */
export function replicaStatus(ready: number, desired: number): Status {
	const raw = `${ready}/${desired}`;
	if (desired === 0) return { tone: 'idle', label: t('status.scaled_to_zero'), raw };
	if (ready >= desired) return { tone: 'ok', label: t('status.ready_count', { ready, desired }), raw };
	if (ready === 0) return { tone: 'bad', label: t('status.ready_count', { ready, desired }), raw };
	return { tone: 'warn', label: t('status.ready_count', { ready, desired }), raw };
}

/** "3 minutes ago" from an ISO time or a Unix time in seconds. */
export function ago(when: string | number | null | undefined): string {
	if (when === null || when === undefined || when === '') return '';
	const ms = typeof when === 'number' ? when * 1000 : Date.parse(when);
	if (!Number.isFinite(ms)) return '';
	const s = Math.max(0, Math.round((Date.now() - ms) / 1000));
	if (s < 60) return t('time.seconds', { n: s });
	const m = Math.round(s / 60);
	if (m < 60) return t('time.minutes', { n: m });
	const h = Math.round(m / 60);
	if (h < 48) return t('time.hours', { n: h });
	return t('time.days', { n: Math.round(h / 24) });
}

export function bytes(n: number): string {
	if (!Number.isFinite(n) || n < 0) return '';
	const units = ['B', 'KB', 'MB', 'GB', 'TB'];
	let i = 0;
	while (n >= 1000 && i < units.length - 1) {
		n /= 1000;
		i += 1;
	}
	return `${n < 10 && i > 0 ? n.toFixed(1) : Math.round(n)} ${units[i]}`;
}
