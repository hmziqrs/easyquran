import { asArray, asLiteral, asNumber, asObject, asString } from "#lib/storage/index.js";

/**
 * Reading threads (docs/reading-model.md §2). A thread is one continuous run
 * of reading: entering from somewhere else starts (or resumes) one, continuing
 * keeps it. The cursor lives on the thread row; progress is per owner.
 * Wire JSON and the per-owner key `easyquran.sessions.{owner}` both decode
 * defensively here — nothing past this module trusts raw storage or responses.
 */

export const SESSIONS_SCHEMA_VERSION = 2;
/** Open threads go stale after this long without a touch (derived, never stored). */
export const STALE_AFTER_MS = 30 * 24 * 60 * 60 * 1000;
/** Local cap: newest threads kept on device (open first); the server keeps the rest. */
export const LOCAL_THREAD_CAP = 60;

export const SESSION_ORIGINS = ["index", "search", "bookmark", "card", "snippet", "link"] as const;
export type SessionOrigin = (typeof SESSION_ORIGINS)[number];
export const END_REASONS = ["completed", "restarted", "dismissed"] as const;
export type EndReason = (typeof END_REASONS)[number];
export const DEVICES = ["phone", "tablet", "desktop"] as const;
export type DeviceKind = (typeof DEVICES)[number];

export interface ReadingThread {
  readonly id: string;
  readonly origin: SessionOrigin;
  /** Origin detail — e.g. the home card that opened the thread. */
  readonly originRef: string | null;
  /** `surah:N` | `page:N` | `juz:N` | `hizb:N` | `snippet:slug` | `range:S:A-S:A`. */
  readonly scope: string;
  /** `S:A` end of a bounded scope; reading it completes the thread. */
  readonly boundEnd: string | null;
  readonly surah: number;
  readonly ayah: number;
  readonly localPage: number | null;
  readonly ratio: number | null;
  readonly sourceId: string | null;
  readonly device: DeviceKind;
  readonly startedAt: string;
  readonly updatedAt: string;
  readonly endedAt: string | null;
  readonly endReason: EndReason | null;
}

export interface SurahProgress {
  readonly furthestAyah: number;
  readonly updatedAt: string;
}

/** Furthest ayah reached per surah (account-level, MAX-merged). */
export type ProgressBySurah = Readonly<Record<number, SurahProgress>>;

export interface SessionsEnvelope {
  readonly v: number;
  readonly threads: readonly ReadingThread[];
  readonly progress: ProgressBySurah;
}

export type SessionMutation =
  | ({ readonly kind: "session.upsert" } & ReadingThread)
  | { readonly kind: "session.delete"; readonly id: string; readonly updatedAt: string }
  | {
      readonly kind: "progress.upsert";
      readonly surah: number;
      readonly furthestAyah: number;
      readonly updatedAt: string;
    };

export interface SessionSnapshot {
  readonly applied: number;
  readonly skipped: number;
  readonly sessions: readonly ReadingThread[];
  readonly progress: readonly ({ readonly surah: number } & SurahProgress)[];
}

export interface ArchivePage {
  readonly sessions: readonly ReadingThread[];
  readonly next: string | null;
}

export function emptyEnvelope(): SessionsEnvelope {
  return { v: SESSIONS_SCHEMA_VERSION, threads: [], progress: {} };
}

/** Open = not ended and touched within STALE_AFTER_MS. */
export function isOpen(thread: ReadingThread, nowMs: number = Date.now()): boolean {
  if (thread.endedAt !== null) return false;
  const touched = Date.parse(thread.updatedAt);
  return Number.isFinite(touched) && nowMs - touched < STALE_AFTER_MS;
}

/** Newest first, ties by id — the server's order, so every device converges. */
export function byNewest(a: ReadingThread, b: ReadingThread): number {
  if (a.updatedAt !== b.updatedAt) return a.updatedAt > b.updatedAt ? -1 : 1;
  if (a.id === b.id) return 0;
  return a.id > b.id ? -1 : 1;
}

/** `S:A` → coordinates, or null for anything else. */
export function parseVerseKey(key: string): { surah: number; ayah: number } | null {
  const match = /^(\d{1,3}):(\d{1,3})$/u.exec(key);
  if (match === null) return null;
  const surah = Number(match[1]);
  const ayah = Number(match[2]);
  if (surah < 1 || surah > 114 || ayah < 1 || ayah > 286) return null;
  return { surah, ayah };
}

/** Lexicographic verse order: negative when a precedes b. */
export function compareVerse(
  a: { surah: number; ayah: number },
  b: { surah: number; ayah: number },
): number {
  return a.surah !== b.surah ? a.surah - b.surah : a.ayah - b.ayah;
}

const SCOPE_RE =
  /^(?:(?:surah|page|juz|hizb):\d{1,3}|snippet:[A-Za-z0-9._-]{1,64}|range:\d{1,3}:\d{1,3}-\d{1,3}:\d{1,3})$/u;

export function isScope(raw: string): boolean {
  return SCOPE_RE.test(raw);
}

/** Non-empty string, else null (absent/null/empty all mean "none"). */
// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is one unparsed field; this helper is its boundary parser.
function stringOrNull(raw: unknown): string | null {
  const value = asString(raw);
  return value !== undefined && value.length > 0 ? value : null;
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is one unparsed nullable number; this helper is its boundary parser.
function numberOrNull(raw: unknown, min: number, max: number): number | null {
  return asNumber(raw, min, max) ?? null;
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is one unparsed thread row (storage or wire); this decoder is its boundary parser.
export function decodeThread(raw: unknown): ReadingThread | null {
  const obj = asObject(raw);
  if (!obj) return null;
  const id = asString(obj.id);
  const origin = asLiteral<SessionOrigin>(obj.origin, SESSION_ORIGINS);
  const scope = asString(obj.scope);
  const surah = asNumber(obj.surah, 1, 114);
  const ayah = asNumber(obj.ayah, 1, 286);
  const device = asLiteral<DeviceKind>(obj.device, DEVICES);
  const startedAt = asString(obj.startedAt);
  const updatedAt = asString(obj.updatedAt);
  if (
    id === undefined ||
    id.length !== 36 ||
    origin === undefined ||
    scope === undefined ||
    !isScope(scope) ||
    surah === undefined ||
    ayah === undefined ||
    device === undefined ||
    startedAt === undefined ||
    updatedAt === undefined
  ) {
    return null;
  }
  const boundEnd = stringOrNull(obj.boundEnd);
  const endedAt = stringOrNull(obj.endedAt);
  const endReason = asLiteral<EndReason>(obj.endReason, END_REASONS) ?? null;
  return {
    id,
    origin,
    originRef: stringOrNull(obj.originRef),
    scope,
    boundEnd: boundEnd !== null && parseVerseKey(boundEnd) !== null ? boundEnd : null,
    surah,
    ayah,
    localPage: numberOrNull(obj.localPage, 1, 10_000),
    ratio: numberOrNull(obj.ratio, 0, 1),
    sourceId: stringOrNull(obj.sourceId),
    device,
    startedAt,
    updatedAt,
    endedAt: endedAt !== null && endReason !== null ? endedAt : null,
    endReason: endedAt !== null ? endReason : null,
  };
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is one unparsed progress row; this decoder is its boundary parser.
function decodeProgressRow(raw: unknown): ({ surah: number } & SurahProgress) | undefined {
  const obj = asObject(raw);
  if (!obj) return undefined;
  const surah = asNumber(obj.surah, 1, 114);
  const furthestAyah = asNumber(obj.furthestAyah, 1, 286);
  const updatedAt = asString(obj.updatedAt);
  if (surah === undefined || furthestAyah === undefined || updatedAt === undefined)
    return undefined;
  return { surah, furthestAyah, updatedAt };
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the unparsed per-owner localStorage value; this decoder is its boundary parser.
export function decodeEnvelope(raw: unknown): SessionsEnvelope | undefined {
  const obj = asObject(raw);
  if (!obj || obj.v !== SESSIONS_SCHEMA_VERSION) return undefined;
  const threads = asArray(obj.threads, (row) => decodeThread(row) ?? undefined);
  const progress: Record<number, SurahProgress> = {};
  const progressObj = asObject(obj.progress) ?? {};
  for (const [key, value] of Object.entries(progressObj)) {
    const row = decodeProgressRow({ ...asObject(value), surah: Number(key) });
    if (row !== undefined)
      progress[row.surah] = { furthestAyah: row.furthestAyah, updatedAt: row.updatedAt };
  }
  return { v: SESSIONS_SCHEMA_VERSION, threads: [...threads].sort(byNewest), progress };
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the unparsed POST /reading-session/v1/sync body; this decoder is its boundary parser.
export function decodeSessionSnapshot(raw: unknown): SessionSnapshot | null {
  const obj = asObject(raw);
  if (!obj || !Array.isArray(obj.sessions) || !Array.isArray(obj.progress)) return null;
  return {
    applied: asNumber(obj.applied, 0, Number.MAX_SAFE_INTEGER) ?? 0,
    skipped: asNumber(obj.skipped, 0, Number.MAX_SAFE_INTEGER) ?? 0,
    sessions: asArray(obj.sessions, (row) => decodeThread(row) ?? undefined),
    progress: asArray(obj.progress, decodeProgressRow),
  };
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the unparsed GET /reading-session/v1/archive body; this decoder is its boundary parser.
export function decodeArchivePage(raw: unknown): ArchivePage | null {
  const obj = asObject(raw);
  if (!obj || !Array.isArray(obj.sessions)) return null;
  return {
    sessions: asArray(obj.sessions, (row) => decodeThread(row) ?? undefined),
    next: stringOrNull(obj.next),
  };
}
