/**
 * The window that asks for a master password when none is set. Without one
 * the vault opens only through the OS keychain, and is lost with it
 * (issue #30), so it is asked for where it can be set, not pointed at in a
 * notice. Once per launch: "Not now" is an answer.
 */
let open = $state(false);
let askedThisLaunch = false;

export function isPasswordPromptOpen(): boolean {
	return open;
}

export function promptForMasterPassword(): void {
	if (askedThisLaunch) return;
	askedThisLaunch = true;
	open = true;
}

export function closePasswordPrompt(): void {
	open = false;
}
