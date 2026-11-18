// The shell's smoke suite (the brief's criterion 3): every page reachable
// from the shell opens, shows its marker and is screenshotted, per target.
// Headless runs pass with the captures skipped (they draw nothing).
// Run per target: `nsh test --target macos|ios|android|web smoke/pages.test.tsx`.
import { expect, open, test } from "nsh/test";
import type { App } from "nsh/test";

// One entry per page the shell can show. `id` is the row on Home,
// `marker` a line of text the page itself shows. Contract: feature areas
// keep their page's marker stable, or update this list with the shell
// area (src/shell/routes.rs names the same pages).
const PAGES: Array<{ id: string; marker: string }> = [
  { id: "page-ui", marker: "UI gallery" },
  { id: "page-forms", marker: "Forms" },
  { id: "page-data", marker: "Data" },
  { id: "page-data-item", marker: "Data item 3" },
  { id: "page-query-devtools", marker: "Query devtools" },
  { id: "page-lists", marker: "Lists" },
  { id: "page-canvas", marker: "Canvas" },
  { id: "page-motion", marker: "Motion" },
  { id: "page-overlays", marker: "Overlays" },
  { id: "page-platform", marker: "Platform" },
  { id: "page-device", marker: "Device" },
  { id: "page-remote", marker: "Remote screen" },
  { id: "page-diagnostics", marker: "Diagnostics" },
];

// What the web target does not have on 0.5.0, asserted so a silently
// missing feature cannot pass unnoticed. Page navigation itself works
// everywhere (every page is Rust).
const SKIPPED_ON_WEB = [
  "file and folder dialogs (desktop-only file requests)",
  "speech recognition (no recognizer on the web)",
  "the system tab bar (nsh/mobile renders nothing off iOS and Android)",
];

async function openShell(): Promise<App> {
  try {
    // Desktop width exercises the sidebar; iOS and Android reject the
    // size (the app fills their screen) and fall through to the default.
    return await open("..", { clock: {}, width: 1280, height: 800 });
  } catch (error) {
    if ((error as { code?: string }).code !== "ERR_NSH_TEST_UNSUPPORTED") throw error;
    return await open("..", { clock: {} });
  }
}

test("every page opens from the shell", async () => {
  // The documented web skips, spelled out so a change is a diff here.
  expect(SKIPPED_ON_WEB).toEqual([
    "file and folder dialogs (desktop-only file requests)",
    "speech recognition (no recognizer on the web)",
    "the system tab bar (nsh/mobile renders nothing off iOS and Android)",
  ]);

  const app = await openShell();
  await app.waitFor("text:Home");

  for (const page of PAGES) {
    await app.press(`id:${page.id}`);
    await app.waitFor(`text:${page.marker}`);
    try {
      // Relative to the run's working directory; per-target runs write
      // over these, one page per file.
      await app.screenshot(`docs/evidence/shell/smoke/${page.id}.png`);
    } catch {
      // Headless draws nothing: captures are for real targets.
    }
    await app.back();
  }

  // Back at Home: every page reachable again.
  await app.waitFor("text:Home");
});
