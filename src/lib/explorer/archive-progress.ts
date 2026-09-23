/**
 * Turning an archive tool's chatter into a progress bar.
 *
 * None of these tools will tell you how far along they are. What they will do,
 * if asked not to be quiet, is name each file as they reach it. So the count
 * comes from counting those lines against a total obtained separately, before
 * the work starts.
 *
 * That makes progress a measure of *files*, not bytes. It is honest and even
 * when there are many files, and jumpy when there is one enormous one — which
 * is why the interface says "N of M files" rather than implying a smooth
 * byte-level estimate it does not have.
 *
 * Each tool announces itself differently, and some do not announce at all.
 * Where nothing can be counted the bar runs indeterminate rather than
 * inventing a number.
 */

/** Matches the lines a tool prints once per file, or null if it prints none. */
type LineMatcher = ((line: string) => boolean) | null;

const MATCHERS: Record<string, LineMatcher> = {
	// tar names each member, one per line, and says nothing else.
	tar: (l) => l.length > 0,
	bsdtar: (l) => l.length > 0,
	// Info-ZIP prefixes each entry with what it did to it.
	zip: (l) => /^\s*(adding|updating|deflated|stored):/i.test(l),
	unzip: (l) => /^\s*(inflating|extracting|creating|linking):/i.test(l),
	// 7-Zip with -bb1 lists each file behind a status character.
	'7z': (l) => /^[+\-UT.] /.test(l),
	'7za': (l) => /^[+\-UT.] /.test(l),
	'7zz': (l) => /^[+\-UT.] /.test(l),
	// Not the stdlib CLI, which says nothing: the backend runs a small script
	// that prints each path as it stores or extracts it.
	python3: (l) => l.length > 0
};

/** Whether this tool can be measured at all. */
export function isMeasurable(tool: string): boolean {
	return MATCHERS[tool] != null;
}

/**
 * Counts files as a tool reports them, across chunks that split mid-line.
 *
 * Output arrives in arbitrary pieces, so a chunk ending halfway through a
 * filename must not be counted as a whole line, nor lost. The tail is held
 * back until its newline arrives.
 */
export function createProgressCounter(tool: string): {
	push(chunk: string): number;
	count(): number;
} {
	const matcher = MATCHERS[tool] ?? null;
	let pending = '';
	let seen = 0;

	return {
		push(chunk: string): number {
			if (matcher === null) return seen;
			pending += chunk;
			// Keep whatever follows the last newline: it is an unfinished line.
			const parts = pending.split('\n');
			pending = parts.pop() ?? '';
			for (const raw of parts) {
				const line = raw.replace(/\r$/, '').trimEnd();
				if (matcher(line)) seen++;
			}
			return seen;
		},
		count(): number {
			return seen;
		}
	};
}

/**
 * The percentage to show, or null when there is nothing honest to show.
 *
 * Capped at 99 while running: a tool that names a file has not finished
 * writing it, and a bar that sits at 100% for the last few seconds of a large
 * archive reads as a hang. Completion is signalled by the operation ending,
 * not by arithmetic.
 */
export function percentOf(done: number, total: number): number | null {
	if (total <= 0) return null;
	return Math.max(0, Math.min(99, Math.floor((done / total) * 100)));
}
