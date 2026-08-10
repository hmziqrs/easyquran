import { browser } from "$app/environment";
import { isArabicSourceId, type QuranReaderSource } from "$lib/data/quran-types";
import { asNumberRecord } from "$lib/storage/decoders";
import {
  asNumber,
  asObject,
  asString,
  isFutureSchema,
  readJSON,
  readRaw,
  removeRaw,
  writeJSON,
  writeRaw,
} from "$lib/storage";

const KEY = "eq:engagement";
const SESSION_KEY = "eq:reader-session-views";
const LEGACY_KEY = "eq:reader-views";
const READER_KEY = "easyquran.reader";
const SOURCE_KEY = "easyquran.reader.source";
const SCHEMA_VERSION = 1;

export interface EngagementState {
  v: 1;
  totalViews: number;
  distinctDays: number;
  lastDay: string;
  firstSeen: number;
  lastSeen: number;
  qualified: boolean;
  legacySeeded: boolean;
  sourceViews: Record<string, number>;
}

export interface ViewBump {
  preBumpSourceViews: number;
  engaged: boolean;
  sessionViews: number;
}

function todayLocal(): string {
  const d = new Date();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${d.getFullYear()}-${m}-${day}`;
}

function blankState(): EngagementState {
  const now = Date.now();
  return {
    v: SCHEMA_VERSION,
    totalViews: 0,
    distinctDays: 0,
    lastDay: "",
    firstSeen: now,
    lastSeen: now,
    qualified: false,
    legacySeeded: false,
    sourceViews: {},
  };
}

export function decodeEngagement(raw: unknown): EngagementState | undefined {
  if (isFutureSchema(raw, SCHEMA_VERSION)) return undefined;
  const obj = asObject(raw);
  if (!obj) return undefined;
  if (obj.v !== SCHEMA_VERSION) return undefined;
  const totalViews = asNumber(obj.totalViews, 0, Number.MAX_SAFE_INTEGER);
  const distinctDays = asNumber(obj.distinctDays, 0, Number.MAX_SAFE_INTEGER);
  const lastDay = typeof obj.lastDay === "string" ? obj.lastDay : undefined;
  const firstSeen = asNumber(obj.firstSeen, 0, Number.MAX_SAFE_INTEGER);
  const lastSeen = asNumber(obj.lastSeen, 0, Number.MAX_SAFE_INTEGER);
  if (
    totalViews === undefined ||
    distinctDays === undefined ||
    lastDay === undefined ||
    firstSeen === undefined ||
    lastSeen === undefined
  ) {
    return undefined;
  }
  return {
    v: SCHEMA_VERSION,
    totalViews,
    distinctDays,
    lastDay,
    firstSeen,
    lastSeen,
    qualified: obj.qualified === true,
    legacySeeded: obj.legacySeeded === true,
    sourceViews: asNumberRecord(obj.sourceViews, 0, Number.MAX_SAFE_INTEGER),
  };
}

function isEngaged(s: EngagementState): boolean {
  return s.qualified || s.distinctDays >= 2 || s.totalViews >= 4;
}

export function isEngagedReader(): boolean {
  if (!browser) return false;
  const s = decodeEngagement(readJSON(KEY));
  return s ? isEngaged(s) : false;
}

export function readerSessionViews(): number {
  if (!browser) return 0;
  return Number(readRaw("session", SESSION_KEY) ?? 0);
}

function seedFromReaderEvidence(s: EngagementState): void {
  const readerRaw = readJSON(READER_KEY);
  if (!isFutureSchema(readerRaw, 1)) {
    const r = asObject(readerRaw);
    if (r) {
      const bm = asObject(r.bookmarks);
      const notes = asObject(r.notes);
      if (
        (bm !== undefined && Object.keys(bm).length > 0) ||
        (notes !== undefined && Object.keys(notes).length > 0)
      ) {
        s.qualified = true;
      }
    }
  }
  const srcRaw = readJSON(SOURCE_KEY);
  if (!isFutureSchema(srcRaw, 1)) {
    const src = asObject(srcRaw);
    const sid = src ? asString(src.sourceId) : undefined;
    if (sid && !isArabicSourceId(sid)) {
      s.sourceViews[sid] = Math.max(s.sourceViews[sid] ?? 0, 1);
    }
  }
}

export function bumpReaderView(sourceId: QuranReaderSource | null | undefined): ViewBump {
  if (!browser) return { preBumpSourceViews: 0, engaged: false, sessionViews: 0 };

  const durable = decodeEngagement(readJSON(KEY)) ?? blankState();
  const translationId =
    typeof sourceId === "string" && !isArabicSourceId(sourceId) ? sourceId : null;
  const preBumpSourceViews = translationId ? (durable.sourceViews[translationId] ?? 0) : 0;

  const legacyVal = Number(readRaw("session", LEGACY_KEY) ?? 0);
  let sessionViews = Number(readRaw("session", SESSION_KEY) ?? 0);

  if (!durable.legacySeeded) {
    if (legacyVal > 0) {
      durable.totalViews += legacyVal;
      if (sessionViews < legacyVal) sessionViews = legacyVal;
    }
    seedFromReaderEvidence(durable);
  }

  durable.totalViews += 1;
  if (translationId) {
    durable.sourceViews[translationId] = (durable.sourceViews[translationId] ?? 0) + 1;
  }
  sessionViews += 1;

  const today = todayLocal();
  if (today > durable.lastDay) {
    durable.distinctDays += 1;
    durable.lastDay = today;
  }
  durable.lastSeen = Date.now();
  durable.legacySeeded = true;

  writeJSON(KEY, durable);
  const confirmed = decodeEngagement(readJSON(KEY));
  writeRaw("session", SESSION_KEY, String(sessionViews));
  if (confirmed && confirmed.legacySeeded === true) {
    removeRaw("session", LEGACY_KEY);
  }

  return {
    preBumpSourceViews,
    engaged: isEngaged(durable),
    sessionViews,
  };
}
