/**
 * capture.ts — screenshots a matrix of routes × palette × mode × width (plan 07).
 *
 *   node scripts/visual/capture.ts                              # commit tier
 *   node scripts/visual/capture.ts --tier phase                 # all palettes @1440
 *   node scripts/visual/capture.ts --tier full                  # 40 combos per route
 *   node scripts/visual/capture.ts --route /design/tokens --palette sepia --mode dark
 *   node scripts/visual/capture.ts --report                     # also write .visual/index.html
 *
 * Output: web/.visual/<check>/<route>__<palette>__<mode>__<width>.png (gitignored).
 * Start the dev server first (`pnpm dev`) or pass --base http://localhost:4173.
 */
import { mkdir, readdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { Args } from "./args.ts";
import { launch, prepare } from "./browser.ts";
import {
  comboSlug,
  isMode,
  isPalette,
  parseWidth,
  ROUTES,
  tierCombos,
  type Combo,
  type RouteDef,
  type Tier,
  type Width,
} from "./matrix.ts";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
export const VISUAL_ROOT = path.resolve(__dirname, "../../.visual");

const slugId = (routePath: string): string =>
  routePath
    .replace(/^\//, "")
    .replaceAll("/", "-")
    .replaceAll("[", "")
    .replaceAll("]", "") || "root";

function resolveRoutes(args: Args): RouteDef[] {
  const requested = args.list("route");
  if (requested.length === 0) return ROUTES;
  return requested.map((req) => {
    const known = ROUTES.find((r) => r.id === req || r.path === req);
    // SAFETY: unknown routes are ad-hoc captures (e.g. /, /app/1) — no check id exists for
    // them yet, so they land under `ad-hoc/` rather than pretending to belong to a V-check.
    return known ?? { id: slugId(req), path: req, check: "ad-hoc" };
  });
}

function parseTier(v: string): Tier {
  if (v === "phase") return "phase";
  if (v === "full") return "full";
  return "commit";
}

function resolveCombos(args: Args): Combo[] {
  let combos = tierCombos(parseTier(args.flag("tier", "commit")));
  const palettes = args.list("palette").filter((v) => isPalette(v));
  const modes = args.list("mode").filter((v) => isMode(v));
  const widths = args
    .list("width")
    .map((v) => parseWidth(v))
    .filter((w): w is Width => w !== null);
  if (palettes.length > 0) combos = combos.filter((c) => palettes.includes(c.palette));
  if (modes.length > 0) combos = combos.filter((c) => modes.includes(c.mode));
  if (widths.length > 0) combos = combos.filter((c) => widths.includes(c.width));
  return combos;
}

interface Shot {
  check: string;
  file: string;
}

/** Contact sheet for the current run — one file to open instead of a directory of PNGs. */
async function writeReport(root: string): Promise<void> {
  const checkDirs = (await readdir(root, { withFileTypes: true }))
    .filter((d) => d.isDirectory())
    .map((d) => d.name)
    .sort();
  const sections: string[] = [];
  for (const check of checkDirs) {
    const files = (await readdir(path.join(root, check))).filter((f) => f.endsWith(".png")).sort();
    if (files.length === 0) continue;
    const cards = files
      .map(
        (file) =>
          `      <figure><img src="${check}/${file}" loading="lazy" alt="${file}"><figcaption>${file}</figcaption></figure>`,
      )
      .join("\n");
    sections.push(`    <section><h2>${check}</h2>\n${cards}\n    </section>`);
  }
  const html = `<!doctype html>
<html lang="en">
<head><meta charset="utf-8"><title>easyquran visual harness — contact sheet</title>
<style>
  body { margin: 0; font: 14px ui-monospace, monospace; background: #1a1a1a; color: #eee; }
  h1 { margin: 0; padding: 16px 24px; font-size: 16px; border-bottom: 1px solid #333; }
  section { padding: 16px 24px; }
  h2 { font-size: 13px; text-transform: uppercase; letter-spacing: 0.1em; color: #9a9a9a; }
  figure { margin: 0 0 20px; }
  img { display: block; max-width: 100%; border: 1px solid #333; }
  figcaption { font-size: 11px; color: #9a9a9a; padding-top: 4px; }
</style></head>
<body>
  <h1>easyquran visual harness</h1>
${sections.join("\n")}
</body>
</html>
`;
  await writeFile(path.join(root, "index.html"), html);
  console.log(`report: ${path.join(root, "index.html")}`);
}

export async function main(argv: string[]): Promise<number> {
  const args = new Args(argv);
  const base = args.flag("base", "http://localhost:5173").replace(/\/$/, "");
  const root = args.flag("out", VISUAL_ROOT);
  const routes = resolveRoutes(args);
  const combos = resolveCombos(args);

  const shots: Shot[] = [];
  const { browser, cleanup } = await launch();
  try {
    for (const route of routes) {
      const dir = path.join(root, route.check);
      await mkdir(dir, { recursive: true });
      for (const combo of combos) {
        const page = await browser.newPage();
        try {
          await prepare(page, `${base}${route.path}`, combo);
          const file = `${comboSlug(route.id, combo)}.png`;
          await page.screenshot({ path: path.join(dir, file), fullPage: true });
          shots.push({ check: route.check, file });
          console.log(`captured ${route.check}/${file}`);
        } finally {
          await page.close();
        }
      }
    }
  } finally {
    await cleanup();
  }

  if (args.has("report")) await writeReport(root);
  console.log(
    `\n${shots.length} captures under ${root} (routes: ${routes.map((r) => r.path).join(", ")})`,
  );
  return shots.length > 0 ? 0 : 1;
}

const invokedDirectly = process.argv[1] === fileURLToPath(import.meta.url);
if (invokedDirectly) {
  const code = await main(process.argv.slice(2));
  process.exitCode = code;
}
