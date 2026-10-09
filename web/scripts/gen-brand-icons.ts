import { mkdir, readFile, writeFile } from "node:fs/promises";
import sharp from "sharp";

const staticRoot = new URL("../static/", import.meta.url);
const source = await readFile(new URL("../src/lib/assets/open-pages.svg", import.meta.url), "utf8");
const pathElement = source.match(/<path\b[^>]*\/>/u)?.[0];
if (!pathElement) throw new Error("Open Pages SVG must contain a path element");
const artwork = pathElement.replace(/\s+id="[^"]*"/u, "");
const cobalt = "#315bd6";

function markSvg(fill: string): string {
  return `<svg xmlns="http://www.w3.org/2000/svg" width="512" height="512" viewBox="0 0 64 64" fill="${fill}">${artwork}</svg>\n`;
}

function tileSvg(radius: number, inset: number): string {
  const scale = (512 - inset * 2) / 64;
  return `<svg xmlns="http://www.w3.org/2000/svg" width="512" height="512" viewBox="0 0 512 512"><rect width="512" height="512" rx="${radius}" fill="${cobalt}"/><g fill="#fff" transform="translate(${inset} ${inset}) scale(${scale})">${artwork}</g></svg>\n`;
}

async function writePng(svg: string, size: number, filename: string): Promise<Buffer> {
  const density = Math.max(72, (size / 512) * 72);
  const png = await sharp(Buffer.from(svg), { density }).resize(size, size).png().toBuffer();
  await writeFile(new URL(filename, staticRoot), png);
  return png;
}

function faviconIco(images: { size: number; png: Buffer }[]): Buffer {
  const directory = Buffer.alloc(6 + images.length * 16);
  directory.writeUInt16LE(1, 2);
  directory.writeUInt16LE(images.length, 4);
  let payloadOffset = directory.length;
  for (const [index, { size, png }] of images.entries()) {
    const entryOffset = 6 + index * 16;
    directory.writeUInt8(size, entryOffset);
    directory.writeUInt8(size, entryOffset + 1);
    directory.writeUInt16LE(1, entryOffset + 4);
    directory.writeUInt16LE(32, entryOffset + 6);
    directory.writeUInt32LE(png.length, entryOffset + 8);
    directory.writeUInt32LE(payloadOffset, entryOffset + 12);
    payloadOffset += png.length;
  }
  return Buffer.concat([directory, ...images.map(({ png }) => png)]);
}

await mkdir(new URL("icons/", staticRoot), { recursive: true });
await mkdir(new URL("icons/oauth/", staticRoot), { recursive: true });
const regular = tileSvg(112, 32);
const maskable = tileSvg(0, 80);
const logo = markSvg(cobalt);
const vectorFiles = [
  ["favicon.svg", regular],
  ["icons/icon.svg", regular],
  ["icons/maskable.svg", maskable],
  ["icons/safari-pinned-tab.svg", markSvg("#000")],
  ["logo.svg", logo],
] as const;
for (const [filename, svg] of vectorFiles) {
  await writeFile(new URL(filename, staticRoot), svg);
}

const faviconImages: { size: number; png: Buffer }[] = [];
for (const size of [16, 32, 48, 192, 512]) {
  const png = await writePng(regular, size, `icons/icon-${size}.png`);
  if (size <= 48) faviconImages.push({ size, png });
}
for (const size of [192, 512]) {
  await writePng(maskable, size, `icons/maskable-${size}.png`);
}
await writePng(tileSvg(0, 32), 180, "apple-touch-icon.png");
await writePng(markSvg("#fff"), 96, "icons/badge-96.png");
await writePng(logo, 512, "logo.png");
await writeFile(new URL("favicon.ico", staticRoot), faviconIco(faviconImages));
const oauthTile = tileSvg(0, 80);
for (const [provider, size] of [
  ["github", 512],
  ["google", 120],
  ["discord", 1024],
] as const) {
  await writePng(oauthTile, size, `icons/oauth/easyquran-${provider}.png`);
}
console.log("Generated web icons from src/lib/assets/open-pages.svg");
