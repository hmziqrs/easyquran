import { readFileSync, writeFileSync, statSync, existsSync, readdirSync, mkdirSync } from "node:fs";
import path from "node:path";
import { gzipSync, brotliCompressSync } from "node:zlib";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const BUILD = path.resolve(__dirname, "..", "build");
const APP = path.join(BUILD, "app");
const label = process.argv[2] ?? "phase0";

interface FileMeasures {
  raw: number;
  gzip: number;
  brotli: number;
}

function measure(p: string): FileMeasures | null {
  if (!existsSync(p)) return null;
  const buf = readFileSync(p);
  return { raw: buf.length, gzip: gzipSync(buf).length, brotli: brotliCompressSync(buf).length };
}

const MODULEPRELOAD_TAG = /<link\b[^>]*\brel="modulepreload"[^>]*>/g;
const HREF = /href="([^"]+)"/;

interface CriticalJs extends FileMeasures {
  files: number;
  largest: { href: string; raw: number } | null;
}

function criticalJs(htmlPath: string): CriticalJs {
  const html = readFileSync(htmlPath, "utf8");
  const base = path.dirname(htmlPath);
  const seen = new Set<string>();
  let raw = 0;
  let gzip = 0;
  let brotli = 0;
  let files = 0;
  let largest: { href: string; raw: number } | null = null;
  for (const tag of html.matchAll(MODULEPRELOAD_TAG)) {
    const m = HREF.exec(tag[0]);
    if (!m) continue;
    const resolved = path.resolve(base, m[1]!);
    if (seen.has(resolved)) continue;
    seen.add(resolved);
    const fm = measure(resolved);
    if (!fm) continue;
    raw += fm.raw;
    gzip += fm.gzip;
    brotli += fm.brotli;
    files += 1;
    if (!largest || fm.raw > largest.raw) largest = { href: m[1]!, raw: fm.raw };
  }
  return { files, raw, gzip, brotli, largest };
}

interface RouteMeasures {
  route: string;
  html: FileMeasures | null;
  data: FileMeasures | null;
  js: CriticalJs;
}

function measureRoute(routeFile: string): RouteMeasures {
  const htmlPath = path.join(APP, routeFile);
  const dataPath = path.join(APP, routeFile.replace(/\.html$/, ""), "__data.json");
  return {
    route: routeFile.replace(/\.html$/, ""),
    html: measure(htmlPath),
    data: measure(dataPath),
    js: criticalJs(htmlPath),
  };
}

const ROUTES = [
  "al-fatihah.html",
  "al-baqarah.html",
  "al-kahf.html",
  "an-nas.html",
  "page/1.html",
  "page/2.html",
  "page/150.html",
  "page/300.html",
  "page/604.html",
  "juz/1.html",
  "juz/15.html",
  "juz/30.html",
];

const perRoute = ROUTES.map((route) => measureRoute(route));

function listReaderHtml(): string[] {
  if (!existsSync(APP)) return [];
  const files: string[] = [];
  const stack = [APP];
  while (stack.length > 0) {
    const current = stack.pop()!;
    for (const entry of readdirSync(current, { withFileTypes: true })) {
      const full = path.join(current, entry.name);
      if (entry.isDirectory()) stack.push(full);
      else if (entry.name.endsWith(".html")) files.push(path.relative(APP, full));
    }
  }
  return files.sort();
}

const allReaderHtml = listReaderHtml();

function aggregate(files: string[]): { docs: number; raw: number; gzip: number } {
  let raw = 0;
  let gzip = 0;
  for (const f of files) {
    const file = path.join(APP, f);
    if (!existsSync(file)) continue;
    const buffer = readFileSync(file);
    raw += buffer.length;
    gzip += gzipSync(buffer).length;
  }
  return { docs: files.length, raw, gzip };
}

const readerHtmlAgg = aggregate(allReaderHtml);
const readerDataAgg = aggregate(allReaderHtml.map((f) => f.replace(/\.html$/, "/__data.json")));

function dirSize(dir: string): number {
  let total = 0;
  const stack = [dir];
  while (stack.length) {
    const cur = stack.pop()!;
    for (const entry of readdirSync(cur)) {
      const full = path.join(cur, entry);
      const st = statSync(full);
      if (st.isDirectory()) stack.push(full);
      else total += st.size;
    }
  }
  return total;
}

const buildSize = existsSync(BUILD) ? dirSize(BUILD) : 0;

const kb = (n: number): string => (n / 1024).toFixed(1).padStart(7);

console.log(`\n=== EasyQuran SSG baseline (${label}) ===\n`);
console.log(
  "route                  html(raw/gz/br KB)        data(raw/gz/br KB)      criticalJS(raw/gz/br KB, n)",
);
for (const r of perRoute) {
  const f = (m: FileMeasures | null): string =>
    m ? `${kb(m.raw)}/${kb(m.gzip)}/${kb(m.brotli)}` : "missing";
  const largest = r.js.largest
    ? ` [${path.basename(r.js.largest.href)} ${kb(r.js.largest.raw)}]`
    : "";
  console.log(
    `${r.route.padEnd(20)}  ${f(r.html)}   ${f(r.data)}   ${f(r.js)} (${r.js.files})${largest}`,
  );
}

console.log("\n=== Aggregate reader output ===");
console.log(`reader docs:        ${readerHtmlAgg.docs}`);
console.log(
  `reader HTML total:  ${kb(readerHtmlAgg.raw)} KB raw / ${kb(readerHtmlAgg.gzip)} KB gzip`,
);
console.log(
  `reader data total:  ${kb(readerDataAgg.raw)} KB raw / ${kb(readerDataAgg.gzip)} KB gzip`,
);
console.log(`entire build/:      ${kb(buildSize)} KB raw`);

const outDir = path.resolve(__dirname, "..", "baselines");
mkdirSync(outDir, { recursive: true });
const snapshot = {
  label,
  perRoute,
  aggregate: {
    readerDocs: readerHtmlAgg.docs,
    readerHtml: readerHtmlAgg,
    readerData: readerDataAgg,
    buildSize,
  },
};
const outPath = path.join(outDir, `${label}.json`);
writeFileSync(outPath, JSON.stringify(snapshot, null, 2) + "\n");
console.log(`\nwrote ${path.relative(process.cwd(), outPath)}`);
