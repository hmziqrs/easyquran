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

async function renderPng(svg: string, size: number): Promise<Buffer> {
  const density = Math.max(72, (size / 512) * 72);
  return sharp(Buffer.from(svg), { density }).resize(size, size).png().toBuffer();
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

// Collect every artifact in memory first, then compare against disk: a byte
// difference (or missing file) is what bumps the manifest ?v= cache-buster,
// so an artwork change can never silently re-trigger stale icons under the
// immutable /icons/* cache rule.
const outputs = new Map<string, Buffer>();
for (const [filename, svg] of [
  ["favicon.svg", regular],
  ["icons/icon.svg", regular],
  ["icons/maskable.svg", maskable],
  ["icons/safari-pinned-tab.svg", markSvg("#000")],
  ["logo.svg", logo],
] as const) {
  outputs.set(filename, Buffer.from(svg, "utf8"));
}

const faviconImages: { size: number; png: Buffer }[] = [];
for (const size of [16, 32, 48, 192, 512]) {
  const png = await renderPng(regular, size);
  outputs.set(`icons/icon-${size}.png`, png);
  if (size <= 48) faviconImages.push({ size, png });
}
for (const size of [192, 512]) {
  outputs.set(`icons/maskable-${size}.png`, await renderPng(maskable, size));
}
outputs.set("icons/monochrome-512.png", await renderPng(markSvg("#000"), 512));
outputs.set("apple-touch-icon.png", await renderPng(tileSvg(0, 32), 180));
outputs.set("icons/badge-96.png", await renderPng(markSvg("#fff"), 96));
outputs.set("logo.png", await renderPng(logo, 512));
outputs.set("favicon.ico", faviconIco(faviconImages));
const oauthTile = tileSvg(0, 80);
for (const [provider, size] of [
  ["github", 512],
  ["google", 120],
  ["discord", 1024],
] as const) {
  outputs.set(`icons/oauth/easyquran-${provider}.png`, await renderPng(oauthTile, size));
}

async function readExisting(url: URL): Promise<Buffer | null> {
  try {
    return await readFile(url);
  } catch (error) {
    // SAFETY: node:fs readFile rejects with ErrnoException, and `code` is the
    // only field inspected here; any other rejection is rethrown untouched.
    if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
    return null;
  }
}

const changed: string[] = [];
for (const [filename, buffer] of outputs) {
  const existing = await readExisting(new URL(filename, staticRoot));
  if (existing && existing.equals(buffer)) continue;
  changed.push(filename);
  await writeFile(new URL(filename, staticRoot), buffer);
}

if (changed.length > 0) {
  const manifestPath = new URL("manifest.webmanifest", staticRoot);
  const manifestText = await readFile(manifestPath, "utf8");
  const versions = [...manifestText.matchAll(/\?v=(\d+)/gu)].map((match) => Number(match[1]));
  const next = Math.max(...versions, 0) + 1;
  await writeFile(manifestPath, manifestText.replace(/\?v=\d+/gu, `?v=${next}`));
  console.log(
    `Icon bytes changed (${changed.join(", ")}); bumped manifest icon version to ?v=${next}`,
  );
}
console.log("Generated web icons from src/lib/assets/open-pages.svg");
