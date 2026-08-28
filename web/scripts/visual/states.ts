/**
 * states.ts — drives hover / focus-visible / active for a board page (plan 07 V5).
 * A screenshot captures rest state only, so each state is produced explicitly:
 *
 *   hover          page-level `el.hover()` (moves the real cursor)
 *   focus-visible  `el.focus({ focusVisible: true })` — the WHATWG-sanctioned way to
 *                  request the keyboard focus-visible state programmatically; plain
 *                  `.focus()` would prove nothing (plan 07 is explicit about this)
 *   active         `mouse.move` onto the element, then `mouse.down()` held during the shot
 *
 *   node scripts/visual/states.ts --route /design/primitives            # plan 03 board
 *   node scripts/visual/states.ts --route /design/tokens --selector "[data-visual]"
 *   node scripts/visual/states.ts --states rest,hover --modes light
 *
 * Elements opt in with `data-visual="<id>"` (any value; defaults to the ordinal). Output:
 * web/.visual/states/<route>__<id>__<state>__<palette>__<mode>__<width>.png
 */
import { mkdir } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import type { ElementHandle, Page } from "puppeteer";

import { Args } from "./args.ts";
import { launch, prepare } from "./browser.ts";
import { VISUAL_ROOT } from "./capture.ts";
import { DEFAULT_PALETTE, isMode, isPalette, MODES, type Mode, type Palette } from "./matrix.ts";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

const DEFAULT_STATES = ["rest", "hover", "focus", "active"] as const;
type State = (typeof DEFAULT_STATES)[number];

function isState(v: string): v is State {
  return v === "rest" || v === "hover" || v === "focus" || v === "active";
}

/** Element box padded by 12px, clamped so the clip never starts off-canvas. */
function clipAround(box: { x: number; y: number; width: number; height: number }) {
  return {
    x: Math.max(0, box.x - 12),
    y: Math.max(0, box.y - 12),
    width: box.width + 24,
    height: box.height + 24,
  };
}

interface Target {
  handle: ElementHandle<Element>;
  id: string;
  box: { x: number; y: number; width: number; height: number };
}

async function targets(page: Page, selector: string): Promise<Target[]> {
  const handles = await page.$$(selector);
  const out: Target[] = [];
  for (let i = 0; i < handles.length; i += 1) {
    const handle = handles[i]!;
    const id = (await handle.evaluate((el) => el.getAttribute("data-visual"))) ?? `${i + 1}`;
    const box = await handle.boundingBox();
    if (!box) continue; // display:none — nothing to capture
    out.push({ handle, id, box });
  }
  return out;
}

/** Releases whatever the previous shot applied. Returns false (nothing pressed). */
async function clearState(page: Page, pressed: boolean): Promise<boolean> {
  await page.mouse.move(0, 0);
  if (pressed) await page.mouse.up();
  await page.evaluate(() => {
    if (document.activeElement instanceof HTMLElement) document.activeElement.blur();
  });
  return false;
}

/** Applies one state; returns whether the mouse button is now held down. */
async function applyState(page: Page, target: Target, state: State): Promise<boolean> {
  // Re-measure after scrollIntoView — the stored box predates any scrolling.
  const { x, y, width, height } = (await target.handle.boundingBox()) ?? target.box;
  const cx = x + width / 2;
  const cy = y + height / 2;
  if (state === "hover" || state === "active") {
    await page.mouse.move(cx, cy);
  }
  if (state === "active") {
    await page.mouse.down();
    return true;
  }
  if (state === "focus") {
    await target.handle.evaluate((el) => {
      if (el instanceof HTMLElement) el.focus({ focusVisible: true });
    });
  }
  return false;
}

export async function main(argv: string[]): Promise<number> {
  const args = new Args(argv);
  const base = args.flag("base", "http://localhost:5173").replace(/\/$/, "");
  const route = args.flag("route", "/design/tokens");
  const selector = args.flag("selector", "[data-visual]");
  const width = Number(args.flag("width", "1440"));
  const rawStates = args.list("states");
  const wanted = (
    rawStates.length > 0 ? rawStates.flatMap((s) => s.split(",")) : [...DEFAULT_STATES]
  ).filter((v) => isState(v));
  const paletteArgs = args.list("palette").filter((v) => isPalette(v));
  const modeArgs = args.list("mode").filter((v) => isMode(v));
  const palettes: Palette[] = paletteArgs.length > 0 ? paletteArgs : [DEFAULT_PALETTE];
  const modes: Mode[] = modeArgs.length > 0 ? modeArgs : [...MODES];
  if (wanted.length === 0) {
    console.error(`no valid states in --states (valid: ${DEFAULT_STATES.join(",")})`);
    return 1;
  }

  const root = path.join(args.flag("out", VISUAL_ROOT), "states");
  await mkdir(root, { recursive: true });
  const routeId = route.replace(/^\//, "").replaceAll("/", "-") || "root";
  let count = 0;

  const { browser, cleanup } = await launch();
  try {
    for (const palette of palettes) {
      for (const mode of modes) {
        const page = await browser.newPage();
        let pressed = false;
        try {
          await prepare(page, `${base}${route}`, { palette, mode, width });
          const list = await targets(page, selector);
          if (list.length === 0) {
            console.warn(`no elements match ${selector} on ${route}`);
          }
          for (const target of list) {
            for (const state of wanted) {
              pressed = await clearState(page, pressed);
              await target.handle.scrollIntoView();
              pressed = await applyState(page, target, state) || pressed;
              // Clip to the element plus breathing room; the box is re-read after
              // scrollIntoView, which can move it since the last measurement.
              const box = await target.handle.boundingBox();
              const file = `${routeId}__${target.id}__${state}__${palette}__${mode}__${width}.png`;
              await page.screenshot({
                path: path.join(root, file),
                clip: box ? clipAround(box) : undefined,
              });
              count += 1;
              console.log(`state ${file}`);
            }
          }
          pressed = await clearState(page, pressed);
        } finally {
          await page.close();
        }
      }
    }
  } finally {
    await cleanup();
  }
  console.log(`\n${count} state captures under ${root}`);
  return count > 0 ? 0 : 1;
}

const invokedDirectly = process.argv[1] === fileURLToPath(import.meta.url);
if (invokedDirectly) {
  const code = await main(process.argv.slice(2));
  process.exitCode = code;
}
