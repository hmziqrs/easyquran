/**
 * reference.ts — renders design/*.html in the harness browser for side-by-side capture
 * (plan 07 V8 support). The reference boards are Design Component files: `.dc.html` wraps
 * the page in `<x-dc>` and parks its stylesheets inside `<helmet>`; both are unwrapped
 * in-page after load so the browser parses the file with its relative vendor scripts
 * intact. The dark board is already plain HTML and passes through untouched.
 *
 *   node scripts/visual/reference.ts                 # both boards @1440
 *   node scripts/visual/reference.ts --width 390
 *   node scripts/visual/reference.ts --file <path>
 *
 * Output: web/.visual/reference/<stem>__<width>.png. The boards use CDN fonts (allowed —
 * they are reference material, never a build input); with no network they render in the
 * fallback face and the geometry comparison still holds.
 */
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import type { Page } from "puppeteer";

import { Args } from "./args.ts";
import { launch, prepare } from "./browser.ts";
import { VISUAL_ROOT } from "./capture.ts";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const DESIGN_DIR = path.resolve(__dirname, "../../../design");

const DEFAULT_BOARDS = ["PillLightCobalt.dc.html", "white-on-colour-deep-cobalt.html"];

/** Strips the DC wrapper: helmet children move into <head>, x-dc children move to <body>. */
async function unwrapDesignComponent(page: Page): Promise<void> {
  const dc = await page.$("x-dc");
  if (!dc) return;
  await page.evaluate((wrapper) => {
    const helmet = wrapper.querySelector("helmet");
    if (helmet) {
      // Array.from: appending each node MOVES it, so the live NodeList must be snapshotted.
      for (const node of Array.from(helmet.childNodes)) document.head.append(node);
    }
    for (const node of Array.from(wrapper.childNodes)) wrapper.parentNode?.insertBefore(node, wrapper);
    wrapper.remove();
  }, dc);
}

export async function main(argv: string[]): Promise<number> {
  const args = new Args(argv);
  const width = Number(args.flag("width", "1440"));
  const root = path.join(args.flag("out", VISUAL_ROOT), "reference");
  const files = args.list("file").length > 0 ? args.list("file") : DEFAULT_BOARDS;

  await mkdir(root, { recursive: true });
  const written: string[] = [];
  const { browser, cleanup } = await launch();
  try {
    for (const file of files) {
      const full = path.resolve(file) === file ? file : path.join(DESIGN_DIR, file);
      let html: string;
      try {
        html = await readFile(full, "utf8");
      } catch {
        console.error(`reference board not found: ${full} (design/ is untracked — see TASK)`);
        continue;
      }
      const page = await browser.newPage();
      try {
        // Navigate the real file URL (not setContent) so ./vendor/*.js resolves on disk.
        await prepare(page, pathToFileURL(full).href, { width });
        if (full.endsWith(".dc.html")) await unwrapDesignComponent(page);
        await page.evaluate(() => document.fonts.ready);
        const stem = path.basename(file).replace(/\.dc?\.html$/, "").replace(/\.html$/, "");
        const out = `${stem}__${width}.png`;
        await page.screenshot({ path: path.join(root, out), fullPage: true });
        written.push(out);
        console.log(`reference ${out} (${html.length} bytes source)`);
      } finally {
        await page.close();
      }
    }
  } finally {
    await cleanup();
  }

  if (args.has("report")) {
    const cards = written.map((f) => `  <img src="reference/${f}" alt="${f}">`).join("\n");
    await writeFile(
      path.join(VISUAL_ROOT, "reference.html"),
      `<!doctype html><meta charset="utf-8"><title>reference boards</title>\n<style>body{margin:0;background:#1a1a1a}img{display:block;max-width:100%;border-bottom:1px solid #333}</style>\n${cards}\n`,
    );
  }
  return written.length > 0 ? 0 : 1;
}

const invokedDirectly = process.argv[1] === fileURLToPath(import.meta.url);
if (invokedDirectly) {
  const code = await main(process.argv.slice(2));
  process.exitCode = code;
}
