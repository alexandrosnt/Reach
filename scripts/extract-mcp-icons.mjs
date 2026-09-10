import * as si from 'simple-icons';
import { writeFileSync } from 'fs';

// slug -> simple-icons export. Only brands Simple Icons actually ships; VS Code
// and OpenAI were removed over trademark policy, so those fall back to a
// neutral glyph rather than borrowing someone else's mark.
const want = [
  ['claude', 'Claude'], ['cursor', 'Cursor'], ['windsurf', 'Windsurf'],
  ['zed', 'Zedindustries'], ['jetbrains', 'Jetbrains'],
  ['gemini', 'Googlegemini'], ['copilot', 'Githubcopilot'],
];

const out = {};
for (const [slug, key] of want) {
  const icon = si['si' + key];
  if (!icon) { console.error('MISSING', key); process.exit(1); }
  out[slug] = { path: icon.path, hex: icon.hex, title: icon.title };
}

const body = `/**
 * Brand marks for the MCP client wizard. Sourced from Simple Icons
 * (simpleicons.org), extracted at build time by scripts/extract-mcp-icons.mjs.
 *
 * Not every client has one: Simple Icons removed Visual Studio Code and OpenAI
 * over trademark policy. Those render the neutral fallback in McpClientIcon
 * rather than borrowing a different company's mark, which would be both wrong
 * and confusing.
 */

export interface ClientIconData {
	path: string;
	hex: string;
	title: string;
}

export const clientIcons: Record<string, ClientIconData> = ${JSON.stringify(out, null, '\t')};
`;
writeFileSync('src/lib/data/mcp-client-icons.ts', body.replace(/\n/g, '\r\n'));
console.log('wrote', Object.keys(out).length, 'icons');
