import { browser } from "$app/env";

import { snippetForSlug } from "#lib/data/snippets.js";
import { TRANSLATION_CATALOGUE_BY_ID } from "#lib/quran/catalogue.js";
import type { JsonRecord } from "#lib/storage/decoders.js";
import {
  asArray,
  asLiteral,
  asNumber,
  asObject,
  asString,
  onStorageKey,
  onPageHide,
  readJSON,
  trailingDebounce,
  writeJSON,
} from "#lib/storage/index.js";

import { parseTimeHHMM, type ScheduleSlot } from "./schedule.js";

/**
 * Home widget cards (pinned surah / ayah / range / zikr / azkar set, plus
 * multi-item "collection" passes), each with an optional recite target, a plain
 * tap counter, a capped recite-event ring for streak/window stats, and an
 * optional fixed-clock schedule. Local-only per §11.3 — sibling of
 * `easyquran.home`, never synced; stable random ids keep a future account-sync
 * layer a 1:1 mapping with no migration.
 */

const STORAGE_KEY = "easyquran.home.cards";
const SCHEMA_VERSION = 1;

export const CARD_KINDS = ["surah", "ayah", "range", "zikr", "snippet", "collection"] as const;
export type CardKind = (typeof CARD_KINDS)[number];

export interface CardBase {
  readonly id: string;
  readonly kind: CardKind;
  /** Recited tally — plain total, no daily/weekly bucketing yet. */
  readonly count: number;
  /** Optional goal; the card reads as done once count reaches it. */
  readonly target?: number;
  readonly createdAt: string;
  /**
   * Recite-event ring (epoch ms, oldest first). Streaks and today/7d/30d
   * window stats derive from this; `count` stays the uncapped fast total, so
   * the two may diverge once COUNT_MAX pins the tally but the ring keeps
   * accruing (and for legacy cards written before the ring existed).
   */
  readonly events?: readonly number[];
  /** Fixed-clock recite slots (see ./schedule.js); absent when unset. */
  readonly schedule?: readonly ScheduleSlot[];
}

export interface SurahCard extends CardBase {
  readonly kind: "surah";
  readonly surah: number;
  /** Pinned translation, or null for the Arabic mushaf. */
  readonly sourceId: string | null;
}

export interface AyahCard extends CardBase {
  readonly kind: "ayah";
  readonly surah: number;
  readonly ayah: number;
  readonly sourceId: string | null;
}

export interface RangeCard extends CardBase {
  readonly kind: "range";
  readonly surah: number;
  readonly ayahStart: number;
  readonly ayahEnd: number;
  readonly sourceId: string | null;
}

export interface ZikrCard extends CardBase {
  readonly kind: "zikr";
  readonly text: string;
}

export interface SnippetCard extends CardBase {
  readonly kind: "snippet";
  readonly slug: string;
  readonly sourceId: string | null;
}

/**
 * One entry of a collection card: a reader reference (surah + optional ayah
 * span) or a snippet slug — mutually exclusive, narrowed downstream via
 * `"slug" in item`. A collection is recited as passes: one increment() = one
 * completed pass over all items.
 */
export type CollectionItem =
  | {
      readonly surah: number;
      readonly ayahStart?: number;
      readonly ayahEnd?: number;
      readonly sourceId: string | null;
    }
  | {
      readonly slug: string;
      readonly sourceId: string | null;
    };

export interface CollectionCard extends CardBase {
  readonly kind: "collection";
  readonly items: readonly CollectionItem[];
}

export type HomeCard = SurahCard | AyahCard | RangeCard | ZikrCard | SnippetCard | CollectionCard;

const SURAH_MIN = 1;
const SURAH_MAX = 114;
/** Flat cap at decode; the add-form validates against the real per-surah ayahCount. */
const AYAH_MAX = 286;
/** Same span cap the range API enforces (EC-39). */
const RANGE_SPAN_MAX = 50;
const TARGET_MAX = 10_000;
const COUNT_MAX = 1_000_000;
const ZIKR_TEXT_MAX = 120;
const ID_MAX = 64;
const CREATED_AT_MAX = 32;
/** Decode keeps the newest MAX_CARDS rows; add() refuses past the cap. */
export const MAX_CARDS = 24;
/** Items per collection card; decode keeps the first MAX_COLLECTION_ITEMS valid ones. */
export const MAX_COLLECTION_ITEMS = 12;
/** Recite-event ring bound (reading-analytics.ts QUEUE_CAP precedent); oldest fall off. */
export const EVENTS_CAP = 500;
/** Slots per card schedule; decode keeps the first MAX_SCHEDULE_SLOTS valid ones. */
export const MAX_SCHEDULE_SLOTS = 8;
const SLOT_LABEL_MAX = 40;
/** Epoch-ms upper bound (year 2100) — drops absurd-future garbage timestamps. */
const EVENT_MS_MAX = 4_102_444_800_000;

/** How long a tap burst waits before the envelope hits localStorage. */
const PERSIST_WAIT_MS = 400;

export function newCardId(): string {
  return crypto.randomUUID();
}

export function nowStamp(): string {
  return new Date().toISOString();
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is one card's unparsed sourceId from the localStorage envelope; this decoder is its parse boundary.
function decodeSourceId(raw: unknown): string | null {
  const id = asString(raw);
  if (id === undefined) return null;
  return TRANSLATION_CATALOGUE_BY_ID.has(id) ? id : null;
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is one card's unparsed target from the localStorage envelope; this decoder is its parse boundary.
function decodeTarget(raw: unknown): number | undefined {
  return asNumber(raw, 1, TARGET_MAX);
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is one element of the unparsed events ring; this decoder is its parse boundary.
function decodeEventMs(raw: unknown): number | undefined {
  return asNumber(raw, 0, EVENT_MS_MAX);
}

/** Newest EVENTS_CAP stamps win; an empty/garbage ring is simply omitted. */
// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the unparsed events array from the localStorage envelope; this decoder is its parse boundary.
function decodeEvents(raw: unknown): number[] | undefined {
  const events = asArray(raw, decodeEventMs).slice(-EVENTS_CAP);
  return events.length > 0 ? events : undefined;
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- row is one element of the unparsed schedule array; this function is its per-row parser.
function decodeSlot(row: unknown): ScheduleSlot | undefined {
  const record = asObject(row);
  if (!record) return undefined;
  const id = asString(record.id);
  const label = asString(record.label);
  const time = parseTimeHHMM(asString(record.time) ?? "");
  if (id === undefined || id.length === 0 || id.length > ID_MAX) return undefined;
  if (label === undefined || label.length === 0 || label.length > SLOT_LABEL_MAX) return undefined;
  if (time === undefined) return undefined;
  return { id, label, time };
}

/** First MAX_SCHEDULE_SLOTS valid slots win; none valid → the field is omitted. */
// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the unparsed schedule array from the localStorage envelope; this decoder is its parse boundary.
function decodeSchedule(raw: unknown): ScheduleSlot[] | undefined {
  const slots = asArray(raw, decodeSlot).slice(0, MAX_SCHEDULE_SLOTS);
  return slots.length > 0 ? slots : undefined;
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- row is one element of the unparsed items array; this function is its per-row parser.
function decodeCollectionItem(row: unknown): CollectionItem | undefined {
  const record = asObject(row);
  if (!record) return undefined;
  const sourceId = decodeSourceId(record.sourceId);
  // The slug is the discriminator: present → snippet item, and reference
  // fields (if any rode along) are dropped like every unknown field.
  const slug = asString(record.slug);
  if (slug !== undefined) {
    if (snippetForSlug(slug) === null) return undefined;
    return { slug, sourceId };
  }
  const surah = asNumber(record.surah, SURAH_MIN, SURAH_MAX);
  if (surah === undefined) return undefined;
  // An ayah span is all-or-nothing: one bound alone drops the item.
  const ayahStart = asNumber(record.ayahStart, 1, AYAH_MAX);
  const ayahEnd = asNumber(record.ayahEnd, 1, AYAH_MAX);
  if (ayahStart === undefined && ayahEnd === undefined) {
    return { surah, sourceId };
  }
  if (ayahStart === undefined || ayahEnd === undefined || ayahStart > ayahEnd) return undefined;
  if (ayahEnd - ayahStart + 1 > RANGE_SPAN_MAX) return undefined;
  return { surah, ayahStart, ayahEnd, sourceId };
}

function decodeBase<K extends CardKind>(
  row: JsonRecord,
  kind: K,
): (Omit<CardBase, "kind"> & { kind: K }) | undefined {
  const id = asString(row.id);
  if (id === undefined || id.length === 0 || id.length > ID_MAX) return undefined;
  const count = asNumber(row.count, 0, COUNT_MAX) ?? 0;
  const createdAt = asString(row.createdAt) ?? nowStamp();
  return {
    id,
    kind,
    count,
    target: decodeTarget(row.target),
    createdAt: createdAt.length > 0 && createdAt.length <= CREATED_AT_MAX ? createdAt : nowStamp(),
    events: decodeEvents(row.events),
    schedule: decodeSchedule(row.schedule),
  };
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- row is one element of the untyped localStorage cards array; this function is its per-row parser.
function decodeCard(row: unknown): HomeCard | undefined {
  const record = asObject(row);
  if (!record) return undefined;
  const shim: JsonRecord = record;
  const kind = asLiteral(shim.kind, CARD_KINDS);
  if (kind === undefined) return undefined;
  const surah = asNumber(shim.surah, SURAH_MIN, SURAH_MAX);
  switch (kind) {
    case "surah": {
      if (surah === undefined) return undefined;
      const base = decodeBase(shim, kind);
      return base === undefined
        ? undefined
        : { ...base, surah, sourceId: decodeSourceId(shim.sourceId) };
    }
    case "ayah": {
      if (surah === undefined) return undefined;
      const ayah = asNumber(shim.ayah, 1, AYAH_MAX);
      if (ayah === undefined) return undefined;
      const base = decodeBase(shim, kind);
      return base === undefined
        ? undefined
        : { ...base, surah, ayah, sourceId: decodeSourceId(shim.sourceId) };
    }
    case "range": {
      if (surah === undefined) return undefined;
      const ayahStart = asNumber(shim.ayahStart, 1, AYAH_MAX);
      const ayahEnd = asNumber(shim.ayahEnd, 1, AYAH_MAX);
      if (ayahStart === undefined || ayahEnd === undefined || ayahStart > ayahEnd) return undefined;
      if (ayahEnd - ayahStart + 1 > RANGE_SPAN_MAX) return undefined;
      const base = decodeBase(shim, kind);
      return base === undefined
        ? undefined
        : { ...base, surah, ayahStart, ayahEnd, sourceId: decodeSourceId(shim.sourceId) };
    }
    case "zikr": {
      const text = asString(shim.text);
      if (text === undefined || text.trim().length === 0 || text.length > ZIKR_TEXT_MAX) {
        return undefined;
      }
      const base = decodeBase(shim, kind);
      return base === undefined ? undefined : { ...base, text };
    }
    case "snippet": {
      const slug = asString(shim.slug);
      if (slug === undefined || snippetForSlug(slug) === null) return undefined;
      const base = decodeBase(shim, kind);
      return base === undefined
        ? undefined
        : { ...base, slug, sourceId: decodeSourceId(shim.sourceId) };
    }
    case "collection": {
      const items = asArray(shim.items, decodeCollectionItem).slice(0, MAX_COLLECTION_ITEMS);
      if (items.length === 0) return undefined;
      const base = decodeBase(shim, kind);
      return base === undefined ? undefined : { ...base, items };
    }
  }
}

/**
 * Duplicate: fresh id and createdAt, zeroed tally, empty history — refs
 * (collection items) and schedule slots deep-copy so the copy shares no arrays
 * with the original.
 */
function duplicateCard(card: HomeCard): HomeCard {
  if (card.kind === "collection") {
    return {
      ...card,
      id: newCardId(),
      count: 0,
      createdAt: nowStamp(),
      events: undefined,
      schedule: card.schedule?.map((slot) => ({ ...slot })),
      items: card.items.map((item) => ({ ...item })),
    };
  }
  return {
    ...card,
    id: newCardId(),
    count: 0,
    createdAt: nowStamp(),
    events: undefined,
    schedule: card.schedule?.map((slot) => ({ ...slot })),
  };
}

/** Replaces (or clears) a card's target without touching the tally. */
function withTarget(card: HomeCard, clamped: number | undefined): HomeCard {
  if (clamped === undefined) return { ...card, target: undefined };
  return { ...card, target: clamped };
}

/** Forward-compatible decode: bad rows drop, caps clamp, nothing throws. */
// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the untyped localStorage JSON boundary (readJSON → JSON.parse); this function is the parser and validates every field below.
export function decodeCards(raw: unknown): readonly HomeCard[] {
  const stored = asObject(raw);
  if (!stored) return [];
  if (stored.v !== SCHEMA_VERSION) return [];
  const cards = asArray(stored.cards, decodeCard);
  const seen = new Set<string>();
  const unique: HomeCard[] = [];
  for (const card of cards) {
    if (seen.has(card.id)) continue;
    seen.add(card.id);
    unique.push(card);
    if (unique.length >= MAX_CARDS) break;
  }
  return unique;
}

class HomeCardsStore {
  #cards = $state.raw<readonly HomeCard[]>([]);
  #hydrated = false;
  #teardown: (() => void) | null = null;
  #persist = trailingDebounce(() => this.#writeNow(), PERSIST_WAIT_MS);

  constructor() {
    if (!browser) return;
    // Cross-tab reconciliation: another tab's add/recite/edit lands here on
    // its write — the same seam the stacked-translations store uses, so the
    // azkar page's zikr panel and every tile stay current in every tab.
    this.#teardown = onStorageKey(STORAGE_KEY, () => {
      const next = decodeCards(readJSON(STORAGE_KEY));
      // Serialized compare: cheap at card-cap size and exact across every
      // field (count, target, schedule, items), so debounced writes that
      // change nothing visible never re-render.
      if (JSON.stringify(next) !== JSON.stringify(this.#cards)) this.#cards = next;
    });
  }

  hydrate(): void {
    if (this.#hydrated || !browser) return;
    this.#hydrated = true;
    this.#cards = decodeCards(readJSON(STORAGE_KEY));
    // A tab close must not eat the last taps of a session.
    onPageHide(() => this.#persist.flush());
  }

  dispose(): void {
    this.#teardown?.();
    this.#teardown = null;
  }

  get list(): readonly HomeCard[] {
    return this.#cards;
  }

  get full(): boolean {
    return this.#cards.length >= MAX_CARDS;
  }

  /** Returns false when the store is full — callers surface that in the form. */
  add(card: HomeCard): boolean {
    if (this.full) return false;
    this.#cards = [...this.#cards, card];
    this.#persist.schedule();
    return true;
  }

  remove(id: string): void {
    this.#cards = this.#cards.filter((card) => card.id !== id);
    this.#persist.schedule();
  }

  /**
   * One completed recite pass. The tally pins at COUNT_MAX but the event ring
   * keeps accruing either way, so streaks and window stats stay live for a
   * maxed-out card (count and ring total may diverge there by design).
   */
  increment(id: string): void {
    const now = Date.now();
    this.#cards = this.#cards.map((card) => {
      if (card.id !== id) return card;
      const events = [...(card.events ?? []), now].slice(-EVENTS_CAP);
      if (card.count >= COUNT_MAX) return { ...card, events };
      return { ...card, count: card.count + 1, events };
    });
    this.#persist.schedule();
  }

  /**
   * Tasbih-style reset: the tally zeroes but the event ring survives, so
   * streaks and history outlive the counter.
   */
  reset(id: string): void {
    this.#cards = this.#cards.map((card) => (card.id === id ? { ...card, count: 0 } : card));
    this.#persist.schedule();
  }

  /**
   * Id-keyed upsert — the single edit-save path for a card's target,
   * translation, schedule and collection items. Ignored when the id is absent
   * (never re-adds a removed card).
   */
  replace(next: HomeCard): void {
    if (!this.#cards.some((card) => card.id === next.id)) return;
    this.#cards = this.#cards.map((card) => (card.id === next.id ? next : card));
    this.#persist.schedule();
  }

  /** Deep copy under a fresh id; false when the id is unknown or the store is full. */
  duplicate(id: string): boolean {
    const card = this.#cards.find((entry) => entry.id === id);
    if (card === undefined) return false;
    return this.add(duplicateCard(card));
  }

  setTarget(id: string, target: number | undefined): void {
    const clamped = target === undefined ? undefined : asNumber(target, 1, TARGET_MAX);
    if (target !== undefined && clamped === undefined) return;
    this.#cards = this.#cards.map((card) => (card.id === id ? withTarget(card, clamped) : card));
    this.#persist.schedule();
  }

  /**
   * Re-pin a card's translation (null = Arabic mushaf). Unknown catalogue ids
   * degrade to null, matching decodeSourceId; count, events and schedule are
   * untouched. This is the "follow what the reader just switched to" path —
   * the azkar page's translation-change offer.
   */
  setSource(id: string, sourceId: string | null): void {
    const next = sourceId === null ? null : decodeSourceId(sourceId);
    if (sourceId !== null && next === null) return;
    this.#cards = this.#cards.map((card) => {
      // Zikr cards carry no sourceId (always the Arabic phrase) — a no-op there.
      if (card.id !== id || !("sourceId" in card) || card.sourceId === next) return card;
      return { ...card, sourceId: next };
    });
    this.#persist.schedule();
  }

  #writeNow(): void {
    if (!browser) return;
    writeJSON(STORAGE_KEY, { v: SCHEMA_VERSION, cards: this.#cards });
  }
}

export const homeCards = new HomeCardsStore();
