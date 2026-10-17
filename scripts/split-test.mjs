/**
 * Tests the rules of two tabs side by side (issue #106).
 *
 * Node 24 strips TypeScript types on import, so the exact module the webview
 * runs is what is under test here.
 *
 * Run: node scripts/split-test.mjs
 */

import { open, activate, close, clampRatio, paneOf, paneStyle, dividerStyle, dropZone, neighbour } from '../src/lib/terminal/split.ts';

let failures = 0;
const check = (name, cond, extra = '') => {
	if (cond) console.log(`  PASS  ${name}`);
	else {
		failures++;
		console.log(`  FAIL  ${name}${extra ? '\n        ' + extra : ''}`);
	}
};
const eq = (a, b) => JSON.stringify(a) === JSON.stringify(b);

console.log('\nopening');
const s = open('a', 'b', 'right');
check('the focused tab goes first, the new one second', eq(s, { first: 'a', second: 'b', direction: 'right', ratio: 0.5 }));
check('a tab is not split with itself', open('a', 'a', 'right') === null);
check('no split without a focused tab', open(undefined, 'b', 'down') === null);

console.log('\nchoosing tabs');
check('choosing one of the two keeps the split', activate(s, 'a', 'b') === s);
check('another tab replaces the focused pane (first)', eq(activate(s, 'a', 'c'), { ...s, first: 'c' }));
check('another tab replaces the focused pane (second)', eq(activate(s, 'b', 'c'), { ...s, second: 'c' }));
check('without a split nothing changes', activate(null, 'a', 'c') === null);

console.log('\nclosing');
check('closing the first ends it, the second keeps focus', eq(close(s, 'a'), { split: null, focus: 'b' }));
check('closing the second ends it, the first keeps focus', eq(close(s, 'b'), { split: null, focus: 'a' }));
check('closing another tab leaves the split', eq(close(s, 'z'), { split: s }));

console.log('\ndivider');
check('stays between 20% and 80%', clampRatio(0.05) === 0.2 && clampRatio(0.95) === 0.8 && clampRatio(0.6) === 0.6);
check('a broken value falls back to the middle', clampRatio(NaN) === 0.5);
check('panes know their place', paneOf(s, 'a') === 'first' && paneOf(s, 'b') === 'second' && paneOf(s, 'c') === null);
check('side by side: widths split at the divider', paneStyle(s, 'first').includes('width:calc(50.000% - 3px)') && paneStyle(s, 'second').includes('left:calc(50.000% + 3px)'));
const d = { ...s, direction: 'down', ratio: 0.3 };
check('one above the other: heights split at the divider', paneStyle(d, 'first').includes('height:calc(30.000% - 3px)') && paneStyle(d, 'second').includes('top:calc(30.000% + 3px)'));
check('the divider sits on the line', dividerStyle(d).includes('top:calc(30.000% - 3px)') && dividerStyle(s).includes('left:calc(50.000% - 3px)'));

console.log('\ndragging a tab over the terminal');
check('right third opens on the right', dropZone(900, 300, 1000, 600) === 'right');
check('bottom third opens below', dropZone(400, 550, 1000, 600) === 'down');
check('the middle drops nowhere', dropZone(500, 300, 1000, 600) === null);
check('the corner goes to whichever edge is nearer', dropZone(990, 420, 1000, 600) === 'right' && dropZone(700, 590, 1000, 600) === 'down');

console.log('\nkeyboard');
check('the next tab comes in', neighbour(['a', 'b', 'c'], 'a') === 'b');
check('from the last tab, the one before', neighbour(['a', 'b', 'c'], 'c') === 'b');
check('a single tab has no neighbour', neighbour(['a'], 'a') === null);

console.log(failures ? `\n${failures} failed` : '\nall passed');
process.exit(failures ? 1 : 0);
