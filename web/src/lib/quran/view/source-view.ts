import type { PrefixCut, SurahNormalization } from "../../data/quran-types.ts";
import {
  OpenerKind,
  OpenerPackaging,
  type OpenerKind as OpenerKindValue,
  type OpenerPackaging as OpenerPackagingValue,
} from "../../data/quran-types.ts";
import type { FirstAyahRow } from "$lib/quran/sql";
import { canonicalOpenerKind } from "./canonical.ts";
import type { QuranSourceProfile } from "./source-profiles.ts";

const ZERO_CUT: PrefixCut = Object.freeze({ openerEndScalar: 0, bodyStartScalar: 0 });

/** Harakat/tanwin, tatweel, superscript alef, and Quranic annotation signs. */
const SKELETON_MARK = /[\u064B-\u065F\u0640\u0670\u06D6-\u06ED]/u;

function isSkeletonMark(value: string): boolean {
  return SKELETON_MARK.test(value);
}

function skeletonScalars(value: string): string[] {
  return Array.from(value).filter((scalar) => !isSkeletonMark(scalar));
}

export function scalarToUtf16Index(value: string, scalarIndex: number): number {
  if (!Number.isInteger(scalarIndex) || scalarIndex < 0) {
    throw new Error(`[quran-view] invalid scalar index ${scalarIndex}`);
  }
  let scalar = 0;
  let utf16 = 0;
  for (const point of value) {
    if (scalar === scalarIndex) return utf16;
    scalar += 1;
    utf16 += point.length;
  }
  if (scalar === scalarIndex) return utf16;
  throw new Error(`[quran-view] scalar index ${scalarIndex} exceeds text length ${scalar}`);
}

export function scalarSlice(value: string, start: number, end?: number): string {
  const startUtf16 = scalarToUtf16Index(value, start);
  const endUtf16 = end === undefined ? value.length : scalarToUtf16Index(value, end);
  return value.slice(startUtf16, endUtf16);
}

/**
 * Detect an embedded source-local opener without rewriting either string.
 * Returns scalar offsets into raw, or null when the skeleton is not its prefix.
 */
export function detectEmbeddedPrefix(raw: string, referenceOpener: string): PrefixCut | null {
  const target = skeletonScalars(referenceOpener);
  const scalars = Array.from(raw);
  let targetIndex = 0;
  let scalarIndex = 0;

  while (scalarIndex < scalars.length && targetIndex < target.length) {
    const scalar = scalars[scalarIndex]!;
    scalarIndex += 1;
    if (isSkeletonMark(scalar)) continue;
    if (scalar !== target[targetIndex]) return null;
    targetIndex += 1;
  }
  if (targetIndex !== target.length) return null;

  while (scalarIndex < scalars.length && isSkeletonMark(scalars[scalarIndex]!)) scalarIndex += 1;
  const openerEndScalar = scalarIndex;
  while (scalarIndex < scalars.length && /\s/u.test(scalars[scalarIndex]!)) scalarIndex += 1;
  const bodyStartScalar = scalarIndex;

  // Embedded sources must contain a separator and a non-empty numbered body.
  if (bodyStartScalar === openerEndScalar || bodyStartScalar >= scalars.length) return null;
  return { openerEndScalar, bodyStartScalar };
}

export interface SourceViewInput {
  profile: QuranSourceProfile;
  firstAyahs: readonly FirstAyahRow[];
  /** Trusted exact opener text for chapter-flag/separate-row packages. */
  openerTexts?: ReadonlyMap<number, string>;
}

export interface QuranSourceView {
  readonly profile: QuranSourceProfile;
  raw(text: string): string;
  body(surah: number, ayah: number, raw: string): string;
  opener(surah: number): { kind: OpenerKindValue; text?: string };
  normalization(surah: number): SurahNormalization;
  normalizations(): SurahNormalization[];
}

function invariant(
  profile: QuranSourceProfile,
  condition: unknown,
  message: string,
): asserts condition {
  if (!condition) throw new Error(`[quran-view:${profile.id}] ${message}`);
}

/** Build and exhaustively validate a source-qualified canonical read view. */
export function createQuranSourceView(input: SourceViewInput): QuranSourceView {
  const { profile } = input;
  const firstBySurah = new Map<number, string>();
  for (const row of input.firstAyahs) {
    invariant(profile, !firstBySurah.has(row.surah), `duplicate first ayah for surah ${row.surah}`);
    firstBySurah.set(row.surah, row.text);
  }
  invariant(profile, firstBySurah.size === 114, `first-ayah count ${firstBySurah.size} != 114`);
  const reference = firstBySurah.get(profile.referenceOpenerSurah);
  invariant(profile, reference, `missing reference opener at ${profile.referenceOpenerSurah}:1`);

  const descriptors: SurahNormalization[] = [];
  for (let surah = 1; surah <= 114; surah++) {
    const raw = firstBySurah.get(surah);
    invariant(profile, raw !== undefined, `missing ${surah}:1`);
    const packaging = profile.packagingBySurah[surah];
    invariant(profile, packaging, `missing packaging for surah ${surah}`);
    const openerKind = canonicalOpenerKind(surah);
    let cut = ZERO_CUT;
    let openerText: string | null = null;

    if (packaging === OpenerPackaging.EmbeddedPrefix) {
      invariant(
        profile,
        openerKind === OpenerKind.Header,
        `embedded prefix at non-header surah ${surah}`,
      );
      const detected = detectEmbeddedPrefix(raw, reference);
      invariant(profile, detected, `expected embedded opener prefix at ${surah}:1`);
      cut = detected;
      openerText = scalarSlice(raw, 0, cut.openerEndScalar);
      const separator = scalarSlice(raw, cut.openerEndScalar, cut.bodyStartScalar);
      const body = scalarSlice(raw, cut.bodyStartScalar);
      invariant(profile, openerText + separator + body === raw, `lossy partition at ${surah}:1`);
    } else if (packaging === OpenerPackaging.NumberedAyah) {
      invariant(
        profile,
        openerKind === OpenerKind.Verse,
        `numbered opener at non-verse surah ${surah}`,
      );
      openerText = raw;
    } else if (
      packaging === OpenerPackaging.ChapterFlag ||
      packaging === OpenerPackaging.SeparateRow
    ) {
      invariant(
        profile,
        openerKind === OpenerKind.Header,
        `${packaging} at non-header surah ${surah}`,
      );
      openerText = input.openerTexts?.get(surah) ?? null;
      invariant(profile, openerText, `missing trusted opener text for ${packaging} surah ${surah}`);
    } else {
      invariant(
        profile,
        packaging === OpenerPackaging.Absent,
        `unsupported packaging ${packaging}`,
      );
      invariant(
        profile,
        openerKind === OpenerKind.None,
        `absent opener at canonical ${openerKind} surah ${surah}`,
      );
    }

    const body = cut.bodyStartScalar > 0 ? scalarSlice(raw, cut.bodyStartScalar) : raw;
    invariant(profile, body.length > 0, `empty body at ${surah}:1`);
    invariant(profile, body.trim() === body, `untrimmed body at ${surah}:1`);
    invariant(
      profile,
      cut.bodyStartScalar > 0 === (packaging === OpenerPackaging.EmbeddedPrefix),
      `cut/package mismatch at ${surah}:1`,
    );
    invariant(
      profile,
      cut.openerEndScalar <= cut.bodyStartScalar,
      `reversed scalar cut at ${surah}:1`,
    );
    invariant(
      profile,
      openerKind === OpenerKind.None ? openerText === null : !!openerText,
      `missing opener at ${surah}`,
    );

    descriptors.push(
      Object.freeze({
        surah,
        sourceId: profile.sourceId,
        script: profile.script,
        sourceProfile: profile.id,
        packaging,
        openerKind,
        openerText,
        openerEndScalar: cut.openerEndScalar,
        bodyStartScalar: cut.bodyStartScalar,
      }),
    );
  }

  const bySurah = new Map(descriptors.map((descriptor, index) => [index + 1, descriptor]));
  const normalization = (surah: number): SurahNormalization => {
    const descriptor = bySurah.get(surah);
    invariant(profile, descriptor, `normalization requested for invalid surah ${surah}`);
    return descriptor;
  };

  return Object.freeze({
    profile,
    raw: (text: string) => text,
    body: (surah: number, ayah: number, raw: string): string =>
      ayah === 1 ? bodyText(raw, ayah, normalization(surah)) : raw,
    opener: (surah: number) => {
      const descriptor = normalization(surah);
      return descriptor.openerText === null
        ? { kind: descriptor.openerKind }
        : { kind: descriptor.openerKind, text: descriptor.openerText };
    },
    normalization,
    normalizations: () => descriptors.slice(),
  });
}

/** Derive a numbered ayah body from a serializable descriptor and raw text. */
export function bodyText(
  raw: string,
  ayah: number,
  normalization: Pick<SurahNormalization, "bodyStartScalar">,
): string {
  if (ayah !== 1 || normalization.bodyStartScalar === 0) return raw;
  return raw.slice(scalarToUtf16Index(raw, normalization.bodyStartScalar));
}

export function packagingCounts(
  normalizations: readonly SurahNormalization[],
): Readonly<Record<OpenerPackagingValue, number>> {
  const counts: Record<OpenerPackagingValue, number> = {
    [OpenerPackaging.NumberedAyah]: 0,
    [OpenerPackaging.EmbeddedPrefix]: 0,
    [OpenerPackaging.ChapterFlag]: 0,
    [OpenerPackaging.SeparateRow]: 0,
    [OpenerPackaging.Absent]: 0,
  };
  for (const descriptor of normalizations) counts[descriptor.packaging] += 1;
  return counts;
}
