/* ════════════════════════════════════════════════════════════════════════
   quran-meta.ts — re-export of the build-time Quran metadata virtual module.

   `quran-meta:data` is produced by quranData() in vite-plugin-quran.ts from
   surah-names.json + quran-data.xml. Its TypeScript shape is declared in the
   sibling quran-meta.d.ts so svelte-check type-checks the re-export.
   ════════════════════════════════════════════════════════════════════════ */

export { CATALOG, NAVIGATION, SAJDAS } from "quran-meta:data";
