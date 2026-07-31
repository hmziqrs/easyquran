/* ════════════════════════════════════════════════════════════════════════
   vite-plugin-quran.ts — build-time Quran metadata generator.

   Produces the `quran-meta:data` virtual module by merging:
     • web/src/lib/data/surah-names.json  — the web-owned {num, slug, name} (114)
     • db/quran/tanzil/quran-data.xml     — Arabic name, place, ayah/revelation
                                            counts, rukus, and all navigation
                                            start markers (juz/page/ruku/
                                            hizb-quarter/manzil) + sajdas

   Output is pure data (no 77 KB XML or parser shipped to the client) and is
   shared by the SSG server load and the client bundle. Verse TEXT is never
   produced here — it is read from quran-uthmani.sqlite at SSG time and from the
   cached sqlite-wasm DB in the browser Worker.

   The virtual module is re-exported and typed via src/lib/data/quran-meta.ts
   and quran-meta.d.ts.
   ════════════════════════════════════════════════════════════════════════ */

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import type { Plugin } from "vite";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const WEB_ROOT = __dirname;
const NAMES_PATH = path.join(WEB_ROOT, "src/lib/data/surah-names.json");
const XML_PATH = path.join(WEB_ROOT, "../db/quran/tanzil/quran-data.xml");

export const QURAN_META_VIRTUAL = "quran-meta:data";
const RESOLVED = "\0" + QURAN_META_VIRTUAL;

/** Expected golden counts (docs/quran-api.md §4 / quran-web-delivery §4). */
const EXPECT = {
  suras: 114,
  juzs: 30,
  pages: 604,
  rukus: 556,
  quarters: 240,
  manzils: 7,
  sajdas: 15,
  ayahs: 6236,
} as const;

type Attrs = Record<string, string>;

/** Pull `name="value"` pairs off a self-closing element's inner text. */
function extractAttrs(inner: string): Attrs {
  const out: Attrs = {};
  const re = /(\w+)="([^"]*)"/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(inner))) out[m[1]!] = m[2]!;
  return out;
}

/** Scan all self-closing `<tag ... />` elements of one tag name, in order. */
function scanEls(xml: string, tag: string): Attrs[] {
  const out: Attrs[] = [];
  const re = new RegExp(`<${tag}\\b([^/>]*)/>`, "g");
  let m: RegExpExecArray | null;
  while ((m = re.exec(xml))) out.push(extractAttrs(m[1]!));
  return out;
}

function assert(cond: boolean, msg: string): void {
  if (!cond) throw new Error(`[easyquran:quran-data] ${msg}`);
}

interface RawSura extends Attrs {
  index: string;
  ayas: string;
  start: string;
  name: string;
  tname: string;
  ename: string;
  type: string;
  order: string;
  rukus: string;
}

interface Compiled {
  catalog: unknown[];
  navigation: unknown;
  sajdas: unknown[];
}

function compile(): Compiled {
  const xml = readFileSync(XML_PATH, "utf8");
  const names = JSON.parse(readFileSync(NAMES_PATH, "utf8")) as {
    num: number;
    slug: string;
    name: string;
  }[];

  const suras = scanEls(xml, "sura") as RawSura[];
  assert(suras.length === EXPECT.suras, `expected ${EXPECT.suras} suras, got ${suras.length}`);

  const nameByNum = new Map(names.map((n) => [n.num, n]));
  /** global ayah index (1-based) of surah s's first ayah = xml start + 1. */
  const startGlobalOf = (s: RawSura) => Number(s.start) + 1;
  const ayasOf = (s: RawSura) => Number(s.ayas);

  // ── catalog: authored slug/name ∪ XML-derived fields ──────────────────
  const catalog = suras.map((s) => {
    const num = Number(s.index);
    const auth = nameByNum.get(num);
    assert(!!auth, `surah-names.json missing entry for surah ${num}`);
    const place = s.type.toLowerCase() === "meccan" ? "meccan" : "medinan";
    const bismillah = num === 1 ? "first-ayah" : num === 9 ? "none" : "embedded-prefix";
    return {
      num,
      slug: auth!.slug,
      name: auth!.name,
      arabic: s.name,
      place,
      ayahCount: Number(s.ayas),
      revelationOrder: Number(s.order),
      rukus: Number(s.rukus),
      bismillah,
      startGlobal: startGlobalOf(s),
    };
  });

  // ordered surah boundaries for global → (surah, ayah) lookup
  const bounds = suras.map((s) => {
    const start = startGlobalOf(s);
    return { num: Number(s.index), start, end: start + ayasOf(s) - 1 };
  });
  const totalAyahs = bounds[bounds.length - 1]!.end;
  assert(totalAyahs === EXPECT.ayahs, `expected ${EXPECT.ayahs} ayahs, got ${totalAyahs}`);

  /** Convert a 1-based global ayah index to "surah:ayah". */
  const globalToKey = (g: number): string => {
    for (const b of bounds) {
      if (g >= b.start && g <= b.end) return `${b.num}:${g - b.start + 1}`;
    }
    return "1:1";
  };

  /** Build inclusive [startGlobal,endGlobal] ranges from start-marker elements. */
  const ranges = (tag: string) => {
    const els = scanEls(xml, tag);
    const starts = els.map((e) => ({
      index: Number(e.index),
      // global index of (sura, aya) = start(sura, 0-based) + aya = startGlobalOf(sura) + aya - 1
      startGlobal: startGlobalOf(suras[Number(e.sura) - 1]!) + Number(e.aya) - 1,
      first: `${Number(e.sura)}:${Number(e.aya)}`,
    }));
    return starts.map((s, i) => {
      const endGlobal = i < starts.length - 1 ? starts[i + 1]!.startGlobal - 1 : totalAyahs;
      return {
        index: s.index,
        startGlobal: s.startGlobal,
        endGlobal,
        first: s.first,
        last: globalToKey(endGlobal),
      };
    });
  };

  const juz = ranges("juz");
  const page = ranges("page");
  const ruku = ranges("ruku");
  const hizbQuarter = ranges("quarter");
  const manzil = ranges("manzil");
  assert(juz.length === EXPECT.juzs, `juzs ${juz.length}`);
  assert(page.length === EXPECT.pages, `pages ${page.length}`);
  assert(ruku.length === EXPECT.rukus, `rukus ${ruku.length}`);
  assert(hizbQuarter.length === EXPECT.quarters, `quarters ${hizbQuarter.length}`);
  assert(manzil.length === EXPECT.manzils, `manzils ${manzil.length}`);

  const sajdaEls = scanEls(xml, "sajda");
  assert(sajdaEls.length === EXPECT.sajdas, `sajdas ${sajdaEls.length}`);
  const sajdas = sajdaEls.map((e) => ({
    index: Number(e.index),
    surah: Number(e.sura),
    ayah: Number(e.aya),
    globalIndex: startGlobalOf(suras[Number(e.sura) - 1]!) + Number(e.aya) - 1,
    kind: e.type === "obligatory" ? "obligatory" : "recommended",
  }));

  return {
    catalog,
    navigation: { juz, page, ruku, hizbQuarter, manzil },
    sajdas,
  };
}

/** Build (and memoize) the virtual module source. */
let cached: string | null = null;
function generate(): string {
  if (cached) return cached;
  const { catalog, navigation, sajdas } = compile();
  cached = [
    "// AUTO-GENERATED by vite-plugin-quran.ts — do not edit.",
    "export const CATALOG = " + JSON.stringify(catalog) + ";",
    "export const NAVIGATION = " + JSON.stringify(navigation) + ";",
    "export const SAJDAS = " + JSON.stringify(sajdas) + ";",
    "",
  ].join("\n");
  return cached;
}

export function quranData(): Plugin {
  return {
    name: "easyquran:quran-data",
    enforce: "pre",
    resolveId(id) {
      return id === QURAN_META_VIRTUAL ? RESOLVED : null;
    },
    load(id) {
      if (id !== RESOLVED) return null;
      return generate();
    },
    configureServer(server) {
      // Regenerate if a source changes during dev.
      server.watcher.add([NAMES_PATH, XML_PATH]);
      server.watcher.on("change", (f) => {
        if (f === NAMES_PATH || f === XML_PATH) {
          cached = null;
          const mod = server.moduleGraph.getModuleById(RESOLVED);
          if (mod) void server.reloadModule(mod);
        }
      });
    },
  };
}
