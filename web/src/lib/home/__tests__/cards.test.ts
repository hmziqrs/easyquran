import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

vi.mock("$app/env", () => ({ browser: true, dev: false, building: false }));
vi.mock("$app/env/public", () => ({
  PUBLIC_API_BASE_URL: "/api",
  PUBLIC_FCM_VAPID_KEY: "",
  PUBLIC_QURAN_API_BASE: "",
  PUBLIC_ENV: "local",
}));

import {
  decodeCards,
  EVENTS_CAP,
  MAX_CARDS,
  MAX_COLLECTION_ITEMS,
  MAX_SCHEDULE_SLOTS,
  type CollectionCard,
  type CollectionItem,
  type HomeCard,
  type SurahCard,
} from "../cards.svelte.js";

const VALID_TRANSLATION_ID = "en.sahih";
const KNOWN_SLUG = "ayat-al-kursi";
const STORAGE_KEY = "easyquran.home.cards";

/** The raw pre-decode row shape a card envelope carries. */
interface RawCardRow {
  id: string;
  kind: string;
  count: number;
  createdAt: string;
  surah?: number;
  ayah?: number;
  ayahStart?: number;
  ayahEnd?: number;
  text?: string;
  slug?: string;
  sourceId?: string | null;
  target?: number;
  items?: unknown;
  events?: unknown;
  schedule?: unknown;
}

function row(overrides: Partial<RawCardRow> = {}): RawCardRow {
  return {
    id: "card-1",
    kind: "surah",
    count: 0,
    createdAt: "2026-10-13T00:00:00.000Z",
    surah: 2,
    sourceId: null,
    ...overrides,
  };
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- cards is the pre-decode envelope payload; decodeCards is its parse boundary.
const envelope = (cards: unknown) => ({ v: 1, cards });

/** Builds a typed card through the decoder so fixtures stay valid by construction. */
function card(overrides: Partial<RawCardRow> = {}): HomeCard {
  const decoded = decodeCards(envelope([row(overrides)]));
  // SAFETY: fixtures above are valid rows, so decode yields exactly one card.
  return decoded[0] as HomeCard;
}

describe("decodeCards — bad rows drop, good rows survive", () => {
  it("returns an empty list for non-objects and version mismatches", () => {
    expect(decodeCards(undefined)).toEqual([]);
    expect(decodeCards("nope")).toEqual([]);
    expect(decodeCards({ v: 99, cards: [] })).toEqual([]);
  });

  it("decodes every kind", () => {
    const cards = decodeCards(
      envelope([
        row(),
        row({ id: "a", kind: "ayah", ayah: 255 }),
        row({ id: "r", kind: "range", ayahStart: 285, ayahEnd: 286 }),
        row({
          id: "z",
          kind: "zikr",
          text: "Subhanallah",
          surah: undefined,
          sourceId: undefined,
        }),
        row({ id: "s", kind: "snippet", slug: KNOWN_SLUG }),
      ]),
    );
    expect(cards.map((entry) => entry.kind)).toEqual(["surah", "ayah", "range", "zikr", "snippet"]);
  });

  it("drops unknown kinds, unknown snippets and malformed rows", () => {
    const cards = decodeCards(
      envelope([
        row({ kind: "weather" }),
        row({ id: "s", kind: "snippet", slug: "not-a-real-slug" }),
        row({ id: "z", kind: "zikr", text: "   " }),
        "junk",
        row({ surah: 999 }),
      ]),
    );
    expect(cards).toEqual([]);
  });

  it("drops ranges that are inverted or past the span cap", () => {
    expect(decodeCards(envelope([row({ kind: "range", ayahStart: 9, ayahEnd: 2 })]))).toEqual([]);
    expect(decodeCards(envelope([row({ kind: "range", ayahStart: 1, ayahEnd: 51 })]))).toEqual([]);
    const kept = decodeCards(envelope([row({ kind: "range", ayahStart: 1, ayahEnd: 50 })]));
    expect(kept).toHaveLength(1);
  });

  it("falls an unknown translation back to Arabic (null sourceId)", () => {
    const arabic = decodeCards(envelope([row({ sourceId: "xx.gone" })]));
    expect(arabic[0]?.kind === "surah" && arabic[0]?.sourceId).toBeNull();
    const kept = decodeCards(envelope([row({ sourceId: VALID_TRANSLATION_ID })]));
    expect(kept[0]?.kind === "surah" && kept[0]?.sourceId).toBe(VALID_TRANSLATION_ID);
  });

  it("keeps an optional target only within bounds", () => {
    expect(decodeCards(envelope([row({ target: 0 })]))[0]?.target).toBeUndefined();
    expect(decodeCards(envelope([row({ target: 33 })]))[0]?.target).toBe(33);
  });

  it("dedupes ids and caps the list", () => {
    const many = Array.from({ length: MAX_CARDS + 5 }, (_, i) => row({ id: `card-${i}` }));
    const cards = decodeCards(envelope(many));
    expect(cards).toHaveLength(MAX_CARDS);
    expect(new Set(cards.map((entry) => entry.id)).size).toBe(MAX_CARDS);
    expect(decodeCards(envelope([row(), row()]))).toHaveLength(1);
  });
});

describe("decodeCards — collection items", () => {
  /** Same SAFETY as card(): a known-good collection row, so decode yields it. */
  function collectionRow(items: unknown[]): RawCardRow {
    return row({ id: "col", kind: "collection", surah: undefined, sourceId: undefined, items });
  }

  function decodedItems(items: unknown[]): CollectionItem[] {
    const first = decodeCards(envelope([collectionRow(items)]))[0];
    if (first === undefined || first.kind !== "collection") return [];
    return [...first.items];
  }

  it("decodes mixed reference and snippet items", () => {
    const items = decodedItems([
      { surah: 2 },
      { surah: 18, ayahStart: 1, ayahEnd: 10, sourceId: VALID_TRANSLATION_ID },
      { slug: KNOWN_SLUG },
    ]);
    expect(items).toEqual([
      { surah: 2, sourceId: null },
      { surah: 18, ayahStart: 1, ayahEnd: 10, sourceId: VALID_TRANSLATION_ID },
      { slug: KNOWN_SLUG, sourceId: null },
    ]);
  });

  it("drops invalid items one by one and keeps the rest", () => {
    const items = decodedItems([
      { slug: "not-a-real-slug" },
      { surah: 999 },
      { surah: 2, ayahStart: 9, ayahEnd: 2 },
      { surah: 2, ayahStart: 1, ayahEnd: 51 },
      { surah: 2, ayahStart: 5 },
      { surah: 2, ayahEnd: 5 },
      "junk",
      { surah: 18, ayahStart: 1, ayahEnd: 10 },
    ]);
    expect(items).toEqual([{ surah: 18, ayahStart: 1, ayahEnd: 10, sourceId: null }]);
  });

  it("drops a collection whose items all fail validation", () => {
    expect(decodeCards(envelope([collectionRow([{ surah: 999 }, { slug: "nope" }])]))).toEqual([]);
    expect(decodeCards(envelope([collectionRow([])]))).toEqual([]);
  });

  it("caps a collection at MAX_COLLECTION_ITEMS items", () => {
    const items = Array.from({ length: MAX_COLLECTION_ITEMS + 1 }, (_, i) => ({ surah: i + 1 }));
    expect(decodedItems(items)).toHaveLength(MAX_COLLECTION_ITEMS);
  });

  it("treats the slug as the discriminator over stray reference fields", () => {
    const items = decodedItems([{ slug: KNOWN_SLUG, surah: 999, ayahStart: 1, ayahEnd: 2 }]);
    expect(items).toEqual([{ slug: KNOWN_SLUG, sourceId: null }]);
  });

  it("falls an unknown item translation back to Arabic", () => {
    const items = decodedItems([{ surah: 2, sourceId: "xx.gone" }]);
    expect(items[0]?.sourceId).toBeNull();
  });
});

describe("decodeCards — event ring and schedule", () => {
  /** Epoch-ms upper bound the decoder accepts (year 2100); kept in sync with the store. */
  const EVENT_MS_MAX = 4_102_444_800_000;

  it("keeps a valid event ring, fractional stamps included", () => {
    const first = decodeCards(envelope([row({ events: [1, 2.5, EVENT_MS_MAX] })]))[0];
    expect(first?.events).toEqual([1, 2.5, EVENT_MS_MAX]);
  });

  it("drops invalid timestamps but keeps the card", () => {
    const first = decodeCards(envelope([row({ events: ["x", null, -1, EVENT_MS_MAX + 1, 5] })]))[0];
    expect(first?.events).toEqual([5]);
  });

  it("omits the ring when no timestamp survives", () => {
    const first = decodeCards(envelope([row({ events: ["x", -1] })]))[0];
    expect(first?.events).toBeUndefined();
  });

  it("trims the ring to the newest EVENTS_CAP stamps", () => {
    const events = Array.from({ length: EVENTS_CAP + 3 }, (_, i) => i + 1);
    const first = decodeCards(envelope([row({ events })]))[0];
    expect(first?.events).toHaveLength(EVENTS_CAP);
    expect(first?.events?.[0]).toBe(4);
    expect(first?.events?.[EVENTS_CAP - 1]).toBe(EVENTS_CAP + 3);
  });

  it("decodes valid slots and drops malformed ones", () => {
    const first = decodeCards(
      envelope([
        row({
          schedule: [
            { id: "s1", label: "Fajr", time: "05:00" },
            { id: "s2", label: "BadHour", time: "24:00" },
            { id: "s3", label: "NoPad", time: "7:30" },
            { id: "s4", label: "BadMinute", time: "05:60" },
            { label: "NoId", time: "05:00" },
            { id: "", label: "EmptyId", time: "05:00" },
            { id: "s7", label: "", time: "05:00" },
            { id: "s8", label: "x".repeat(41), time: "05:00" },
            "junk",
            { id: "s10", label: "Isha", time: "19:30" },
          ],
        }),
      ]),
    )[0];
    expect(first?.schedule).toEqual([
      { id: "s1", label: "Fajr", time: "05:00" },
      { id: "s10", label: "Isha", time: "19:30" },
    ]);
  });

  it("caps the schedule at MAX_SCHEDULE_SLOTS slots", () => {
    const schedule = Array.from({ length: MAX_SCHEDULE_SLOTS + 1 }, (_, i) => ({
      id: `s${i}`,
      label: `Slot ${i}`,
      time: "05:00",
    }));
    const first = decodeCards(envelope([row({ schedule })]))[0];
    expect(first?.schedule).toHaveLength(MAX_SCHEDULE_SLOTS);
  });

  it("omits the schedule when no slot survives", () => {
    const first = decodeCards(
      envelope([row({ schedule: [{ id: "s", label: "Fajr", time: "24:00" }] })]),
    )[0];
    expect(first?.schedule).toBeUndefined();
  });
});

describe("homeCards store — hydrate, mutate, persist", () => {
  // A fresh module instance per test: the store's hydrate() runs once per
  // instance, so tests re-import instead of sharing singleton state.
  let store: (typeof import("../cards.svelte.js"))["homeCards"];

  beforeEach(async () => {
    vi.resetModules();
    vi.useFakeTimers();
    window.localStorage.clear();
    window.localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify(
        envelope([
          row({ id: "card-1", count: 2 }),
          card({ id: "card-2", kind: "zikr", text: "Alhamdulillah" }),
        ]),
      ),
    );
    ({ homeCards: store } = await import("../cards.svelte.js"));
    store.hydrate();
  });

  afterEach(() => {
    // Fresh instances per test install their own cross-tab listener.
    store.dispose();
    vi.useRealTimers();
  });

  function flush(): void {
    vi.advanceTimersByTime(500);
  }

  function storedIds(): string[] {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    // SAFETY: the store only ever writes its own { v, cards } envelope under this key.
    const parsed = JSON.parse(raw ?? '{"cards":[]}') as { cards: Array<{ id: string }> };
    return parsed.cards.map((entry) => entry.id);
  }

  it("hydrates from localStorage", () => {
    expect(store.list).toHaveLength(2);
    expect(store.list[0]?.id).toBe("card-1");
  });

  it("increments a count and persists the tap", () => {
    store.increment("card-1");
    expect(store.list[0]?.count).toBe(3);
    flush();
    // SAFETY: the store only ever writes its own { v, cards } envelope under this key.
    const parsed = JSON.parse(window.localStorage.getItem(STORAGE_KEY) ?? '{"cards":[]}') as {
      cards: Array<{ count: number }>;
    };
    expect(parsed.cards[0]?.count).toBe(3);
  });

  it("resets, sets and clears a target, removes", () => {
    store.reset("card-1");
    store.setTarget("card-1", 33);
    expect(store.list[0]?.count).toBe(0);
    expect(store.list[0]?.target).toBe(33);
    store.setTarget("card-1", undefined);
    expect(store.list[0]?.target).toBeUndefined();
    store.remove("card-2");
    flush();
    expect(store.list.map((entry) => entry.id)).toEqual(["card-1"]);
    expect(storedIds()).toEqual(["card-1"]);
  });

  it("setSource re-pins a translation and preserves the tally", () => {
    const before = store.list[0]?.count;
    store.setSource("card-1", VALID_TRANSLATION_ID);
    // SAFETY: card-1 seeds as a surah card, which carries sourceId.
    const surah = store.list[0] as SurahCard;
    expect(surah.sourceId).toBe(VALID_TRANSLATION_ID);
    expect(surah.count).toBe(before);
    flush();
    // SAFETY: the store only ever writes its own { v, cards } envelope under this key.
    const parsed = JSON.parse(window.localStorage.getItem(STORAGE_KEY) ?? '{"cards":[]}') as {
      cards: Array<{ sourceId?: string | null }>;
    };
    expect(parsed.cards[0]?.sourceId).toBe(VALID_TRANSLATION_ID);
  });

  it("setSource degrades unknown ids to Arabic and tolerates zikr cards", () => {
    store.setSource("card-1", "xx.unknown");
    // SAFETY: card-1 seeds as a surah card, which carries sourceId.
    expect((store.list[0] as SurahCard).sourceId).toBeNull();
    // Zikr cards carry no sourceId — the call must be a silent no-op.
    expect(() => store.setSource("card-2", VALID_TRANSLATION_ID)).not.toThrow();
    expect(store.list[1]?.kind).toBe("zikr");
  });

  it("refuses to add past the cap", () => {
    store.remove("card-1");
    store.remove("card-2");
    for (let i = 0; i < MAX_CARDS; i += 1) {
      expect(store.add(card({ id: `bulk-${i}` }))).toBe(true);
    }
    expect(store.full).toBe(true);
    expect(store.add(card({ id: "over" }))).toBe(false);
  });

  /** Re-imports the store singleton against a custom seed (fresh instance). */
  async function reseed(rows: RawCardRow[]): Promise<void> {
    vi.resetModules();
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(envelope(rows)));
    ({ homeCards: store } = await import("../cards.svelte.js"));
    store.hydrate();
  }

  const SCHEDULE = [{ id: "s1", label: "Fajr", time: "05:00" }];

  function surahCard(entry: HomeCard | undefined): SurahCard | undefined {
    return entry?.kind === "surah" ? entry : undefined;
  }

  function collectionCard(entry: HomeCard | undefined): CollectionCard | undefined {
    return entry?.kind === "collection" ? entry : undefined;
  }

  it("records a recite event on every increment", () => {
    const now = Date.now();
    store.increment("card-1");
    expect(store.list[0]?.count).toBe(3);
    expect(store.list[0]?.events).toEqual([now]);
  });

  it("keeps accruing events on a capped card", async () => {
    await reseed([row({ id: "card-1", count: 1_000_000, events: [111] })]);
    const now = Date.now();
    store.increment("card-1");
    expect(store.list[0]?.count).toBe(1_000_000);
    expect(store.list[0]?.events).toEqual([111, now]);
    flush();
    // SAFETY: the store only ever writes its own { v, cards } envelope under this key.
    const parsed = JSON.parse(window.localStorage.getItem(STORAGE_KEY) ?? '{"cards":[]}') as {
      cards: Array<{ count: number; events: number[] }>;
    };
    expect(parsed.cards[0]?.count).toBe(1_000_000);
    expect(parsed.cards[0]?.events).toEqual([111, now]);
  });

  it("trims the event ring to EVENTS_CAP on increment", async () => {
    // Seeded exactly at the cap (decode's own trim is covered above); the store
    // must evict the OLDEST stamp to fit the new one.
    const events = Array.from({ length: EVENTS_CAP }, (_, i) => i + 1);
    await reseed([row({ id: "card-1", events })]);
    const now = Date.now();
    store.increment("card-1");
    expect(store.list[0]?.count).toBe(1);
    expect(store.list[0]?.events).toHaveLength(EVENTS_CAP);
    expect(store.list[0]?.events?.[0]).toBe(2);
    expect(store.list[0]?.events?.[EVENTS_CAP - 1]).toBe(now);
  });

  it("reset zeroes the tally but keeps the history", async () => {
    await reseed([row({ id: "card-1", count: 5, events: [111] })]);
    store.reset("card-1");
    expect(store.list[0]?.count).toBe(0);
    expect(store.list[0]?.events).toEqual([111]);
  });

  it("replace upserts by id and ignores unknown ids", async () => {
    await reseed([
      row({ id: "card-1", events: [111] }),
      card({ id: "card-2", kind: "zikr", text: "Alhamdulillah" }),
    ]);
    const edited = card({ id: "card-1", target: 33, schedule: SCHEDULE, events: [111] });
    store.replace(edited);
    expect(store.list[0]).toEqual(edited);
    store.replace(card({ id: "ghost" }));
    expect(store.list.map((entry) => entry.id)).toEqual(["card-1", "card-2"]);
    flush();
    // SAFETY: the store only ever writes its own { v, cards } envelope under this key.
    const parsed = JSON.parse(window.localStorage.getItem(STORAGE_KEY) ?? '{"cards":[]}') as {
      cards: Array<{ id: string; target?: number }>;
    };
    expect(parsed.cards[0]?.id).toBe("card-1");
    expect(parsed.cards[0]?.target).toBe(33);
  });

  it("replace retargets a collection's items", async () => {
    await reseed([
      row({
        id: "col",
        kind: "collection",
        surah: undefined,
        sourceId: undefined,
        items: [{ surah: 2 }],
      }),
    ]);
    const edited = card({
      id: "col",
      kind: "collection",
      surah: undefined,
      sourceId: undefined,
      items: [{ surah: 18, ayahStart: 1, ayahEnd: 10 }, { slug: KNOWN_SLUG }],
    });
    store.replace(edited);
    expect(collectionCard(store.list[0])?.items).toEqual([
      { surah: 18, ayahStart: 1, ayahEnd: 10, sourceId: null },
      { slug: KNOWN_SLUG, sourceId: null },
    ]);
  });

  it("duplicate deep-copies the card under a fresh id", async () => {
    await reseed([row({ id: "card-1", target: 33, events: [111], schedule: SCHEDULE })]);
    expect(store.duplicate("card-1")).toBe(true);
    expect(store.list).toHaveLength(2);
    const original = surahCard(store.list[0]);
    const copy = surahCard(store.list[1]);
    expect(copy?.id).not.toBe("card-1");
    expect(copy?.kind).toBe("surah");
    expect(copy?.surah).toBe(2);
    expect(copy?.count).toBe(0);
    expect(copy?.events).toBeUndefined();
    expect(copy?.target).toBe(33);
    expect(copy?.schedule).toEqual(SCHEDULE);
    expect(copy?.schedule?.[0]).not.toBe(original?.schedule?.[0]);
    flush();
    expect(storedIds()).toHaveLength(2);
  });

  it("duplicate shares no item objects for collections", async () => {
    await reseed([
      row({
        id: "col",
        kind: "collection",
        surah: undefined,
        sourceId: undefined,
        items: [{ surah: 2, sourceId: VALID_TRANSLATION_ID }],
      }),
    ]);
    expect(store.duplicate("col")).toBe(true);
    const original = collectionCard(store.list[0]);
    const copy = collectionCard(store.list[1]);
    expect(copy?.id).not.toBe("col");
    expect(copy?.items[0]).toEqual(original?.items[0]);
    expect(copy?.items[0]).not.toBe(original?.items[0]);
  });

  it("duplicate refuses when the store is full or the id is unknown", async () => {
    store.remove("card-1");
    store.remove("card-2");
    for (let i = 0; i < MAX_CARDS; i += 1) {
      store.add(card({ id: `bulk-${i}` }));
    }
    expect(store.duplicate("bulk-0")).toBe(false);
    expect(store.list).toHaveLength(MAX_CARDS);
    expect(store.duplicate("ghost")).toBe(false);
  });

  it("reconciles a foreign-tab write — add, tally and removal land", () => {
    window.localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify(
        envelope([
          {
            id: "z1",
            kind: "zikr",
            text: "SubhanAllah",
            count: 4,
            createdAt: "2026-10-27T00:00:00.000Z",
          },
        ]),
      ),
    );
    window.dispatchEvent(new StorageEvent("storage", { key: STORAGE_KEY }));
    expect(store.list.map((entry) => entry.id)).toEqual(["z1"]);
    expect(store.list[0]?.count).toBe(4);
  });

  it("ignores storage events for other keys", () => {
    window.dispatchEvent(new StorageEvent("storage", { key: "easyquran.prefs" }));
    expect(store.list.map((entry) => entry.id)).toEqual(["card-1", "card-2"]);
  });

  it("dispose detaches the cross-tab listener", () => {
    store.dispose();
    window.localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify(
        envelope([
          {
            id: "z9",
            kind: "zikr",
            text: "AllahuAkbar",
            count: 0,
            createdAt: "2026-10-27T00:00:00.000Z",
          },
        ]),
      ),
    );
    window.dispatchEvent(new StorageEvent("storage", { key: STORAGE_KEY }));
    expect(store.list.map((entry) => entry.id)).toEqual(["card-1", "card-2"]);
  });
});
