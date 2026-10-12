import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

vi.mock("$app/env", () => ({ browser: true, dev: false, building: false }));
vi.mock("$app/env/public", () => ({
  PUBLIC_API_BASE_URL: "/api",
  PUBLIC_FCM_VAPID_KEY: "",
  PUBLIC_QURAN_API_BASE: "",
  PUBLIC_ENV: "local",
}));

import { decodeCards, MAX_CARDS, type HomeCard } from "../cards.svelte.js";

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

  it("refuses to add past the cap", () => {
    store.remove("card-1");
    store.remove("card-2");
    for (let i = 0; i < MAX_CARDS; i += 1) {
      expect(store.add(card({ id: `bulk-${i}` }))).toBe(true);
    }
    expect(store.full).toBe(true);
    expect(store.add(card({ id: "over" }))).toBe(false);
  });
});
