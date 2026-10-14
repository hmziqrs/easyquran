import type { UiLocale } from "#lib/i18n/locales.js";
import {
  reader_cards_due,
  reader_cards_items_many,
  reader_cards_items_one,
  reader_cards_kind_ayah,
  reader_cards_kind_collection,
  reader_cards_kind_range,
  reader_cards_kind_snippet,
  reader_cards_kind_surah,
  reader_cards_kind_zikr,
  reader_cards_next_hours,
  reader_cards_next_minutes,
  reader_cards_passes_one,
  reader_cards_passes_plain,
} from "#lib/i18n/m/reader.js";
import { publicHref } from "#lib/i18n/public-href.js";
import { yoursPageHref } from "#lib/i18n/reader.js";
import { peekTranslationName } from "#lib/quran/catalogue.js";
import { snippetForSlug } from "#lib/data/snippets.js";
import {
  snippetPathFor,
  surahAyahPathFor,
  surahPathFor,
  surahRouteContext,
  type SurahRouteContext,
} from "#lib/data/quran.js";
import type { QuranData } from "#lib/data/quran-data.js";
import { entryAttrs, type EntryLinkAttrs } from "#lib/sessions/entry.js";
import type { CollectionCard, CollectionItem, HomeCard } from "./cards.svelte.js";
import type { SlotState } from "./schedule.js";

/**
 * Pure copy + link builders shared by the card tile, the add/edit form and the
 * per-card detail page, so every surface renders the same title, meta line and
 * reader navigation for a card. Reader hrefs come only from the guarded
 * `*For(ctx, …)` family — the context is the card's own pinned translation
 * when it still resolves in the catalogue, else the Arabic mushaf (the
 * ctxForSavedSource pattern of /yours, EC-38). App routes (the detail page,
 * /yours) come from their canonical helpers, never hand-built reader strings.
 */

/** Canonical href of a card's detail page; the id is a device-local uuid. */
export function cardDetailHref(id: string): `/${string}` {
  return `/yours/cards/${encodeURIComponent(id)}`;
}

/** Back-link href for card surfaces: the canonical /yours hub (rerouted for /ar). */
export function yoursBackHref(): string {
  return publicHref(yoursPageHref());
}

// Saved-translation reopen: still in the catalogue → that translation, else
// Arabic. Same seam as /yours' bookmark rows and the reader's resume links.
export function ctxForSavedSource(sourceId: string | null | undefined): SurahRouteContext {
  if (sourceId !== null && sourceId !== undefined && peekTranslationName(sourceId) !== null) {
    return surahRouteContext(sourceId);
  }
  return surahRouteContext("uthmani");
}

export function cardKindLabel(kind: HomeCard["kind"], locale: UiLocale): string {
  if (kind === "surah") return reader_cards_kind_surah(undefined, { locale });
  if (kind === "ayah") return reader_cards_kind_ayah(undefined, { locale });
  if (kind === "range") return reader_cards_kind_range(undefined, { locale });
  if (kind === "zikr") return reader_cards_kind_zikr(undefined, { locale });
  if (kind === "snippet") return reader_cards_kind_snippet(undefined, { locale });
  return reader_cards_kind_collection(undefined, { locale });
}

/** One completed pass per +1 — collections count passes, not recites. */
export function passesCountLabel(count: number, locale: UiLocale): string {
  if (count === 1) return reader_cards_passes_one(undefined, { locale });
  return reader_cards_passes_plain({ count }, { locale });
}

export function itemsCountLabel(count: number, locale: UiLocale): string {
  if (count === 1) return reader_cards_items_one(undefined, { locale });
  return reader_cards_items_many({ count }, { locale });
}

export interface ScheduleBadgeCopy {
  readonly due: boolean;
  readonly label: string;
}

/** Badge copy for one slotStateFor result — minutes under an hour, hours after. */
export function scheduleBadgeCopy(state: SlotState, locale: UiLocale): ScheduleBadgeCopy {
  if (state.state === "due") {
    return { due: true, label: reader_cards_due(undefined, { locale }) };
  }
  if (state.minutesUntil < 60) {
    return {
      due: false,
      label: reader_cards_next_minutes({ minutes: state.minutesUntil }, { locale }),
    };
  }
  return {
    due: false,
    label: reader_cards_next_hours({ hours: Math.floor(state.minutesUntil / 60) }, { locale }),
  };
}

export interface TileCopy {
  readonly title: string;
  readonly arabicTitle: boolean;
  /** "·"-separated kind / coordinates / pinned translation line. */
  readonly meta: string;
  readonly href: `/${string}` | null;
}

/** Collections carry no card-level sourceId — the first item's pins the context. */
function collectionSourceId(card: CollectionCard): string | null {
  return card.items[0]?.sourceId ?? null;
}

function sourceLabel(sourceId: string | null): string {
  if (sourceId === null) return "";
  return peekTranslationName(sourceId) ?? "";
}

function joinMeta(parts: readonly string[]): string {
  return parts.filter((part) => part.length > 0).join(" · ");
}

function localizedSnippetTitle(slug: string, locale: UiLocale): string {
  const snippet = snippetForSlug(slug);
  if (snippet === null) return slug;
  if (locale === "ar") return snippet.title.ar;
  return snippet.title.en;
}

/**
 * The title a collection renders when it carries no explicit name: the first
 * complete item (localized surah name, "s:a–b" span, or localized snippet
 * title), else the kind label — a collection is never untitled.
 */
export function collectionTitleFor(
  card: CollectionCard,
  quranData: QuranData | undefined,
  locale: UiLocale,
): string {
  const first = card.items[0];
  if (first === undefined) return cardKindLabel("collection", locale);
  if ("slug" in first) return localizedSnippetTitle(first.slug, locale);
  if (first.ayahStart !== undefined && first.ayahEnd !== undefined) {
    return `${first.surah}:${first.ayahStart}–${first.ayahEnd}`;
  }
  const surah = quranData?.surahByNum(first.surah);
  if (surah === undefined) return `Surah ${first.surah}`;
  return surah.name;
}

/** Per-item reader link; null only while the item's surah cannot resolve. */
export function collectionItemHref(
  item: CollectionItem,
  quranData: QuranData | undefined,
): `/${string}` | null {
  const ctx = ctxForSavedSource(item.sourceId);
  if ("slug" in item) return snippetPathFor(ctx, item.slug);
  const surah = quranData?.surahByNum(item.surah);
  if (surah === undefined) return null;
  if (item.ayahStart !== undefined && item.ayahEnd !== undefined) {
    return surahAyahPathFor(ctx, surah, item.ayahStart);
  }
  return surahPathFor(ctx, surah);
}

/** Contents-row label for one collection item (detail page lists these). */
export function collectionItemLabel(
  item: CollectionItem,
  quranData: QuranData | undefined,
  locale: UiLocale,
): string {
  if ("slug" in item) return localizedSnippetTitle(item.slug, locale);
  const surah = quranData?.surahByNum(item.surah);
  const name = surah === undefined ? `Surah ${item.surah}` : surah.name;
  if (item.ayahStart !== undefined && item.ayahEnd !== undefined) {
    return `${name} · ${item.surah}:${item.ayahStart}–${item.ayahEnd}`;
  }
  return name;
}

export interface CardContentRow {
  readonly label: string;
  /** Reader href via the guarded family; null = no link (zikr, unresolved surah). */
  readonly href: `/${string}` | null;
}

/** The detail page's Contents rows, builder order kept for collections. */
export function cardContents(
  card: HomeCard,
  quranData: QuranData | undefined,
  locale: UiLocale,
): readonly CardContentRow[] {
  if (card.kind === "collection") {
    return card.items.map((item) => ({
      label: collectionItemLabel(item, quranData, locale),
      href: collectionItemHref(item, quranData),
    }));
  }
  if (card.kind === "zikr") return [{ label: card.text, href: null }];
  if (card.kind === "snippet") {
    return [
      {
        label: localizedSnippetTitle(card.slug, locale),
        href: snippetPathFor(ctxForSavedSource(card.sourceId), card.slug),
      },
    ];
  }

  // surah / ayah / range — one row, the card's own pinned translation context.
  const ctx = ctxForSavedSource(card.sourceId);
  const surah = quranData?.surahByNum(card.surah);
  if (surah === undefined) return [];
  if (card.kind === "surah") return [{ label: surah.name, href: surahPathFor(ctx, surah) }];
  if (card.kind === "ayah") {
    return [
      {
        label: `${surah.name} · ${card.surah}:${card.ayah}`,
        href: surahAyahPathFor(ctx, surah, card.ayah),
      },
    ];
  }
  return [
    {
      label: `${surah.name} · ${card.surah}:${card.ayahStart}–${card.ayahEnd}`,
      href: surahAyahPathFor(ctx, surah, card.ayahStart),
    },
  ];
}

/** Everything a tile (or a detail header) renders for one card. */
export function tileCopyFor(
  card: HomeCard,
  quranData: QuranData | undefined,
  locale: UiLocale,
): TileCopy {
  if (card.kind === "zikr") {
    return { title: card.text, arabicTitle: true, meta: cardKindLabel("zikr", locale), href: null };
  }
  if (card.kind === "snippet") {
    return {
      title: localizedSnippetTitle(card.slug, locale),
      arabicTitle: false,
      meta: joinMeta([cardKindLabel("snippet", locale), sourceLabel(card.sourceId)]),
      href: snippetPathFor(ctxForSavedSource(card.sourceId), card.slug),
    };
  }
  if (card.kind === "collection") {
    const sourceId = collectionSourceId(card);
    const first = card.items[0];
    return {
      title: collectionTitleFor(card, quranData, locale),
      arabicTitle: false,
      meta: joinMeta([
        cardKindLabel("collection", locale),
        itemsCountLabel(card.items.length, locale),
        sourceLabel(sourceId),
      ]),
      href: first === undefined ? null : collectionItemHref(first, quranData),
    };
  }

  // surah / ayah / range — a personal card pinned to a translation opens that
  // translation once it still resolves; unknown ids fall back to the mushaf.
  const ctx = ctxForSavedSource(card.sourceId);
  const surah = quranData?.surahByNum(card.surah);
  if (surah === undefined) {
    return {
      title: `Surah ${card.surah}`,
      arabicTitle: false,
      meta: joinMeta([cardKindLabel(card.kind, locale), sourceLabel(card.sourceId)]),
      href: null,
    };
  }
  if (card.kind === "surah") {
    return {
      title: surah.name,
      arabicTitle: false,
      meta: joinMeta([cardKindLabel("surah", locale), sourceLabel(card.sourceId)]),
      href: surahPathFor(ctx, surah),
    };
  }
  if (card.kind === "ayah") {
    return {
      title: surah.name,
      arabicTitle: false,
      meta: joinMeta([
        cardKindLabel("ayah", locale),
        `${card.surah}:${card.ayah}`,
        sourceLabel(card.sourceId),
      ]),
      href: surahAyahPathFor(ctx, surah, card.ayah),
    };
  }
  return {
    title: surah.name,
    arabicTitle: false,
    meta: joinMeta([
      cardKindLabel("range", locale),
      `${card.surah}:${card.ayahStart}–${card.ayahEnd}`,
      sourceLabel(card.sourceId),
    ]),
    href: surahAyahPathFor(ctx, surah, card.ayahStart),
  };
}

interface ItemScope {
  readonly scope: string;
  readonly boundEnd: string | null;
}

/** Reading-thread scope for one collection item (docs/reading-model.md §2). */
function itemScope(item: CollectionItem): ItemScope {
  if ("slug" in item) return { scope: `snippet:${item.slug}`, boundEnd: null };
  if (item.ayahStart !== undefined && item.ayahEnd !== undefined) {
    return {
      scope: `range:${item.surah}:${item.ayahStart}-${item.surah}:${item.ayahEnd}`,
      boundEnd: `${item.surah}:${item.ayahEnd}`,
    };
  }
  return { scope: `surah:${item.surah}`, boundEnd: null };
}

/**
 * Entry tag for a link that opens a card: every open starts its own thread
 * (`origin=card`, the card id as ref), scoped to what the card holds so a
 * range or ayah card completes when its last verse is read.
 */
export function cardEntryAttrs(card: HomeCard, item?: CollectionItem): EntryLinkAttrs {
  const base = { origin: "card" as const, ref: card.id };
  if (item !== undefined) return entryAttrs({ ...base, ...itemScope(item) });
  if (card.kind === "collection") {
    const first = card.items[0];
    return first === undefined ? entryAttrs(base) : entryAttrs({ ...base, ...itemScope(first) });
  }
  if (card.kind === "snippet") return entryAttrs({ ...base, scope: `snippet:${card.slug}` });
  if (card.kind === "surah") return entryAttrs({ ...base, scope: `surah:${card.surah}` });
  if (card.kind === "ayah") {
    const key = `${card.surah}:${card.ayah}`;
    return entryAttrs({ ...base, scope: `range:${key}-${key}`, boundEnd: key });
  }
  if (card.kind === "range") {
    return entryAttrs({
      ...base,
      scope: `range:${card.surah}:${card.ayahStart}-${card.surah}:${card.ayahEnd}`,
      boundEnd: `${card.surah}:${card.ayahEnd}`,
    });
  }
  return entryAttrs(base);
}
