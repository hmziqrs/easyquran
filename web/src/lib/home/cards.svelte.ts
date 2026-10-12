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
  onPageHide,
  readJSON,
  trailingDebounce,
  writeJSON,
} from "#lib/storage/index.js";

/**
 * Home widget cards (pinned surah / ayah / range / zikr / azkar set), each with
 * an optional recite target and a plain tap counter. Local-only per §11.3 —
 * sibling of `easyquran.home`, never synced; stable random ids keep a future
 * account-sync layer a 1:1 mapping with no migration.
 */

const STORAGE_KEY = "easyquran.home.cards";
const SCHEMA_VERSION = 1;

export const CARD_KINDS = ["surah", "ayah", "range", "zikr", "snippet"] as const;
export type CardKind = (typeof CARD_KINDS)[number];

export interface CardBase {
  readonly id: string;
  readonly kind: CardKind;
  /** Recited tally — plain total, no daily/weekly bucketing yet. */
  readonly count: number;
  /** Optional goal; the card reads as done once count reaches it. */
  readonly target?: number;
  readonly createdAt: string;
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

export type HomeCard = SurahCard | AyahCard | RangeCard | ZikrCard | SnippetCard;

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
  }
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
  #persist = trailingDebounce(() => this.#writeNow(), PERSIST_WAIT_MS);

  hydrate(): void {
    if (this.#hydrated || !browser) return;
    this.#hydrated = true;
    this.#cards = decodeCards(readJSON(STORAGE_KEY));
    // A tab close must not eat the last taps of a session.
    onPageHide(() => this.#persist.flush());
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

  increment(id: string): void {
    this.#cards = this.#cards.map((card) =>
      card.id === id && card.count < COUNT_MAX ? { ...card, count: card.count + 1 } : card,
    );
    this.#persist.schedule();
  }

  reset(id: string): void {
    this.#cards = this.#cards.map((card) => (card.id === id ? { ...card, count: 0 } : card));
    this.#persist.schedule();
  }

  setTarget(id: string, target: number | undefined): void {
    const clamped = target === undefined ? undefined : asNumber(target, 1, TARGET_MAX);
    if (target !== undefined && clamped === undefined) return;
    this.#cards = this.#cards.map((card) => (card.id === id ? withTarget(card, clamped) : card));
    this.#persist.schedule();
  }

  #writeNow(): void {
    if (!browser) return;
    writeJSON(STORAGE_KEY, { v: SCHEMA_VERSION, cards: this.#cards });
  }
}

export const homeCards = new HomeCardsStore();
