/**
 * A yes/no question in Reach's own dialog, in place of the browser's
 * `confirm()`: that one is an unstyled system box with a generic title, out
 * of the theme and the phone's safe areas, and it stops the page while open.
 * Shown by AskHost, mounted once in the layout.
 */

export interface AskOptions {
	/** The confirming button's label; Confirm by default. */
	confirmLabel?: string;
}

interface Pending {
	message: string;
	options: AskOptions;
	resolve: (ok: boolean) => void;
}

let pending = $state<Pending | null>(null);

/** Resolves true when the person confirms, false when they cancel. A new
 *  question cancels one still open. */
export function ask(message: string, options: AskOptions = {}): Promise<boolean> {
	pending?.resolve(false);
	return new Promise((resolve) => {
		pending = { message, options, resolve };
	});
}

export function currentQuestion(): Pending | null {
	return pending;
}

export function answer(ok: boolean): void {
	const p = pending;
	pending = null;
	p?.resolve(ok);
}
