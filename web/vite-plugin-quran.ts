import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import type { Plugin } from "vite";
import {
  projectQuranCoordinates,
  scanQuranElements,
  type QuranSuraAttrs,
} from "./quran-data-source";
import { Bismillah } from "./src/lib/data/quran-types";
import { canonicalOpenerKind } from "./src/lib/quran/view/canonical";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const WEB_ROOT = __dirname;
const NAMES_PATH = path.join(WEB_ROOT, "src/lib/data/surah-names.json");
const COORDINATES_PATH = path.join(WEB_ROOT, "src/lib/data/quran-coordinates.json");
const XML_PATH = path.join(WEB_ROOT, "../db/quran/tanzil/quran-data.xml");

export const QURAN_META_VIRTUAL = "quran-meta:data";
const RESOLVED = "\0" + QURAN_META_VIRTUAL;

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

function assert(cond: boolean, msg: string): void {
  if (!cond) throw new Error(`[easyquran:quran-data] ${msg}`);
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

  const suras = scanQuranElements(xml, "sura") as QuranSuraAttrs[];
  assert(suras.length === EXPECT.suras, `expected ${EXPECT.suras} suras, got ${suras.length}`);

  const nameByNum = new Map(names.map((n) => [n.num, n]));
  const startGlobalOf = (s: QuranSuraAttrs) => Number(s.start) + 1;
  const ayasOf = (s: QuranSuraAttrs) => Number(s.ayas);

  const catalog = suras.map((s) => {
    const num = Number(s.index);
    const auth = nameByNum.get(num);
    assert(!!auth, `surah-names.json missing entry for surah ${num}`);
    const place = s.type.toLowerCase() === "meccan" ? "meccan" : "medinan";
    const openerKind = canonicalOpenerKind(num);
    const bismillah =
      num === 1 ? Bismillah.FirstAyah : num === 9 ? Bismillah.None : Bismillah.EmbeddedPrefix;
    return {
      num,
      slug: auth!.slug,
      name: auth!.name,
      arabic: s.name,
      place,
      ayahCount: Number(s.ayas),
      revelationOrder: Number(s.order),
      rukus: Number(s.rukus),
      openerKind,
      bismillah,
      startGlobal: startGlobalOf(s),
    };
  });

  const bounds = suras.map((s) => {
    const start = startGlobalOf(s);
    return { num: Number(s.index), start, end: start + ayasOf(s) - 1 };
  });
  const totalAyahs = bounds[bounds.length - 1]!.end;
  assert(totalAyahs === EXPECT.ayahs, `expected ${EXPECT.ayahs} ayahs, got ${totalAyahs}`);
  const projectedCoordinates = projectQuranCoordinates(suras);
  assert(
    projectedCoordinates.rowCount === totalAyahs,
    `coordinate projection ${projectedCoordinates.rowCount} != ${totalAyahs}`,
  );

  const coordinates = JSON.parse(
    readFileSync(COORDINATES_PATH, "utf8"),
  ) as typeof projectedCoordinates;
  assert(
    coordinates.rowCount === totalAyahs,
    "quran-coordinates.json row count drifted from quran-data.xml",
  );
  assert(
    Array.isArray(coordinates.surahs) && coordinates.surahs.length === bounds.length,
    "quran-coordinates.json surah count drifted from quran-data.xml",
  );
  for (const [index, bound] of bounds.entries()) {
    const coordinate = coordinates.surahs[index];
    assert(
      coordinate?.surah === bound.num &&
        coordinate.startGlobal === bound.start &&
        coordinate.ayahCount === bound.end - bound.start + 1,
      `quran-coordinates.json drifted at surah ${bound.num}`,
    );
  }

  const globalToKey = (g: number): string => {
    for (const b of bounds) {
      if (g >= b.start && g <= b.end) return `${b.num}:${g - b.start + 1}`;
    }
    return "1:1";
  };

  const ranges = (tag: string) => {
    const els = scanQuranElements(xml, tag);
    const starts = els.map((e) => ({
      index: Number(e.index),
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

  const sajdaEls = scanQuranElements(xml, "sajda");
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
      server.watcher.add([NAMES_PATH, XML_PATH, COORDINATES_PATH]);
      server.watcher.on("change", (f) => {
        if (f === NAMES_PATH || f === XML_PATH || f === COORDINATES_PATH) {
          cached = null;
          const mod = server.moduleGraph.getModuleById(RESOLVED);
          if (mod) void server.reloadModule(mod);
        }
      });
    },
  };
}
