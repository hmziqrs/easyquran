/** Generate the Worker-safe canonical coordinate artifact from quran-data.xml. */
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  projectQuranCoordinates,
  scanQuranElements,
  type QuranSuraAttrs,
} from "../quran-data-source";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const out = path.resolve(scriptDir, "../src/lib/data/quran-coordinates.json");
const xmlPath = path.resolve(scriptDir, "../../db/quran/tanzil/quran-data.xml");
const xml = readFileSync(xmlPath, "utf8");
const surahs = scanQuranElements(xml, "sura") as QuranSuraAttrs[];
const coordinates = projectQuranCoordinates(surahs);
if (coordinates.surahs.length !== 114 || coordinates.rowCount !== 6236) {
  throw new Error(
    `refusing to write invalid coordinates (${coordinates.surahs.length} surahs, ${coordinates.rowCount} ayahs)`,
  );
}

mkdirSync(path.dirname(out), { recursive: true });
writeFileSync(out, JSON.stringify(coordinates, null, 2) + "\n");
console.log(`wrote ${out} (${coordinates.surahs.length} surahs, ${coordinates.rowCount} ayahs)`);
