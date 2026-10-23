/**
 * Draws the open list of every native <select> in the page, in the theme.
 *
 * A native select's popup belongs to the platform, not the page. On Linux,
 * WebKitGTK draws it as a GTK menu that ignores every style on <option>, so
 * under a dark KDE/GTK theme it could come out white with the theme's white
 * text on it (#110), and on Windows and macOS it is the system's grey, not
 * the theme. CSS cannot reach it on any of them.
 *
 * So the popup is replaced, not the select: the <select> stays in the page as
 * the field people focus and tab to, keeping its own styling, its label and
 * every `bind:value` and `onchange` on it; only opening it shows this list
 * instead. Picking an option sets the select's value and fires the same
 * `input` and `change` events a native pick does.
 *
 * Focus stays on the select while the list is open (the list is pointed to by
 * aria-activedescendant), so dialogs that keep focus inside themselves never
 * fight it.
 */

import { safeInsets } from '$lib/edges';

/** One row of the list: an option, or the heading of an <optgroup>. */
export interface MenuRow {
	label: string;
	/** Index into select.options; -1 for a group heading. */
	index: number;
	disabled: boolean;
	group: boolean;
	/** An option inside an <optgroup>, drawn indented under its heading. */
	inGroup: boolean;
}

/** The rows a select shows, in order. Hidden options are left out. */
export function menuRows(select: HTMLSelectElement): MenuRow[] {
	const rows: MenuRow[] = [];
	const options = Array.from(select.options);
	const add = (o: HTMLOptionElement, inGroup: boolean, groupDisabled: boolean) => {
		if (o.hidden) return;
		rows.push({
			label: o.label || o.text,
			index: options.indexOf(o),
			disabled: o.disabled || groupDisabled,
			group: false,
			inGroup
		});
	};
	for (const child of Array.from(select.children)) {
		if (child instanceof HTMLOptGroupElement) {
			if (child.hidden) continue;
			rows.push({ label: child.label, index: -1, disabled: true, group: true, inGroup: false });
			for (const o of Array.from(child.children)) {
				if (o instanceof HTMLOptionElement) add(o, true, child.disabled);
			}
		} else if (child instanceof HTMLOptionElement) {
			add(child, false, false);
		}
	}
	return rows;
}

/** The next row that can be picked from `from`, going `step` (±1), or `from` if none. */
export function stepRow(rows: MenuRow[], from: number, step: number): number {
	for (let i = from + step; i >= 0 && i < rows.length; i += step) {
		if (!rows[i].disabled) return i;
	}
	return from;
}

/**
 * The row type-to-find lands on for `typed`. Typing the same letter again
 * moves to the next row starting with it, as native lists do.
 */
export function findRow(rows: MenuRow[], typed: string, from: number): number {
	const q = typed.toLocaleLowerCase();
	const repeat = q.length > 1 && [...q].every((c) => c === q[0]);
	const needle = repeat ? q[0] : q;
	const start = needle.length === 1 ? from + 1 : from;
	for (let k = 0; k < rows.length; k++) {
		const i = (start + k) % rows.length;
		const r = rows[i];
		if (!r.disabled && r.label.trim().toLocaleLowerCase().startsWith(needle)) return i;
	}
	return -1;
}

/** The select this event is about, when it is one whose popup we draw. */
function selectOf(target: EventTarget | null): HTMLSelectElement | null {
	if (!(target instanceof Element)) return null;
	const el = target.closest('select');
	if (!el || el.disabled || el.multiple || el.size > 1) return null;
	return el;
}

const GAP = 4;
const MARGIN = 8;
const MAX_HEIGHT = 320;

let ids = 0;

/** Starts drawing select popups in the page. Returns the way to stop. */
export function installSelectMenus(): () => void {
	let select: HTMLSelectElement | null = null;
	let rows: MenuRow[] = [];
	let active = -1;
	let menu: HTMLDivElement | null = null;
	let typed = '';
	let typedAt = 0;

	function pick(row: number) {
		const s = select;
		const r = rows[row];
		close();
		if (!s || !r || r.disabled) return;
		if (s.selectedIndex !== r.index) {
			s.selectedIndex = r.index;
			s.dispatchEvent(new Event('input', { bubbles: true }));
			s.dispatchEvent(new Event('change', { bubbles: true }));
		}
	}

	function setActive(row: number) {
		if (!menu) return;
		menu.querySelector('.select-menu-row.active')?.classList.remove('active');
		active = row;
		const el = menu.children[row] as HTMLElement | undefined;
		if (!el) {
			select?.removeAttribute('aria-activedescendant');
			return;
		}
		el.classList.add('active');
		select?.setAttribute('aria-activedescendant', el.id);
		// Keep it in view without scrolling the page behind.
		const top = el.offsetTop;
		const bottom = top + el.offsetHeight;
		if (top < menu.scrollTop) menu.scrollTop = top;
		else if (bottom > menu.scrollTop + menu.clientHeight) menu.scrollTop = bottom - menu.clientHeight;
	}

	function place() {
		if (!menu || !select) return;
		const r = select.getBoundingClientRect();
		// The window less a phone's status bar, camera cutout and gesture bar.
		const safe = safeInsets();
		const vw = window.innerWidth - safe.right;
		const vh = window.innerHeight - safe.bottom;
		const below = vh - r.bottom - GAP - MARGIN;
		const above = r.top - safe.top - GAP - MARGIN;
		menu.style.minWidth = `${Math.round(r.width)}px`;
		menu.style.maxWidth = `${vw - safe.left - MARGIN * 2}px`;
		menu.style.maxHeight = '';
		const natural = Math.min(menu.scrollHeight, MAX_HEIGHT);
		// Downward unless it would not fit there and there is more room above.
		const down = natural <= below || below >= above;
		const room = Math.max(0, Math.min(MAX_HEIGHT, down ? below : above));
		menu.style.maxHeight = `${room}px`;
		const h = Math.min(natural, room);
		menu.style.top = `${down ? r.bottom + GAP : r.top - GAP - h}px`;
		const w = menu.offsetWidth;
		menu.style.left = `${Math.min(Math.max(safe.left + MARGIN, r.left), Math.max(safe.left + MARGIN, vw - w - MARGIN))}px`;
	}

	function open(s: HTMLSelectElement) {
		close();
		select = s;
		rows = menuRows(s);
		const id = `select-menu-${++ids}`;
		menu = document.createElement('div');
		menu.className = 'select-menu';
		menu.id = id;
		menu.setAttribute('role', 'listbox');
		const name = s.getAttribute('aria-label') ?? s.labels?.[0]?.textContent?.trim();
		if (name) menu.setAttribute('aria-label', name);
		// Same font as the field it opened from, so a compact toolbar select
		// opens a compact list.
		const cs = getComputedStyle(s);
		menu.style.fontFamily = cs.fontFamily;
		menu.style.fontSize = cs.fontSize;

		rows.forEach((r, i) => {
			const el = document.createElement('div');
			el.id = `${id}-${i}`;
			el.className = 'select-menu-row';
			el.textContent = r.label;
			if (r.group) {
				el.classList.add('group');
				el.setAttribute('role', 'presentation');
			} else {
				el.setAttribute('role', 'option');
				const chosen = r.index === s.selectedIndex;
				el.setAttribute('aria-selected', String(chosen));
				if (chosen) el.classList.add('selected');
				if (r.inGroup) el.classList.add('in-group');
				if (r.disabled) {
					el.classList.add('disabled');
					el.setAttribute('aria-disabled', 'true');
				}
			}
			menu!.appendChild(el);
		});

		menu.addEventListener('mousedown', (e) => e.preventDefault());
		menu.addEventListener('mousemove', (e) => {
			const row = rowOf(e.target);
			if (row >= 0 && row !== active && !rows[row].disabled) setActive(row);
		});
		menu.addEventListener('click', (e) => {
			const row = rowOf(e.target);
			if (row >= 0 && !rows[row].disabled) pick(row);
		});

		document.body.appendChild(menu);
		s.setAttribute('aria-expanded', 'true');
		s.setAttribute('aria-controls', id);
		place();
		const current = rows.findIndex((r) => r.index === s.selectedIndex && !r.disabled);
		setActive(current >= 0 ? current : stepRow(rows, -1, 1));
	}

	function rowOf(target: EventTarget | null): number {
		if (!menu || !(target instanceof Element)) return -1;
		const el = target.closest('.select-menu-row');
		return el ? Array.prototype.indexOf.call(menu.children, el) : -1;
	}

	function close() {
		if (select) {
			select.removeAttribute('aria-expanded');
			select.removeAttribute('aria-controls');
			select.removeAttribute('aria-activedescendant');
		}
		menu?.remove();
		menu = null;
		select = null;
		rows = [];
		active = -1;
		typed = '';
	}

	/** Moves a closed select to the next or previous option, as arrow keys do. */
	function nudge(s: HTMLSelectElement, step: number) {
		const r = menuRows(s);
		const at = r.findIndex((x) => x.index === s.selectedIndex);
		const next = stepRow(r, at, step);
		if (next === at || next < 0 || r[next].disabled) return;
		s.selectedIndex = r[next].index;
		s.dispatchEvent(new Event('input', { bubbles: true }));
		s.dispatchEvent(new Event('change', { bubbles: true }));
	}

	function onMouseDown(e: MouseEvent) {
		if (menu?.contains(e.target as Node)) return;
		const s = selectOf(e.target);
		if (s && e.button === 0) {
			e.preventDefault();
			s.focus();
			if (select === s) close();
			else open(s);
			return;
		}
		close();
	}

	function onKeyDown(e: KeyboardEvent) {
		if (menu && select) {
			if (e.target !== select && !menu.contains(e.target as Node)) {
				close();
				return;
			}
			let handled = true;
			switch (e.key) {
				case 'ArrowDown':
					setActive(stepRow(rows, active, 1));
					break;
				case 'ArrowUp':
					if (e.altKey) pick(active);
					else setActive(stepRow(rows, active, -1));
					break;
				case 'Home':
					setActive(stepRow(rows, -1, 1));
					break;
				case 'End':
					setActive(stepRow(rows, rows.length, -1));
					break;
				case 'PageDown':
				case 'PageUp': {
					const step = e.key === 'PageDown' ? 1 : -1;
					let row = active;
					for (let i = 0; i < 10; i++) row = stepRow(rows, row, step);
					setActive(row);
					break;
				}
				case 'Enter':
					pick(active);
					break;
				case ' ':
					if (typed && Date.now() - typedAt < 600) handled = typeAhead(e.key);
					else pick(active);
					break;
				case 'Escape':
					close();
					break;
				case 'Tab':
					// Let focus move on as usual; the list just goes away.
					close();
					handled = false;
					break;
				default:
					handled = e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey && typeAhead(e.key);
			}
			if (handled) {
				e.preventDefault();
				e.stopPropagation();
			}
			return;
		}

		const s = selectOf(e.target);
		if (!s || e.ctrlKey || e.metaKey) return;
		// Enter too: WebKitGTK opens its own popup on it.
		const opens =
			e.key === 'F4' ||
			e.key === ' ' ||
			e.key === 'Enter' ||
			((e.key === 'ArrowDown' || e.key === 'ArrowUp') && e.altKey);
		if (opens) {
			e.preventDefault();
			e.stopPropagation();
			open(s);
		} else if ((e.key === 'ArrowDown' || e.key === 'ArrowUp') && !e.shiftKey) {
			// Native arrows open the popup on macOS and step the value elsewhere;
			// stepping everywhere keeps one behaviour and never shows the old popup.
			e.preventDefault();
			e.stopPropagation();
			nudge(s, e.key === 'ArrowDown' ? 1 : -1);
		}
	}

	function typeAhead(key: string): boolean {
		const now = Date.now();
		typed = now - typedAt < 600 ? typed + key : key;
		typedAt = now;
		const row = findRow(rows, typed, active);
		if (row >= 0) setActive(row);
		return true;
	}

	function onScroll(e: Event) {
		if (menu && !menu.contains(e.target as Node)) close();
	}

	function onBlur() {
		close();
	}

	// Capture, on window: ahead of dialogs and shortcuts, so Escape closes the
	// list rather than the dialog under it.
	window.addEventListener('mousedown', onMouseDown, true);
	window.addEventListener('keydown', onKeyDown, true);
	window.addEventListener('scroll', onScroll, true);
	window.addEventListener('resize', onBlur);
	window.addEventListener('blur', onBlur);
	return () => {
		close();
		window.removeEventListener('mousedown', onMouseDown, true);
		window.removeEventListener('keydown', onKeyDown, true);
		window.removeEventListener('scroll', onScroll, true);
		window.removeEventListener('resize', onBlur);
		window.removeEventListener('blur', onBlur);
	};
}
