/**
 * Formatting utilities for the Reach app.
 */

const BYTE_UNITS = ['B', 'KB', 'MB', 'GB', 'TB', 'PB'] as const;

/**
 * Format bytes into a human-readable string.
 * e.g., 1536 → "1.5 KB", 1073741824 → "1.0 GB"
 */
export function formatBytes(bytes: number, decimals: number = 1): string {
	if (bytes === 0) return '0 B';
	if (bytes < 0) return '-' + formatBytes(-bytes, decimals);

	const k = 1024;
	const i = Math.floor(Math.log(bytes) / Math.log(k));
	const unit = BYTE_UNITS[Math.min(i, BYTE_UNITS.length - 1)];
	const value = bytes / Math.pow(k, i);

	return `${value.toFixed(decimals)} ${unit}`;
}

/**
 * Format a percentage value with optional decimal places.
 * e.g., 0.4567 → "45.7%", 67.89 → "67.9%"
 * Input can be 0-1 (ratio) or 0-100 (percentage).
 */
export function formatPercent(value: number, decimals: number = 1): string {
	const pct = value <= 1 ? value * 100 : value;
	return `${pct.toFixed(decimals)}%`;
}

/**
 * Format a Unix timestamp (seconds) into a localized date string.
 */
export function formatDate(timestamp: number): string {
	const date = new Date(timestamp * 1000);
	return date.toLocaleDateString(undefined, {
		year: 'numeric',
		month: 'short',
		day: 'numeric',
		hour: '2-digit',
		minute: '2-digit'
	});
}

/**
 * Format a Unix timestamp into a relative time string.
 * e.g., "2 minutes ago", "3 hours ago", "yesterday"
 */
export function formatRelativeTime(timestamp: number): string {
	const now = Math.floor(Date.now() / 1000);
	const diff = now - timestamp;

	if (diff < 60) return 'just now';
	if (diff < 3600) {
		const m = Math.floor(diff / 60);
		return `${m} minute${m !== 1 ? 's' : ''} ago`;
	}
	if (diff < 86400) {
		const h = Math.floor(diff / 3600);
		return `${h} hour${h !== 1 ? 's' : ''} ago`;
	}
	if (diff < 172800) return 'yesterday';

	const d = Math.floor(diff / 86400);
	return `${d} day${d !== 1 ? 's' : ''} ago`;
}

/**
 * Format file permissions from numeric mode to rwx string.
 * e.g., 0o755 → "rwxr-xr-x"
 */
export function formatPermissions(mode: number): string {
	const rwx = (n: number): string => {
		return (n & 4 ? 'r' : '-') + (n & 2 ? 'w' : '-') + (n & 1 ? 'x' : '-');
	};
	return rwx((mode >> 6) & 7) + rwx((mode >> 3) & 7) + rwx(mode & 7);
}

/**
 * Truncate a string to a maximum length with ellipsis.
 */
export function truncate(str: string, maxLength: number): string {
	if (str.length <= maxLength) return str;
	return str.slice(0, maxLength - 1) + '\u2026';
}
