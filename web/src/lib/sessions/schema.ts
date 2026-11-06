import type { JsonRecord, JsonValue } from "#lib/storage/decoders.js";
import { asArray, asLiteral, asNumber, asObject, asString } from "#lib/storage/index.js";
import { parseStamp } from "#lib/sync/merge.js";

/**
 * Reading threads. A thread is one continuous run
 * of reading: entering from somewhere else starts (or resumes) one, continuing
 * keeps it. The cursor lives on the thread row; progress is per owner.
 * Wire JSON and the per-owner key `easyquran.sessions.{owner}` both decode
 * defensively here — nothing past this module trusts raw storage or responses.
 */

export const SESSIONS_SCHEMA_VERSION = 3;
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

export const THREAD_STATES = ["open", "ended", "deleted"] as const;
export type ThreadState = (typeof THREAD_STATES)[number];

/** Registers of a row. Null until the row first belongs to an account. */
export interface ThreadStamps {
  readonly identityStamp: string | null;
  readonly cursorStamp: string | null;
  readonly lifecycleStamp: string | null;
}

export interface ReadingThread extends ThreadStamps {
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
  readonly state: ThreadState;
  readonly deviceId: string | null;
  /**
   * The server's name for `deviceId` (devices.md D5): null for an unknown, removed or
   * unnamed device. Set only by a pull; never sent back to the server.
   */
  readonly deviceName: string | null;
  /** The row is the queue: true until a result acknowledges these exact stamps. */
  readonly dirty: boolean;
  /** Refused by the server. Kept locally, not resent until the next edit. */
  readonly rejected: boolean;
  /** Fields this build does not know, kept for whoever does. */
  readonly extras: Readonly<Record<string, JsonValue>>;
}

export interface SurahProgress {
  readonly furthestAyah: number;
  readonly updatedAt: string;
  readonly dirty?: boolean;
}

/** Furthest ayah reached per surah (account-level, MAX-merged). */
export type ProgressBySurah = Readonly<Record<number, SurahProgress>>;

export interface SessionsEnvelope {
  readonly v: number;
  readonly threads: readonly ReadingThread[];
  readonly progress: ProgressBySurah;
  /** Last pulled `rev`; 0 means a full pull is next. */
  readonly since: number;
  /** Page cursor inside a full pull. */
  readonly after: number;
  readonly epoch: string | null;
  readonly lastSeenAt: number;
}

export type SessionWireRow =
  | ({ readonly kind: "session.upsert"; readonly id: string } & Partial<ReadingThread>)
  | {
      readonly kind: "progress.upsert";
      readonly surah: number;
      readonly furthestAyah: number;
      readonly updatedAt: string;
    };

export type RowStatus = "applied" | "superseded" | "rejected" | "capped";

export interface RowResult {
  readonly id: string;
  readonly status: RowStatus;
  readonly stamps: Partial<Record<"identity" | "cursor" | "lifecycle", string | null>> | null;
}

export interface ThreadTombstone extends ThreadStamps {
  readonly id: string;
}

export interface SessionRound {
  readonly serverTime: number;
  readonly syncEpoch: string | null;
  readonly results: readonly RowResult[];
  readonly sessions: readonly ReadingThread[];
  readonly tombstones: readonly ThreadTombstone[];
  readonly progress: readonly ({ readonly surah: number } & SurahProgress)[];
  readonly rev: number;
  readonly latestRev: number;
  readonly more: boolean;
  readonly tombstoneFloor: number;
}

export interface ArchivePage {
  readonly sessions: readonly ReadingThread[];
  readonly next: string | null;
}

export function emptyEnvelope(): SessionsEnvelope {
  return {
    v: SESSIONS_SCHEMA_VERSION,
    threads: [],
    progress: {},
    since: 0,
    after: -1,
    epoch: null,
    lastSeenAt: 0,
  };
}

/** Open = not ended and touched within STALE_AFTER_MS. */
export function isOpen(thread: ReadingThread, nowMs: number = Date.now()): boolean {
  if (thread.state !== "open" || thread.endedAt !== null) return false;
  const touched = touchedAt(thread);
  return Number.isFinite(touched) && nowMs - touched < STALE_AFTER_MS;
}

/** When the cursor last moved. Signed-out rows have no stamp yet. */
export function touchedAt(thread: ReadingThread): number {
  const stamp = thread.cursorStamp === null ? null : parseStamp(thread.cursorStamp);
  return stamp === null ? Date.parse(thread.updatedAt) : stamp.ms;
}

/** Newest first, ties by id — the server's order, so every device converges. */
export function byNewest(a: ReadingThread, b: ReadingThread): number {
  const [left, right] = [touchedAt(a), touchedAt(b)];
  if (left !== right) return left > right ? -1 : 1;
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

const KNOWN_THREAD_FIELDS = new Set([
  "id",
  "kind",
  "origin",
  "originRef",
  "scope",
  "boundEnd",
  "surah",
  "ayah",
  "localPage",
  "ratio",
  "sourceId",
  "device",
  "deviceId",
  "deviceName",
  "startedAt",
  "updatedAt",
  "endedAt",
  "endReason",
  "state",
  "identityStamp",
  "cursorStamp",
  "lifecycleStamp",
  "dirty",
  "rejected",
  "rev",
  "extras",
]);

function extrasOf(obj: JsonRecord) {
  const stored = asObject(obj.extras) ?? {};
  const unknown = Object.entries(obj).filter(([key]) => !KNOWN_THREAD_FIELDS.has(key));
  return { ...stored, ...Object.fromEntries(unknown) };
}

function stateOf(obj: JsonRecord): ThreadState {
  const state = asLiteral<ThreadState>(obj.state, THREAD_STATES);
  if (state !== undefined) return state;
  return stringOrNull(obj.endedAt) === null ? "open" : "ended";
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is one unparsed thread row (storage or wire); this decoder is its boundary parser.
export function decodeThread(raw: unknown): ReadingThread | null {
  const obj = asObject(raw);
  if (!obj) return null;
  const id = asString(obj.id);
  const scope = asString(obj.scope);
  const surah = asNumber(obj.surah, 1, 114);
  const ayah = asNumber(obj.ayah, 1, 286);
  const startedAt = asString(obj.startedAt);
  const updatedAt = asString(obj.updatedAt);
  if (
    id === undefined ||
    id.length !== 36 ||
    scope === undefined ||
    surah === undefined ||
    ayah === undefined ||
    startedAt === undefined ||
    updatedAt === undefined
  ) {
    return null;
  }
  const boundEnd = stringOrNull(obj.boundEnd);
  const endedAt = stringOrNull(obj.endedAt);
  // Newer clients may send values this build has never seen: show a neutral
  // default, and a scope it cannot read falls back at display time. The stored
  // register on the server is never touched by reading a row.
  const endReason = asLiteral<EndReason>(obj.endReason, END_REASONS) ?? "dismissed";
  return {
    id,
    origin: asLiteral<SessionOrigin>(obj.origin, SESSION_ORIGINS) ?? "link",
    originRef: stringOrNull(obj.originRef),
    scope,
    boundEnd: boundEnd !== null && parseVerseKey(boundEnd) !== null ? boundEnd : null,
    surah,
    ayah,
    localPage: numberOrNull(obj.localPage, 1, 10_000),
    ratio: numberOrNull(obj.ratio, 0, 1),
    sourceId: stringOrNull(obj.sourceId),
    device: asLiteral<DeviceKind>(obj.device, DEVICES) ?? "desktop",
    startedAt,
    updatedAt,
    endedAt,
    endReason: endedAt === null ? null : endReason,
    state: stateOf(obj),
    deviceId: stringOrNull(obj.deviceId),
    deviceName: stringOrNull(obj.deviceName),
    identityStamp: stringOrNull(obj.identityStamp),
    cursorStamp: stringOrNull(obj.cursorStamp),
    lifecycleStamp: stringOrNull(obj.lifecycleStamp),
    dirty: obj.dirty === true,
    rejected: obj.rejected === true,
    extras: extrasOf(obj),
  };
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is one unparsed tombstone row from a pull; this decoder is its boundary parser.
function decodeTombstone(raw: unknown): ThreadTombstone | undefined {
  const obj = asObject(raw);
  const id = asString(obj?.id);
  if (obj === undefined || id === undefined || obj.state !== "deleted") return undefined;
  return {
    id,
    identityStamp: stringOrNull(obj.identityStamp),
    cursorStamp: stringOrNull(obj.cursorStamp),
    lifecycleStamp: stringOrNull(obj.lifecycleStamp),
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
  return obj.dirty === true
    ? { surah, furthestAyah, updatedAt, dirty: true }
    : { surah, furthestAyah, updatedAt };
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the unparsed per-owner localStorage value; this decoder is its boundary parser.
export function decodeEnvelope(raw: unknown): SessionsEnvelope | undefined {
  const obj = asObject(raw);
  // Older versions migrate forward through the tolerant row decoder. A newer
  // store is left alone; see `isNewerStore`.
  if (!obj || isNewerStore(raw)) return undefined;
  const threads = asArray(obj.threads, (row) => decodeThread(row) ?? undefined);
  const progress: Record<number, SurahProgress> = {};
  const progressObj = asObject(obj.progress) ?? {};
  for (const [key, value] of Object.entries(progressObj)) {
    const row = decodeProgressRow({ ...asObject(value), surah: Number(key) });
    if (row !== undefined) {
      const { surah, ...rest } = row;
      progress[surah] = rest;
    }
  }
  return {
    v: SESSIONS_SCHEMA_VERSION,
    threads: [...threads].sort(byNewest),
    progress,
    since: asNumber(obj.since, 0, Number.MAX_SAFE_INTEGER) ?? 0,
    after: asNumber(obj.after, -1, Number.MAX_SAFE_INTEGER) ?? -1,
    epoch: stringOrNull(obj.epoch),
    lastSeenAt: asNumber(obj.lastSeenAt, 0, Number.MAX_SAFE_INTEGER) ?? 0,
  };
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the unparsed per-owner localStorage value; this check reads only its version.
export function isNewerStore(raw: unknown): boolean {
  const version = asNumber(asObject(raw)?.v, 0, Number.MAX_SAFE_INTEGER);
  return version !== undefined && version > SESSIONS_SCHEMA_VERSION;
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is one unparsed per-row result; this decoder is its boundary parser.
function decodeResult(raw: unknown): RowResult | undefined {
  const obj = asObject(raw);
  const id = asString(obj?.id);
  const status = asLiteral<RowStatus>(obj?.status, ["applied", "superseded", "rejected", "capped"]);
  if (obj === undefined || id === undefined || status === undefined) return undefined;
  const stamps = asObject(obj.stamps);
  return {
    id,
    status,
    stamps:
      stamps === undefined
        ? null
        : {
            identity: stringOrNull(stamps.identity),
            cursor: stringOrNull(stamps.cursor),
            lifecycle: stringOrNull(stamps.lifecycle),
          },
  };
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the unparsed POST /reading-session/v1/sync body; this decoder is its boundary parser.
export function decodeSessionRound(raw: unknown): SessionRound | null {
  const obj = asObject(raw);
  if (!obj || !Array.isArray(obj.sessions) || !Array.isArray(obj.progress)) return null;
  const bound = Number.MAX_SAFE_INTEGER;
  const rows = obj.sessions;
  return {
    serverTime: asNumber(obj.serverTime, 0, bound) ?? Date.now(),
    syncEpoch: stringOrNull(obj.syncEpoch),
    results: asArray(obj.results, decodeResult),
    sessions: asArray(rows, (row) => decodeThread(row) ?? undefined),
    tombstones: asArray(rows, decodeTombstone),
    progress: asArray(obj.progress, decodeProgressRow),
    rev: asNumber(obj.rev, 0, bound) ?? 0,
    latestRev: asNumber(obj.latestRev, 0, bound) ?? 0,
    more: obj.more === true,
    tombstoneFloor: asNumber(obj.tombstoneFloor, 0, bound) ?? 0,
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
