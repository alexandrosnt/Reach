/**
 * What a snippet types into the terminal.
 *
 * A multi-line snippet used to go in as raw text, one line after another, so
 * the shell ran the first line and the rest waited in the terminal's input
 * for whatever read it next. Anything that reads the keyboard took it: on
 * Debian 13 `apt list` opens its output in `less`, which ate the next line as
 * pager keys (`apt upgrade` arrived as `t upgrade`, issue #154), and a
 * `[Y/n]` question would have been answered by the line after it.
 *
 * When the shell has asked for bracketed paste (bash 5.1 and later, zsh and
 * fish all do), the snippet goes in the way a paste does: the shell reads all
 * of it into its own line at once and runs the lines in order on one Enter,
 * so nothing is left over for a pager or a prompt to take. It also makes the
 * palette's Paste what it says it is: every line waits for Enter, where
 * before all lines but the last ran on arrival.
 *
 * A shell that has not asked for it (sh, older bash) gets the lines as
 * before, because the markers would be typed in as text.
 */

const PASTE_START = '\x1b[200~';
const PASTE_END = '\x1b[201~';

export interface SnippetInputOptions {
	/** The terminal's shell has turned bracketed paste on (DECSET 2004). */
	bracketed: boolean;
	/** Press Enter after it: the snippet panel's Run. */
	run: boolean;
}

/**
 * The text to send for `command`.
 *
 * Line ends become CR, the Enter key, as xterm does for a paste. Trailing
 * line ends are dropped so Run presses Enter once, not once per blank line.
 * Paste markers inside the command are removed: a snippet can come from a
 * shared vault, and an end marker in it would end the paste early and run
 * the rest as typed.
 */
export function snippetInput(command: string, options: SnippetInputOptions): string {
	const text = command
		.split(PASTE_START).join('')
		.split(PASTE_END).join('')
		.replace(/\r\n?/g, '\n')
		.replace(/\n+$/, '');
	if (!text) return '';
	const body = text.replace(/\n/g, '\r');
	const typed = options.bracketed ? PASTE_START + body + PASTE_END : body;
	return options.run ? typed + '\r' : typed;
}
