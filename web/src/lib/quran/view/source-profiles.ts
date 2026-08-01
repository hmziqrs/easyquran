import {
  OPENER_PACKAGING_VALUES,
  OpenerPackaging,
  QuranScript,
  QuranSourceId,
  type QuranScript as QuranScriptValue,
  type QuranSourceId as QuranSourceIdValue,
  type OpenerPackaging as OpenerPackagingValue,
} from "../../data/quran-types.ts";
import { TANZIL_QURAN_DATABASE, type QuranDatabaseAdapter } from "../sql.ts";

export interface QuranSourceArtifact {
  readonly repositoryPath: string;
  readonly r2Path: string;
  readonly sizeBytes: number;
  readonly sha256: string;
}

export interface QuranSourceProfile {
  /** Stable profile identity, qualified by its immutable artifact digest. */
  readonly id: string;
  /** Manifest/cache identity for this concrete source artifact. */
  readonly sourceId: QuranSourceIdValue;
  /** Orthographic/script style exposed to presentation consumers. */
  readonly script: QuranScriptValue;
  readonly artifact: QuranSourceArtifact;
  readonly database: QuranDatabaseAdapter;
  readonly canonicalRowCount: number;
  readonly packagingBySurah: readonly OpenerPackagingValue[];
  readonly expectedPackagingCounts: Readonly<Record<OpenerPackagingValue, number>>;
  /** A numbered verse whose entire raw text is the source's reference opener. */
  readonly referenceOpenerSurah: number;
}

/** Build and validate a registry entry. Index 0 is deliberately unused. */
export function defineSourceProfile(profile: QuranSourceProfile): QuranSourceProfile {
  if (profile.packagingBySurah.length !== 115) {
    throw new Error(`[quran-view:${profile.id}] packaging registry must contain indexes 0..114`);
  }
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

const PROFILES = Object.freeze([
  defineSourceProfile({
    id: "tanzil-uthmani-581cc540",
    sourceId: QuranSourceId.TanzilUthmani,
    script: QuranScript.Uthmani,
    artifact: Object.freeze({
      repositoryPath: "db/quran/tanzil/arabic/quran-uthmani.sqlite",
      r2Path: "tanzil/arabic/quran-uthmani.sqlite",
      sizeBytes: 1_593_344,
      sha256: "581cc5405831bc072fccd8db55cd1db72c5c5440c39bd975edbf03447efecf53",
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
      repositoryPath: "db/quran/tanzil/arabic/quran-simple-clean.sqlite",
      r2Path: "tanzil/arabic/quran-simple-clean.sqlite",
      sizeBytes: 929_792,
      sha256: "a0c52760d6660ac5be1de5c76bb10df7a839a3e8a87ecb0e636fe2ed45b2e4a3",
    }),
    database: TANZIL_QURAN_DATABASE,
    canonicalRowCount: 6236,
    packagingBySurah: TANZIL_PACKAGING,
    expectedPackagingCounts: TANZIL_COUNTS,
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

/** Fail closed when a manifest source id is paired with an unregistered digest. */
export function resolveSourceProfile(
  sourceId: QuranSourceIdValue,
  fileSha256: string,
): QuranSourceProfile {
  const profile = sourceProfile(sourceId);
  if (profile.artifact.sha256 !== fileSha256.toLowerCase()) {
    throw new Error(
      `[quran-view:${sourceId}] unregistered source digest ${fileSha256}; expected ${profile.artifact.sha256}`,
    );
  }
  return profile;
}
