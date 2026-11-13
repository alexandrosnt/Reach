/** A file's modified time as the Details view shows it: `2026-11-13 14:05`,
 *  local time, sortable by eye. Nothing when the server gave no time. */
export function formatModified(epochSeconds: number): string {
	if (!epochSeconds) return '';
	const d = new Date(epochSeconds * 1000);
	const p = (n: number) => String(n).padStart(2, '0');
	return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
}
