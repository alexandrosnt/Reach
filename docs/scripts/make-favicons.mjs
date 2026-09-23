/**
 * Build the site's icons from the application's own icon.
 *
 * The site was shipping Astro's default starburst favicon, because Starlight
 * falls back to `public/favicon.svg` when no favicon is configured and nobody
 * had replaced it. Browsers request `/favicon.ico` whether or not a page asks
 * them to, so that file has to exist regardless of what the markup says.
 *
 * Source is src-tauri/icons/icon.png — 512x512 with transparency, the same
 * artwork the installed application uses, so the tab icon and the taskbar
 * icon match.
 *
 * What gets written:
 *
 *   favicon.ico          16/32/48 in one file, for the request browsers make
 *                        on their own and for bookmarks and history
 *   apple-touch-icon.png 180x180, flattened — iOS ignores transparency and
 *                        composites onto white, which would put a white halo
 *                        around a dark icon, so the background is explicit
 *   icon-192 / icon-512  Android home screen and the install prompt
 *   site.webmanifest     names the above, and makes "Add to home screen"
 *                        produce something that looks deliberate
 *
 * No SVG favicon: the source is raster, and an SVG wrapping a bitmap is
 * larger than the PNG with none of the crispness that would justify it.
 *
 * Run: node scripts/make-favicons.mjs
 */

import sharp from 'sharp';
import { writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';

const SOURCE = fileURLToPath(new URL('../../src-tauri/icons/icon.png', import.meta.url));
const PUBLIC = fileURLToPath(new URL('../public/', import.meta.url));

/** The application's own background, so flattened icons match the product. */
const BACKDROP = { r: 10, g: 10, b: 10, alpha: 1 };

const png = (size, flatten = false) => {
	let pipeline = sharp(SOURCE).resize(size, size, {
		fit: 'contain',
		background: { r: 0, g: 0, b: 0, alpha: 0 },
	});
	if (flatten) pipeline = pipeline.flatten({ background: BACKDROP });
	return pipeline.png({ compressionLevel: 9 }).toBuffer();
};

/**
 * Pack PNGs into an .ico.
 *
 * The container is a 6-byte header, one 16-byte directory entry per image,
 * then the images back to back. Every browser since Vista reads PNG payloads
 * inside an ICO, which avoids hand-rolling a BMP encoder for no benefit.
 */
function ico(images) {
	const header = Buffer.alloc(6);
	header.writeUInt16LE(0, 0); // reserved
	header.writeUInt16LE(1, 2); // 1 = icon
	header.writeUInt16LE(images.length, 4);

	let offset = 6 + images.length * 16;
	const entries = images.map(({ size, data }) => {
		const e = Buffer.alloc(16);
		e.writeUInt8(size >= 256 ? 0 : size, 0); // 0 means 256
		e.writeUInt8(size >= 256 ? 0 : size, 1);
		e.writeUInt8(0, 2); // palette size, 0 for true colour
		e.writeUInt8(0, 3); // reserved
		e.writeUInt16LE(1, 4); // colour planes
		e.writeUInt16LE(32, 6); // bits per pixel
		e.writeUInt32LE(data.length, 8);
		e.writeUInt32LE(offset, 12);
		offset += data.length;
		return e;
	});

	return Buffer.concat([header, ...entries, ...images.map((i) => i.data)]);
}

const icoSizes = [16, 32, 48];
const icoImages = await Promise.all(
	icoSizes.map(async (size) => ({ size, data: await png(size) })),
);
await writeFile(new URL('favicon.ico', `file://${PUBLIC}`), ico(icoImages));

await writeFile(new URL('apple-touch-icon.png', `file://${PUBLIC}`), await png(180, true));
await writeFile(new URL('icon-192.png', `file://${PUBLIC}`), await png(192, true));
await writeFile(new URL('icon-512.png', `file://${PUBLIC}`), await png(512, true));

const manifest = {
	name: 'Reach — SSH Client and Server Manager',
	short_name: 'Reach',
	description:
		'A modern, lightweight SSH client and remote server manager. Terminals, files, tunnels, secrets and automation in one place.',
	start_url: '/',
	display: 'standalone',
	background_color: '#0a0a0a',
	theme_color: '#0a0a0a',
	icons: [
		{ src: '/icon-192.png', sizes: '192x192', type: 'image/png' },
		{ src: '/icon-512.png', sizes: '512x512', type: 'image/png' },
	],
};
await writeFile(
	new URL('site.webmanifest', `file://${PUBLIC}`),
	`${JSON.stringify(manifest, null, '\t')}\n`,
);

console.log(
	`favicon.ico (${icoSizes.join('/')}), apple-touch-icon.png, icon-192, icon-512, site.webmanifest`,
);
