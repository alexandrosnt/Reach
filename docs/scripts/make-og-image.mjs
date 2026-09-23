/**
 * Build the social preview card from the product banner.
 *
 * Open Graph wants 1200x630 (1.91:1) and the banner is 1536x1024 (3:2), so
 * the two do not agree and something has to give. The first version used
 * `fit: cover`, which crops — and cropping a 3:2 image into 1.91:1 removes
 * nearly a fifth of its height, top and bottom. That is what showed up as a
 * cut-off card in Discord.
 *
 * Stretching instead would keep every pixel but distort the artwork, which is
 * worse. So neither: the banner is scaled to fit whole (945x630, its own
 * proportions untouched) and the 128px gutter either side is filled with a
 * blurred, darkened copy of the same image. Nothing is cropped, nothing is
 * stretched, and there are no black bars — the fill reads as depth because it
 * is made of the picture it sits behind.
 *
 * Written as JPEG rather than PNG: the banner is a soft gradient with no flat
 * colour and no transparency, which is the case PNG is worst at — the same
 * card came to 914 KB as a PNG and under a fifth of that as a JPEG, with no
 * visible difference. Preview crawlers fetch this on a timeout, so weight is
 * not cosmetic.
 *
 * Run: node scripts/make-og-image.mjs
 */

import sharp from 'sharp';
import { fileURLToPath } from 'node:url';

const SOURCE = fileURLToPath(new URL('../../assets/banner.png', import.meta.url));
const OUT = fileURLToPath(new URL('../public/og.jpg', import.meta.url));

const WIDTH = 1200;
const HEIGHT = 630;

const { width: srcW, height: srcH } = await sharp(SOURCE).metadata();

// Largest size that fits inside the card with the aspect ratio intact.
const scale = Math.min(WIDTH / srcW, HEIGHT / srcH);
const fitW = Math.round(srcW * scale);
const fitH = Math.round(srcH * scale);

// The gutter filler: same image, cropped to the card, blurred past
// recognition and dimmed so it never competes with the artwork on top.
const backdrop = await sharp(SOURCE)
	.resize(WIDTH, HEIGHT, { fit: 'cover', position: 'centre' })
	.blur(48)
	.modulate({ brightness: 0.42, saturation: 1.1 })
	.toBuffer();

const artwork = await sharp(SOURCE)
	.resize(fitW, fitH, { fit: 'inside' })
	.toBuffer();

const info = await sharp(backdrop)
	.composite([
		{
			input: artwork,
			left: Math.round((WIDTH - fitW) / 2),
			top: Math.round((HEIGHT - fitH) / 2),
		},
	])
	.jpeg({ quality: 88, mozjpeg: true, chromaSubsampling: '4:4:4' })
	.toFile(OUT);

console.log(
	`og.jpg ${info.width}x${info.height}, ${Math.round(info.size / 1024)} KB — ` +
		`banner placed whole at ${fitW}x${fitH}, no crop, no stretch.`,
);
