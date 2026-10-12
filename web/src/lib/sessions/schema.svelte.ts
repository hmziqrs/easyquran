import type {
  LastReadAnchor,
  RecentsEntry,
  SurahProgress,
} from "#lib/stores/reader-core.svelte.js";
import { decodeReader } from "#lib/stores/reader-persistence.svelte.js";
import { newBookmarkEntityId } from "#lib/bookmarks/schema.js";
import { asArray, asLiteral, asNumber, asObject, asString } from "#lib/storage/index.js";

/**
 * Reading-session local schema (Feature 4). Session state lives under its own
 * per-owner key `easyquran.sessions.{owner}` — never inside the reader blob —
 * so a tab still running a previous build cannot wipe it (EC-54) and one
 * account's sessions are never read for another (§9.5).
 */

export const SESSIONS_SCHEMA_VERSION = 1;
export const SESSION_NAME_MAX = 100;
export const SESSION_KINDS = ["daily", "weekly", "free", "shared"] as const;
export type SessionKind = (typeof SESSION_KINDS)[number];

/** Device-local calendar day as `YYYY-MM-DD` — same format as `todayLocal()` (engagement-state). */
function localDayOf(d: Date): string {
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${d.getFullYear()}-${m}-${day}`;
}

/** Device-local ISO week as `YYYY-Www` (Thursday-based, so boundary weeks belong to the year they mostly cover). */
function localIsoWeekOf(d: Date): string {
  const local = new Date(d.getFullYear(), d.getMonth(), d.getDate());
  const weekday = (local.getDay() + 6) % 7; // Monday = 0
  const thursday = new Date(local.getFullYear(), local.getMonth(), local.getDate() - weekday + 3);
  // UTC-normalized day math: no DST shift can nudge the week count at a boundary.
  const days = Math.floor(
    (Date.UTC(thursday.getFullYear(), thursday.getMonth(), thursday.getDate()) -
      Date.UTC(thursday.getFullYear(), 0, 1)) /
      86_400_000,
  );
  return `${thursday.getFullYear()}-W${String(Math.floor(days / 7) + 1).padStart(2, "0")}`;
}

/**
 * Derived bucket key for timed kinds (Feature 4.8, EC-36): daily → device-local
 * day, weekly → device-local ISO week, else null. Derived per read, never stored.
 */
export function sessionBucketFor(kind: SessionKind, now: Date = new Date()): string | null {
  if (kind === "daily") return localDayOf(now);
  if (kind === "weekly") return localIsoWeekOf(now);
  return null;
}

/**
 * Fixed id of the device-local default session seeded from the legacy reader
 * blob. Never uploaded: on login the merge review re-keys it onto the account
 * default session (§9.4, EC-22).
 */
export const LOCAL_DEFAULT_SESSION_ID = "00000000-0000-4000-8000-000000000000";
export const LOCAL_DEFAULT_SESSION_NAME = "Reading";

/** Fixed UUIDv5 namespace for account default session ids (EC-22). */
const ACCOUNT_DEFAULT_SESSION_NAMESPACE = "a4c2f6b1-3d8e-4f5a-9b7c-1e2d3f4a5b6c";

/** Session rows use RFC3339 strings (server columns are TEXT, §9.1). */
const EPOCH_ISO = "1970-01-01T00:00:00.000Z";

export interface ReadingSession {
  readonly id: string;
  readonly name: string;
  readonly kind: SessionKind;
  readonly translationId: string | null;
  readonly createdAt: string;
  readonly updatedAt: string;
  readonly archivedAt: string | null;
}

/** Per-session reader state. Recents stay device-local (§9.2) and never sync. */
export interface SessionReaderState {
  readonly lastRead: { num: number; n: number; sourceId?: string } | null;
  readonly lastReadAnchor: LastReadAnchor | null;
  readonly recents: RecentsEntry[];
  readonly progress: Record<number, SurahProgress>;
}

export interface SessionsEnvelope {
  readonly v: number;
  readonly activeId: string;
  readonly sessions: readonly ReadingSession[];
  readonly state: Readonly<Record<string, SessionReaderState>>;
}

export function emptySessionState(): SessionReaderState {
  return { lastRead: null, lastReadAnchor: null, recents: [], progress: {} };
}

/** The default session a fresh per-owner envelope starts from. */
export function defaultSessionFor(id: string): ReadingSession {
  return {
    id,
    name: LOCAL_DEFAULT_SESSION_NAME,
    kind: "free",
    translationId: null,
    createdAt: EPOCH_ISO,
    updatedAt: EPOCH_ISO,
    archivedAt: null,
  };
}

/** Pure-JS SHA-1: UUIDv5 namespacing needs it synchronously, and crypto.subtle is async and missing on insecure origins. */
function sha1(input: Uint8Array): Uint8Array {
  const h = new Uint32Array([0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476, 0xc3d2e1f0]);
  const padded = new Uint8Array((((input.length + 8) >>> 6) + 1) << 6);
  padded.set(input);
  padded[input.length] = 0x80;
  const dv = new DataView(padded.buffer);
  dv.setUint32(padded.length - 8, Math.floor(input.length / 0x20000000));
  dv.setUint32(padded.length - 4, input.length << 3);
  const w = new Uint32Array(80);
  for (let off = 0; off < padded.length; off += 64) {
    for (let i = 0; i < 16; i += 1) w[i] = dv.getUint32(off + i * 4);
    for (let i = 16; i < 80; i += 1) {
      const v = (w[i - 3] ?? 0) ^ (w[i - 8] ?? 0) ^ (w[i - 14] ?? 0) ^ (w[i - 16] ?? 0);
      w[i] = (v << 1) | (v >>> 31);
    }
    let a = h[0] ?? 0;
    let b = h[1] ?? 0;
    let c = h[2] ?? 0;
    let d = h[3] ?? 0;
    let e = h[4] ?? 0;
    for (let i = 0; i < 80; i += 1) {
      let f = 0;
      let k = 0;
      if (i < 20) {
        f = (b & c) | (~b & d);
        k = 0x5a827999;
      } else if (i < 40) {
        f = b ^ c ^ d;
        k = 0x6ed9eba1;
      } else if (i < 60) {
        f = (b & c) | (b & d) | (c & d);
        k = 0x8f1bbcdc;
      } else {
        f = b ^ c ^ d;
        k = 0xca62c1d6;
      }
      const t = (((a << 5) | (a >>> 27)) + f + e + k + (w[i] ?? 0)) >>> 0;
      e = d;
      d = c;
      c = (b << 30) | (b >>> 2);
      b = a;
      a = t;
    }
    h[0] = ((h[0] ?? 0) + a) >>> 0;
    h[1] = ((h[1] ?? 0) + b) >>> 0;
    h[2] = ((h[2] ?? 0) + c) >>> 0;
    h[3] = ((h[3] ?? 0) + d) >>> 0;
    h[4] = ((h[4] ?? 0) + e) >>> 0;
  }
  const out = new Uint8Array(20);
  const odv = new DataView(out.buffer);
  for (let i = 0; i < 5; i += 1) odv.setUint32(i * 4, h[i] ?? 0);
  return out;
}

function uuidV5(namespace: string, name: string): string {
  const ns = Uint8Array.from({ length: 16 }, (_, i) =>
    Number.parseInt(namespace.slice(i * 2, i * 2 + 2), 16),
  );
  const nameBytes = new TextEncoder().encode(name);
  const input = new Uint8Array(ns.length + nameBytes.length);
  input.set(ns);
  input.set(nameBytes, ns.length);
  const bytes = sha1(input).slice(0, 16);
  bytes[6] = ((bytes[6] ?? 0) & 0x0f) | 0x50;
  bytes[8] = ((bytes[8] ?? 0) & 0x3f) | 0x80;
  const hex = Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}

/**
 * Deterministic account default session id: every device of the account derives
 * the same id from the user id, so migrated default sessions converge instead
 * of duplicating "Reading" (EC-22).
 */
export function accountDefaultSessionId(userId: number): string {
  return uuidV5(ACCOUNT_DEFAULT_SESSION_NAMESPACE, String(userId));
}

/** Non-empty string, else null (absent/null/empty all mean "none"). */
// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is one unparsed field from the per-owner key; this helper is its boundary parser.
function stringOrNull(raw: unknown): string | null {
  const value = asString(raw);
  return value !== undefined && value.length > 0 ? value : null;
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is one unparsed session row; this decoder is its boundary parser.
export function decodeReadingSession(raw: unknown): ReadingSession | null {
  const obj = asObject(raw);
  if (!obj) return null;
  const id = asString(obj.id);
  const name = asString(obj.name);
  const kind = asLiteral<SessionKind>(obj.kind, SESSION_KINDS);
  const createdAt = asString(obj.createdAt);
  const updatedAt = asString(obj.updatedAt);
  if (
    id === undefined ||
    id.length === 0 ||
    kind === undefined ||
    createdAt === undefined ||
    updatedAt === undefined ||
    name === undefined ||
    name.length === 0 ||
    name.length > SESSION_NAME_MAX
  ) {
    return null;
  }
  return {
    id,
    name,
    kind,
    translationId: stringOrNull(obj.translationId),
    createdAt,
    updatedAt,
    archivedAt: stringOrNull(obj.archivedAt),
  };
}

/**
 * Session state decodes through the reader-blob field decoder: the shape is the
 * v4 blob's `lastRead`/`lastReadAnchor`/`recents`/`progress` exactly, so the
 * migration and every later decode share one validation path.
 */
// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is one unparsed state record; this decoder is its boundary parser.
export function decodeSessionState(raw: unknown): SessionReaderState {
  const decoded = decodeReader(raw);
  return {
    lastRead: decoded.lastRead ?? null,
    lastReadAnchor: decoded.lastReadAnchor ?? null,
    recents: decoded.recents ?? [],
    progress: decoded.progress ?? {},
  };
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the unparsed localStorage value of easyquran.sessions.{owner}; this decoder is its boundary parser.
export function decodeSessionsEnvelope(raw: unknown): SessionsEnvelope | undefined {
  const stored = asObject(raw);
  if (!stored) return undefined;
  if (stored.v !== SESSIONS_SCHEMA_VERSION) return undefined;
  const activeId = asString(stored.activeId);
  if (activeId === undefined || activeId.length === 0) return undefined;
  const sessionsRaw = Array.isArray(stored.sessions) ? stored.sessions : [];
  const sessions = sessionsRaw
    .map((item) => decodeReadingSession(item))
    .filter((session): session is ReadingSession => session !== null);
  const state: Record<string, SessionReaderState> = {};
  const stateObj = asObject(stored.state);
  if (stateObj) {
    for (const [sessionId, value] of Object.entries(stateObj)) {
      state[sessionId] = decodeSessionState(value);
    }
  }
  return { v: SESSIONS_SCHEMA_VERSION, activeId, sessions, state };
}

/** Client-minted session ids share the bookmarks entity-id minter (36-char dashed UUID, §9.1). */
export function newSessionId(): string {
  return newBookmarkEntityId();
}

// --- sync wire (§9.2: POST /reading-session/v1/sync) ---

export type SessionMutation =
  | {
      kind: "session.upsert";
      id: string;
      name: string;
      sessionKind: SessionKind;
      translationId: string | null;
      archivedAt: string | null;
      updatedAt: string;
    }
  | { kind: "session.delete"; id: string; updatedAt: string }
  | {
      kind: "position.upsert";
      sessionId: string;
      surah: number;
      ayah: number;
      localPage: number | null;
      ratio: number | null;
      sourceId: string | null;
      updatedAt: string;
    }
  | {
      kind: "progress.upsert";
      sessionId: string;
      surah: number;
      furthestAyah: number;
      updatedAt: string;
    };

export interface SessionPositionWire {
  readonly sessionId: string;
  readonly surah: number;
  readonly ayah: number;
  readonly localPage: number | null;
  readonly ratio: number | null;
  readonly sourceId: string | null;
  readonly updatedAt: string;
}

export interface SessionProgressWire {
  readonly sessionId: string;
  readonly surah: number;
  readonly furthestAyah: number;
  readonly updatedAt: string;
}

export interface SessionSnapshot {
  readonly applied: number;
  readonly skipped: number;
  readonly sessions: readonly ReadingSession[];
  readonly positions: readonly SessionPositionWire[];
  readonly progress: readonly SessionProgressWire[];
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- value is one raw timestamp field from the sync snapshot; this helper is its boundary parser.
function tsOf(value: unknown): number {
  const parsed = asString(value);
  if (parsed === undefined) return 0;
  const ms = Date.parse(parsed);
  return Number.isFinite(ms) ? ms : 0;
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is one unparsed position row from the sync snapshot; this decoder is its boundary parser.
function decodePosition(raw: unknown): SessionPositionWire | undefined {
  const obj = asObject(raw);
  if (!obj) return undefined;
  const sessionId = asString(obj.sessionId);
  const surah = asNumber(obj.surah, 1, 114);
  const ayah = asNumber(obj.ayah, 1, 286);
  const updatedAt = asString(obj.updatedAt);
  if (
    sessionId === undefined ||
    surah === undefined ||
    ayah === undefined ||
    updatedAt === undefined
  ) {
    return undefined;
  }
  const localPage = asNumber(obj.localPage, 1, 604);
  const ratio = asNumber(obj.ratio, 0, 1);
  return {
    sessionId,
    surah,
    ayah,
    localPage: localPage ?? null,
    ratio: ratio ?? null,
    sourceId: stringOrNull(obj.sourceId),
    updatedAt,
  };
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is one unparsed progress row from the sync snapshot; this decoder is its boundary parser.
function decodeProgress(raw: unknown): SessionProgressWire | undefined {
  const obj = asObject(raw);
  if (!obj) return undefined;
  const sessionId = asString(obj.sessionId);
  const surah = asNumber(obj.surah, 1, 114);
  const furthestAyah = asNumber(obj.furthestAyah, 1, 286);
  const updatedAt = asString(obj.updatedAt);
  if (
    sessionId === undefined ||
    surah === undefined ||
    furthestAyah === undefined ||
    updatedAt === undefined
  ) {
    return undefined;
  }
  return { sessionId, surah, furthestAyah, updatedAt };
}

/**
 * Decode a sync response snapshot `{applied, skipped, sessions, positions,
 * progress}`. Malformed rows drop individually (existing decoder behavior);
 * an unreadable envelope is a transport error (null).
 */
// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the unparsed JSON body of the reading-session sync response; this decoder is its boundary parser.
export function decodeSessionSnapshot(raw: unknown): SessionSnapshot | null {
  const obj = asObject(raw);
  if (!obj) return null;
  const sessions = asArray(obj.sessions, (item) => decodeReadingSession(item) ?? undefined);
  const positions = asArray(obj.positions, decodePosition);
  const progress = asArray(obj.progress, decodeProgress);
  return {
    applied: asNumber(obj.applied, 0, 200) ?? 0,
    skipped: asNumber(obj.skipped, 0, 200) ?? 0,
    sessions,
    positions,
    progress,
  };
}

/** Server timestamps are RFC3339 strings; the client mirror keeps epoch-ms. */
export function progressTs(updatedAt: string): number {
  return tsOf(updatedAt);
}
