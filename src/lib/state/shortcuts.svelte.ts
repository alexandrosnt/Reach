import { createTab, closeTab, getActiveTab, getTabs, activateTab } from './tabs.svelte';
import { toggleAIPanel } from './ai-chat.svelte';

export interface Shortcut {
	key: string;
	ctrl?: boolean;
	shift?: boolean;
	alt?: boolean;
	meta?: boolean;
	action: string;
	handler: () => void;
}

let settingsOpener: (() => void) | null = null;

export function registerSettingsOpener(fn: () => void): void {
	settingsOpener = fn;
}

function isMac(): boolean {
	return typeof navigator !== 'undefined' && /Mac|iPod|iPhone|iPad/.test(navigator.platform);
}

function nextTab(): void {
	const tabs = getTabs();
	if (tabs.length <= 1) return;

	const activeIndex = tabs.findIndex((t) => t.active);
	const nextIndex = (activeIndex + 1) % tabs.length;
	activateTab(tabs[nextIndex].id);
}

function previousTab(): void {
	const tabs = getTabs();
	if (tabs.length <= 1) return;

	const activeIndex = tabs.findIndex((t) => t.active);
	const prevIndex = (activeIndex - 1 + tabs.length) % tabs.length;
	activateTab(tabs[prevIndex].id);
}

function closeActiveTab(): void {
	const active = getActiveTab();
	if (active) {
		closeTab(active.id);
	}
}

function openSettings(): void {
	settingsOpener?.();
}

function openCommandPalette(): void {
	// Placeholder for command palette
}

const shortcuts: Shortcut[] = [
	{ key: 't', ctrl: true, action: 'New tab', handler: () => createTab('local') },
	{ key: 'w', ctrl: true, action: 'Close tab', handler: closeActiveTab },
	{ key: 'Tab', ctrl: true, action: 'Next tab', handler: nextTab },
	{ key: 'Tab', ctrl: true, shift: true, action: 'Previous tab', handler: previousTab },
	{ key: ',', ctrl: true, action: 'Open settings', handler: openSettings },
	{ key: 'p', ctrl: true, shift: true, action: 'Command palette', handler: openCommandPalette },
	{ key: 'a', ctrl: true, shift: true, action: 'Toggle AI panel', handler: toggleAIPanel }
];

function matchesShortcut(e: KeyboardEvent, shortcut: Shortcut): boolean {
	const mac = isMac();
	const modKey = mac ? e.metaKey : e.ctrlKey;

	if (shortcut.ctrl && !modKey) return false;
	if (!shortcut.ctrl && modKey) return false;

	if (shortcut.shift && !e.shiftKey) return false;
	if (!shortcut.shift && e.shiftKey && shortcut.key !== 'Tab') return false;

	if (shortcut.alt && !e.altKey) return false;

	return e.key.toLowerCase() === shortcut.key.toLowerCase() || e.key === shortcut.key;
}

function onKeydown(e: KeyboardEvent): void {
	for (const shortcut of shortcuts) {
		if (matchesShortcut(e, shortcut)) {
			e.preventDefault();
			e.stopPropagation();
			shortcut.handler();
			return;
		}
	}
}

let initialized = false;

export function initShortcuts(): void {
	if (initialized) return;
	document.addEventListener('keydown', onKeydown, true);
	initialized = true;
}

export function cleanupShortcuts(): void {
	if (!initialized) return;
	document.removeEventListener('keydown', onKeydown, true);
	initialized = false;
}

export function getShortcuts(): Shortcut[] {
	return shortcuts;
}
