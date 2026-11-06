/**
 * Pure merge rules for every synced domain: no IO, no clock. `contracts/sync.md`
 * is the spec; `contracts/sync-vectors.json` pins this file and
 * `rust/crates/eq-sync-core` to the same results. Rows are plain JSON so a
 * register this build does not know about survives a merge untouched.
 */

import { asNumber, asObject, asString } from "#lib/storage/decoders.js";

export type Json = null | boolean | number | string | readonly Json[] | JsonObject;
export interface JsonObject {
  readonly [key: string]: Json;
}

export type MergeRule = "highest" | "lowest";
export type MergeDomain = "thread" | "progress" | "bookmark" | "folder" | "event";

const STAMP_RE = /^\d{13}-[0-9a-f]{4}-[A-Za-z0-9._-]{1,64}$/u;

export function isValidStamp(stamp: string): boolean {
  return STAMP_RE.test(stamp);
}

export function formatStamp(ms: number, counter: number, device: string): string {
  const time = String(ms).padStart(13, "0");
  const count = Math.min(counter, 0xffff).toString(16).padStart(4, "0");
  return `${time}-${count}-${device}`;
}

export interface ParsedStamp {
  readonly ms: number;
  readonly counter: number;
  readonly device: string;
}

export function parseStamp(stamp: string): ParsedStamp | null {
  if (!isValidStamp(stamp)) return null;
  return {
    ms: Number(stamp.slice(0, 13)),
    counter: Number.parseInt(stamp.slice(14, 18), 16),
    device: stamp.slice(19),
  };
}

/** ASCII by format, so code-unit order is byte order. `null` ranks lowest. */
export function compareStamps(a: string | null, b: string | null): number {
  if (a === b) return 0;
  if (a === null) return -1;
  if (b === null) return 1;
  return a < b ? -1 : 1;
}

function compareUtf16(a: string, b: string): number {
  if (a === b) return 0;
  return a < b ? -1 : 1;
}

/** RFC 8785 canonical JSON, used only to break ties between identical stamps. */
export function canonicalJson(value: Json | undefined): string {
  if (value === null || value === undefined) return "null";
  if (isJsonArray(value)) return `[${value.map((item) => canonicalJson(item)).join(",")}]`;
  const record = asRecord(value);
  if (record === null) return JSON.stringify(value);
  const keys = Object.keys(record).sort((a, b) => compareUtf16(a, b));
  const fields = keys.map((key) => `${JSON.stringify(key)}:${canonicalJson(record[key])}`);
  return `{${fields.join(",")}}`;
}

function isJsonArray(value: Json): value is readonly Json[] {
  return Array.isArray(value);
}

const utf8 = new TextEncoder();

function compareBytes(a: string, b: string): number {
  const left = utf8.encode(a);
  const right = utf8.encode(b);
  const shared = Math.min(left.length, right.length);
  for (let i = 0; i < shared; i += 1) {
    const diff = (left[i] ?? 0) - (right[i] ?? 0);
    if (diff !== 0) return diff;
  }
  return left.length - right.length;
}

function asRecord(value: Json | undefined): JsonObject | null {
  return asObject(value) ?? null;
}

function stampOf(register: Json | undefined): string | null {
  return asString(asRecord(register)?.stamp) ?? null;
}

function tieBreak<T extends Json>(a: T, b: T): T {
  return compareBytes(canonicalJson(a), canonicalJson(b)) >= 0 ? a : b;
}

export function mergeRegister<T extends Json>(a: T, b: T, rule: MergeRule = "highest"): T {
  const order = compareStamps(stampOf(a), stampOf(b));
  if (order === 0) return tieBreak(a, b);
  const aWins = rule === "highest" ? order > 0 : order < 0;
  return aWins ? a : b;
}

function isRegister(value: Json | undefined): boolean {
  const record = asRecord(value);
  return record !== null && "stamp" in record;
}

function mergeField(x: Json | undefined, y: Json | undefined, rule: MergeRule): Json {
  if (x === undefined) return y ?? null;
  if (y === undefined) return x;
  if (isRegister(x) && isRegister(y)) return mergeRegister(x, y, rule);
  return tieBreak(x, y);
}

function mergeFields(a: JsonObject, b: JsonObject, ruleFor: (key: string) => MergeRule): JsonObject {
  const out: Record<string, Json> = {};
  for (const key of new Set([...Object.keys(a), ...Object.keys(b)])) {
    out[key] = mergeField(a[key], b[key], ruleFor(key));
  }
  return out;
}

const highest = (): MergeRule => "highest";

/** Identity is set once; cursor, lifecycle and anything newer take the highest stamp. */
export function mergeThread(a: JsonObject, b: JsonObject): JsonObject {
  return mergeFields(a, b, (key) => (key === "identity" ? "lowest" : "highest"));
}

function ayahOf(row: JsonObject): number {
  return asNumber(row.furthestAyah, 0, Number.MAX_SAFE_INTEGER) ?? 0;
}

/** MAX of `furthestAyah`: progress never regresses. */
export function mergeProgress(a: JsonObject, b: JsonObject): JsonObject {
  const left = ayahOf(a);
  const right = ayahOf(b);
  if (left !== right) return left > right ? a : b;
  return tieBreak(a, b);
}

/** Whole-row register: folder, key, context and deleted move together. */
export function mergeBookmark(a: JsonObject, b: JsonObject): JsonObject {
  return mergeFields(a, b, highest);
}

function forceAlive(row: JsonObject): JsonObject {
  const deleted = asRecord(row.deleted);
  return deleted === null ? row : { ...row, deleted: { ...deleted, value: false } };
}

/**
 * Name, position and deleted each merge alone. The default folder cannot be
 * deleted, and either side knowing a folder is the default is enough.
 */
export function mergeFolder(a: JsonObject, b: JsonObject): JsonObject {
  if (a.isDefault === true || b.isDefault === true) {
    return mergeFields(forceAlive(a), forceAlive(b), highest);
  }
  return mergeFields(a, b, highest);
}

/** One register per setting key, so two settings changed apart both survive. */
export function mergeSettings(a: JsonObject, b: JsonObject): JsonObject {
  return mergeFields(a, b, highest);
}

/** The home layout is one document. */
export function mergeLayout(a: JsonObject, b: JsonObject): JsonObject {
  return mergeRegister(a, b, "highest");
}

/** Reading events are insert-once by id, so both inputs are the same event. */
export function mergeEvent(a: JsonObject, b: JsonObject): JsonObject {
  return tieBreak(a, b);
}

const DOMAIN_MERGE = {
  thread: mergeThread,
  progress: mergeProgress,
  bookmark: mergeBookmark,
  folder: mergeFolder,
  event: mergeEvent,
} satisfies Record<MergeDomain, (a: JsonObject, b: JsonObject) => JsonObject>;

function rowId(domain: MergeDomain, row: JsonObject): string {
  if (domain !== "progress") return asString(row.id) ?? "";
  const surah = asNumber(row.surah, 0, Number.MAX_SAFE_INTEGER);
  return surah === undefined ? "" : String(surah).padStart(8, "0");
}

/**
 * Union by id, sorted by id. Rows with different ids are never merged, so two
 * threads on one scope and two folders with one name both stay.
 */
export function mergeRows(
  domain: MergeDomain,
  a: readonly JsonObject[],
  b: readonly JsonObject[],
): JsonObject[] {
  const byId = new Map<string, JsonObject>();
  const merge = DOMAIN_MERGE[domain];
  for (const row of [...a, ...b]) {
    const id = rowId(domain, row);
    const existing = byId.get(id);
    byId.set(id, existing === undefined ? row : merge(existing, row));
  }
  return [...byId.entries()]
    .sort(([x], [y]) => compareBytes(x, y))
    .map(([, row]) => row);
}

function bookmarkValue(row: JsonObject): JsonObject | null {
  return asRecord(asRecord(row.register)?.value);
}

function withBookmarkField(row: JsonObject, field: string, value: Json): JsonObject {
  const register = asRecord(row.register);
  const current = bookmarkValue(row);
  if (register === null || current === null) return row;
  return { ...row, register: { ...register, value: { ...current, [field]: value } } };
}

function isLive(row: JsonObject): boolean {
  return bookmarkValue(row)?.deleted !== true;
}

function outranks(a: JsonObject, b: JsonObject): boolean {
  const order = compareStamps(stampOf(a.register), stampOf(b.register));
  if (order !== 0) return order > 0;
  return compareBytes(canonicalJson(a), canonicalJson(b)) > 0;
}

function applyFolderDelete(
  row: JsonObject,
  folders: readonly JsonObject[],
  defaultFolderId: string,
): JsonObject {
  if (!isLive(row)) return row;
  const folderId = bookmarkValue(row)?.folderId;
  const deleted = asRecord(folders.find((f) => f.id === folderId)?.deleted);
  if (deleted === null || deleted.value !== true) return row;
  return compareStamps(stampOf(row.register), stampOf(deleted)) > 0
    ? withBookmarkField(row, "folderId", defaultFolderId)
    : withBookmarkField(row, "deleted", true);
}

/**
 * Set-level rules, run after merging rows. Writes keep their stamp: at equal
 * stamps `deleted: true` has the higher canonical bytes, so a tombstone made here
 * beats the live copy on every replica. Older bookmarks die with a deleted folder
 * and newer ones move to the default folder; then two live ids on one key leave
 * only the highest stamp.
 */
export function convergeBookmarks(
  rows: readonly JsonObject[],
  folders: readonly JsonObject[],
  defaultFolderId: string,
): JsonObject[] {
  const out = rows.map((row) => applyFolderDelete(row, folders, defaultFolderId));
  const winners = new Map<string, number>();
  const losers: number[] = [];
  out.forEach((row, index) => {
    if (!isLive(row)) return;
    const value = bookmarkValue(row);
    const slot = `${asString(value?.folderId) ?? ""}|${asString(value?.key) ?? ""}`;
    const current = winners.get(slot);
    const holder = current === undefined ? undefined : out[current];
    if (current === undefined || holder === undefined) {
      winners.set(slot, index);
    } else if (outranks(holder, row)) {
      losers.push(index);
    } else {
      losers.push(current);
      winners.set(slot, index);
    }
  });
  for (const index of losers) {
    const row = out[index];
    if (row !== undefined) out[index] = withBookmarkField(row, "deleted", true);
  }
  return out;
}

/** Applied before a ratio is stamped so every replica stores the same value. */
export function roundRatio(ratio: number): number {
  return Math.round(ratio * 10_000) / 10_000;
}
