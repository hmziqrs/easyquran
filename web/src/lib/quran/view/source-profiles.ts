import {
  OPENER_PACKAGING_VALUES,
  OpenerPackaging,
  QuranScript,
  QuranSourceId,
  type QuranScript as QuranScriptValue,
  type QuranSourceId as QuranSourceIdValue,
  type OpenerPackaging as OpenerPackagingValue,
} from "../../data/quran-types.ts";
import {
  TANZIL_QURAN_DATABASE,
  decodeIntegerField,
  decodeTextField,
  defineQuranDatabaseAdapter,
  defineQuranQuery,
  type FirstAyahRow,
  type QuranDatabaseAdapter,
} from "../sql.ts";

export interface QuranSourceArtifact {
  readonly repositoryPath: string;
  readonly r2Path: string;
  readonly sizeBytes: number;
}

export interface QuranSourceProfile {
  readonly id: string;
  readonly sourceId: QuranSourceIdValue;
  readonly script: QuranScriptValue;
  readonly artifact: QuranSourceArtifact;
  readonly database: QuranDatabaseAdapter;
  readonly canonicalRowCount: number;
  readonly packagingBySurah: readonly OpenerPackagingValue[];
  readonly expectedPackagingCounts: Readonly<Record<OpenerPackagingValue, number>>;
  readonly referenceOpenerSurah: number;
}

export function defineSourceProfile(profile: QuranSourceProfile): QuranSourceProfile {
  if (profile.packagingBySurah.length !== 115) {
    throw new Error(`[quran-view:${profile.id}] packaging registry must contain indexes 0..114`);
  }
  // SAFETY: OPENER_PACKAGING_VALUES enumerates every OpenerPackagingValue exactly once, so
  // Object.fromEntries builds precisely the keys of Record<OpenerPackagingValue, number>, all 0.
  const observed = Object.fromEntries(OPENER_PACKAGING_VALUES.map((kind) => [kind, 0])) as Record<
    OpenerPackagingValue,
    number
  >;
  for (let surah = 1; surah <= 114; surah++) {
    const packaging = profile.packagingBySurah[surah];
    if (!packaging)
      throw new Error(`[quran-view:${profile.id}] missing packaging for surah ${surah}`);
    observed[packaging] += 1;
  }
  for (const kind of OPENER_PACKAGING_VALUES) {
    if (observed[kind] !== profile.expectedPackagingCounts[kind]) {
      throw new Error(
        `[quran-view:${profile.id}] ${kind} registry count ${observed[kind]} != ${profile.expectedPackagingCounts[kind]}`,
      );
    }
  }
  return Object.freeze(profile);
}

const TANZIL_PACKAGING = Object.freeze(
  Array.from({ length: 115 }, (_, surah): OpenerPackagingValue => {
    if (surah === 1) return OpenerPackaging.NumberedAyah;
    if (surah === 9 || surah === 0) return OpenerPackaging.Absent;
    return OpenerPackaging.EmbeddedPrefix;
  }),
);

const TANZIL_COUNTS: Readonly<Record<OpenerPackagingValue, number>> = Object.freeze({
  [OpenerPackaging.NumberedAyah]: 1,
  [OpenerPackaging.EmbeddedPrefix]: 112,
  [OpenerPackaging.ChapterFlag]: 0,
  [OpenerPackaging.SeparateRow]: 0,
  [OpenerPackaging.Absent]: 1,
});

// The IndoPak and Tajweed variants do NOT embed the bismillah prefix inside each
// surah's first ayah (measured on the built DBs — only surah 1 carries it, as
// verse 1). Openers therefore ride as a separate row: the trusted opener text is
// each DB's own 1:1 (the canonical bismillah, markup included for tajweed).
const VARIANT_PACKAGING = Object.freeze(
  Array.from({ length: 115 }, (_, surah): OpenerPackagingValue => {
    if (surah === 1) return OpenerPackaging.NumberedAyah;
    if (surah === 9 || surah === 0) return OpenerPackaging.Absent;
    return OpenerPackaging.SeparateRow;
  }),
);

const VARIANT_COUNTS: Readonly<Record<OpenerPackagingValue, number>> = Object.freeze({
  [OpenerPackaging.NumberedAyah]: 1,
  [OpenerPackaging.EmbeddedPrefix]: 0,
  [OpenerPackaging.ChapterFlag]: 0,
  [OpenerPackaging.SeparateRow]: 112,
  [OpenerPackaging.Absent]: 1,
});

/** Same quran_text schema, plus the trusted-opener query the variants need. */
const VARIANT_QURAN_DATABASE: QuranDatabaseAdapter = defineQuranDatabaseAdapter({
  id: "variant-quran-text-v1",
  queries: Object.freeze({
    ...TANZIL_QURAN_DATABASE.queries,
    openers: defineQuranQuery(
      `SELECT sura AS surah, (SELECT text FROM quran_text WHERE sura = 1 AND aya = 1) AS text
       FROM quran_text WHERE aya = 1 AND sura > 1 AND sura <> 9 ORDER BY sura`,
      (row) => ({ surah: decodeIntegerField(row, "surah"), text: decodeTextField(row) }) satisfies FirstAyahRow,
    ),
  }),
});

const PROFILES = Object.freeze([
  defineSourceProfile({
    id: "tanzil-uthmani-581cc540",
    sourceId: QuranSourceId.TanzilUthmani,
    script: QuranScript.Uthmani,
    artifact: Object.freeze({
      repositoryPath: "db/quran/arabic/quran-uthmani.sqlite",
      r2Path: "tanzil/arabic/quran-uthmani.sqlite",
      sizeBytes: 1_593_344,
    }),
    database: TANZIL_QURAN_DATABASE,
    canonicalRowCount: 6236,
    packagingBySurah: TANZIL_PACKAGING,
    expectedPackagingCounts: TANZIL_COUNTS,
    referenceOpenerSurah: 1,
  }),
  defineSourceProfile({
    id: "tanzil-simple-clean-a0c52760",
    sourceId: QuranSourceId.TanzilSimpleClean,
    script: QuranScript.SimpleClean,
    artifact: Object.freeze({
      repositoryPath: "db/quran/arabic/quran-simple-clean.sqlite",
      r2Path: "tanzil/arabic/quran-simple-clean.sqlite",
      sizeBytes: 929_792,
    }),
    database: TANZIL_QURAN_DATABASE,
    canonicalRowCount: 6236,
    packagingBySurah: TANZIL_PACKAGING,
    expectedPackagingCounts: TANZIL_COUNTS,
    referenceOpenerSurah: 1,
  }),
  // IndoPak mushaf text (Naveed Ahmad / Quran.com-lineage Naskh script). Bismillah
  // is NOT embedded in first ayahs (measured) — SeparateRow openers come from the
  // DB's own 1:1 via VARIANT_QURAN_DATABASE.queries.openers.
  defineSourceProfile({
    id: "indopak-naveed-7d3c21e0",
    sourceId: QuranSourceId.Indopak,
    script: QuranScript.IndoPak,
    artifact: Object.freeze({
      repositoryPath: "db/quran/arabic/quran-indopak.sqlite",
      r2Path: "tanzil/arabic/quran-indopak.sqlite",
      sizeBytes: 1_634_304,
    }),
    database: VARIANT_QURAN_DATABASE,
    canonicalRowCount: 6236,
    packagingBySurah: VARIANT_PACKAGING,
    expectedPackagingCounts: VARIANT_COUNTS,
    referenceOpenerSurah: 1,
  }),
  // Tajweed mushaf text (Dar Al-Islam colored tajweed, via alquran.cloud). Text
  // column carries inline tajweed markup (`[h:1468[ٱ]`-style segments) verbatim —
  // A4 renders/parses it; stripping happens in views, never in the DB.
  defineSourceProfile({
    id: "tajweed-daralislam-5b9f48d2",
    sourceId: QuranSourceId.Tajweed,
    script: QuranScript.Tajweed,
    artifact: Object.freeze({
      repositoryPath: "db/quran/arabic/quran-tajweed.sqlite",
      r2Path: "tanzil/arabic/quran-tajweed.sqlite",
      sizeBytes: 2_015_232,
    }),
    database: VARIANT_QURAN_DATABASE,
    canonicalRowCount: 6236,
    packagingBySurah: VARIANT_PACKAGING,
    expectedPackagingCounts: VARIANT_COUNTS,
    referenceOpenerSurah: 1,
  }),
] satisfies readonly QuranSourceProfile[]);

const PROFILE_BY_SOURCE = new Map(PROFILES.map((profile) => [profile.sourceId, profile]));

export function registeredSourceProfiles(): readonly QuranSourceProfile[] {
  return PROFILES;
}

export function sourceProfile(sourceId: QuranSourceIdValue): QuranSourceProfile {
  const profile = PROFILE_BY_SOURCE.get(sourceId);
  if (!profile) throw new Error(`[quran-view] unregistered source ${sourceId}`);
  return profile;
}

export function resolveSourceProfile(sourceId: QuranSourceIdValue): QuranSourceProfile {
  return sourceProfile(sourceId);
}
