export type ToastType = 'info' | 'success' | 'warning' | 'error';

export interface Toast {
	id: string;
	message: string;
	type: ToastType;
	duration: number;
	dismissing: boolean;
}

let toasts = $state<Toast[]>([]);

const timers = new Map<string, ReturnType<typeof setTimeout>>();

export function getToasts(): Toast[] {
	return toasts;
}

export function addToast(
	message: string,
	type: ToastType = 'info',
	duration: number = 3000
): Toast {
	const id = crypto.randomUUID();

	const toast: Toast = { id, message, type, duration, dismissing: false };
	toasts.push(toast);

	try {
		const audio = new Audio('/sounds/notification.wav');
		audio.volume = 0.4;
		audio.play();
	} catch {
		// Audio playback not available
	}

	if (duration > 0) {
		const timer = setTimeout(() => {
			dismissToast(id);
		}, duration);
		timers.set(id, timer);
	}

	return toast;
}

export function dismissToast(id: string): void {
	const toast = toasts.find((t) => t.id === id);
	if (toast && !toast.dismissing) {
		toast.dismissing = true;
		setTimeout(() => removeToast(id), 250);
	}
}

export function removeToast(id: string): void {
	const timer = timers.get(id);
	if (timer) {
		clearTimeout(timer);
		timers.delete(id);
	}

	const index = toasts.findIndex((t) => t.id === id);
	if (index !== -1) {
		toasts.splice(index, 1);
	}
}

export function clearAllToasts(): void {
	for (const timer of timers.values()) {
		clearTimeout(timer);
	}
	timers.clear();
	toasts.length = 0;
}
