/**
 * Shared Puppeteer plumbing for the visual harness (docs/plan/07-visual-verification.md).
 *
 * Every launch uses a throwaway profile under the OS temp dir — never the developer's own
 * Chrome, never a shared session — so captures cannot inherit extensions, logged-in state,
 * or cached themes. `prepare()` centralises the two waits that make screenshots stable:
 * the palette/mode attributes are set before anything is measured, and `document.fonts`
 * must be ready before every screenshot or a font load races the capture and every diff
 * is noise.
 */
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";

import puppeteer, { type Browser, type Page } from "puppeteer";

export interface Launch {
  browser: Browser;
  /** Removes the throwaway profile. Always call in a finally block. */
  cleanup: () => Promise<void>;
}

export async function launch(): Promise<Launch> {
  const profile = await mkdtemp(path.join(tmpdir(), "easyquran-visual-"));
  const browser = await puppeteer.launch({
    userDataDir: profile,
    headless: true,
    args: ["--force-color-profile=srgb", "--disable-lcd-text"],
  });
  return {
    browser,
    cleanup: async () => {
      await browser.close();
      await rm(profile, { recursive: true, force: true });
    },
  };
}

export interface PrepareOptions {
  palette?: string;
  mode?: string;
  width: number;
  height?: number;
}

/**
 * Navigates, applies the palette/mode attributes the way prefs.svelte.ts `apply()` does
 * (direct dataset writes — no UI driving, no flake), waits for fonts, then waits two
 * animation frames so style recalculation from the attribute change is settled.
 */
export async function prepare(page: Page, url: string, opts: PrepareOptions): Promise<void> {
  await page.setViewport({ width: opts.width, height: opts.height ?? 900 });
  await page.goto(url, { waitUntil: "networkidle2", timeout: 60_000 });
  if (opts.palette !== undefined && opts.mode !== undefined) {
    await page.evaluate(([palette, mode]) => {
      document.documentElement.dataset.palette = palette;
      document.documentElement.dataset.mode = mode;
    }, [opts.palette, opts.mode]);
  }
  await page.evaluate(() => document.fonts.ready);
  await page.evaluate(
    () =>
      new Promise<void>((resolve) => {
        requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
      }),
  );
}
