import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

vi.mock("$app/env", () => ({ browser: true, dev: false, building: false }));
vi.mock("$app/env/public", () => ({
  PUBLIC_API_BASE_URL: "/api",
  PUBLIC_FCM_VAPID_KEY: "",
  PUBLIC_QURAN_API_BASE: "",
  PUBLIC_ENV: "local",
}));

import { cardEntryAttrs, tileCopyFor } from "#lib/home/card-copy.js";
import { QURAN_DATA } from "#lib/server/quran-data.js";
import { decodeCards, MAX_CARDS, type HomeCard, type SnippetCard } from "../cards.svelte.js";
import { boundCardFor, isTargetDone, nextZikrCard } from "../next-zikr.js";
import type { ReadingThread } from "#lib/sessions/schema.js";

const VALID_TRANSLATION_ID = "en.sahih";
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
    createdAt: "2026-10-27T00:00:00.000Z",
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

const snippet = (id: string, slug: string, extra: Partial<RawCardRow> = {}): HomeCard =>
  card({ id, kind: "snippet", slug, sourceId: null, ...extra });

/** A full ReadingThread literal — every field explicit, no casts. */
function thread(overrides: Partial<ReadingThread> = {}): ReadingThread {
  return {
    id: "0123456789abcdef0123456789abcdef",
    origin: "card",
    originRef: "card-1",
    scope: "snippet:ayat-al-kursi",
    boundEnd: null,
    surah: 2,
    ayah: 255,
    localPage: null,
    ratio: null,
    sourceId: null,
    device: "phone",
    startedAt: "2026-10-27T00:00:00.000Z",
    updatedAt: "2026-10-27T00:00:00.000Z",
    endedAt: null,
    endReason: null,
    ...overrides,
  };
}

describe("isTargetDone", () => {
  it("is done only with a target at or below the count", () => {
    expect(isTargetDone(snippet("a", "al-ikhlas", { count: 3, target: 3 }))).toBe(true);
    expect(isTargetDone(snippet("a", "al-ikhlas", { count: 4, target: 3 }))).toBe(true);
    expect(isTargetDone(snippet("a", "al-ikhlas", { count: 2, target: 3 }))).toBe(false);
    expect(isTargetDone(snippet("a", "al-ikhlas", { count: 100 }))).toBe(false);
  });
});

describe("boundCardFor — card-origin binding", () => {
  it("returns the thread's originRef card, not the first slug match", () => {
    const cards = [
      snippet("card-a", "ayat-al-kursi", { count: 2, target: 10 }),
      snippet("card-b", "ayat-al-kursi", { count: 7, target: 10 }),
    ];
    const bound = boundCardFor(
      thread({ originRef: "card-b", scope: "snippet:ayat-al-kursi" }),
      cards,
      "ayat-al-kursi",
    );
    expect(bound?.id).toBe("card-b");
    expect(bound?.count).toBe(7);
  });

  it("ignores a null thread, a non-card origin and a null originRef", () => {
    const cards = [snippet("card-a", "ayat-al-kursi")];
    expect(boundCardFor(null, cards, "ayat-al-kursi")).toBeNull();
    expect(
      boundCardFor(thread({ origin: "search", originRef: "card-a" }), cards, "ayat-al-kursi"),
    ).toBeNull();
    expect(boundCardFor(thread({ originRef: null }), cards, "ayat-al-kursi")).toBeNull();
  });

  it("ignores a card-origin thread scoped to another page", () => {
    const cards = [snippet("card-a", "ayat-al-kursi")];
    expect(
      boundCardFor(
        thread({ originRef: "card-a", scope: "snippet:al-ikhlas" }),
        cards,
        "ayat-al-kursi",
      ),
    ).toBeNull();
  });

  it("ignores an originRef that resolves to another slug or a non-snippet card", () => {
    const cards = [
      card({ id: "card-s", kind: "surah", surah: 114 }),
      snippet("card-x", "al-ikhlas"),
    ];
    expect(
      boundCardFor(
        thread({ originRef: "card-s", scope: "snippet:ayat-al-kursi" }),
        cards,
        "ayat-al-kursi",
      ),
    ).toBeNull();
    expect(
      boundCardFor(
        thread({ originRef: "card-x", scope: "snippet:ayat-al-kursi" }),
        cards,
        "ayat-al-kursi",
      ),
    ).toBeNull();
  });

  it("returns null when the originRef card no longer exists (cross-tab removal)", () => {
    expect(
      boundCardFor(
        thread({ originRef: "card-gone" }),
        [snippet("card-a", "ayat-al-kursi")],
        "ayat-al-kursi",
      ),
    ).toBeNull();
  });

  it("still binds an ended thread — the thread keeps naming the right card", () => {
    const cards = [snippet("card-a", "ayat-al-kursi")];
    const ended = thread({
      originRef: "card-a",
      endedAt: "2026-10-27T01:00:00.000Z",
      endReason: "completed",
    });
    expect(boundCardFor(ended, cards, "ayat-al-kursi")?.id).toBe("card-a");
  });
});

describe("nextZikrCard — the pick rule", () => {
  it("returns the card after the current one in store list order", () => {
    const cards = [
      snippet("a", "al-ikhlas"),
      snippet("b", "baqarah-last-two"),
      snippet("c", "al-muawwidhatayn"),
    ];
    expect(nextZikrCard(cards, { id: "a", slug: "al-ikhlas" })?.id).toBe("b");
  });

  it("skips target-reached cards between the current one and the next candidate", () => {
    const cards = [
      snippet("a", "al-ikhlas"),
      snippet("b", "baqarah-last-two", { count: 10, target: 10 }),
      snippet("c", "al-muawwidhatayn"),
    ];
    expect(nextZikrCard(cards, { id: "a", slug: "al-ikhlas" })?.id).toBe("c");
  });

  it("wraps to the first eligible card when the current card is last", () => {
    const cards = [snippet("a", "al-ikhlas"), snippet("b", "baqarah-last-two")];
    expect(nextZikrCard(cards, { id: "b", slug: "baqarah-last-two" })?.id).toBe("a");
  });

  it("returns null when the current card is the only card", () => {
    expect(nextZikrCard([snippet("a", "al-ikhlas")], { id: "a", slug: "al-ikhlas" })).toBeNull();
  });

  it("returns null when every other card is target-reached, mid-list gap included", () => {
    const done = { count: 5, target: 5 };
    const cards = [
      snippet("a", "al-ikhlas"),
      snippet("b", "baqarah-last-two", done),
      snippet("c", "al-muawwidhatayn", done),
    ];
    expect(nextZikrCard(cards, { id: "a", slug: "al-ikhlas" })).toBeNull();
    // The gap mid-list: the done card between two others blocks nothing —
    // the open card after it is still the pick.
    expect(
      nextZikrCard(
        [
          snippet("a", "al-ikhlas"),
          snippet("b", "baqarah-last-two", done),
          snippet("c", "al-muawwidhatayn"),
        ],
        { id: "a", slug: "al-ikhlas" },
      )?.id,
    ).toBe("c");
  });

  it("never returns the current card itself", () => {
    const cards = [
      snippet("a", "al-ikhlas"),
      snippet("b", "baqarah-last-two", { count: 1, target: 1 }),
    ];
    expect(nextZikrCard(cards, { id: "a", slug: "al-ikhlas" })).toBeNull();
  });

  it("excludes same-slug snippet cards — navigating would reload this page", () => {
    const cards = [
      snippet("a", "al-ikhlas"),
      snippet("b", "al-ikhlas"),
      snippet("c", "baqarah-last-two"),
    ];
    expect(nextZikrCard(cards, { id: "a", slug: "al-ikhlas" })?.id).toBe("c");
    expect(
      nextZikrCard([snippet("a", "al-ikhlas"), snippet("b", "al-ikhlas")], {
        id: "a",
        slug: "al-ikhlas",
      }),
    ).toBeNull();
  });

  it("excludes zikr phrase cards (no reader destination)", () => {
    const cards = [snippet("a", "al-ikhlas"), card({ id: "z", kind: "zikr", text: "Subhanallah" })];
    expect(nextZikrCard(cards, { id: "a", slug: "al-ikhlas" })).toBeNull();
  });

  it("accepts every card kind as a candidate — a surah card is returned", () => {
    const nas = card({ id: "nas", kind: "surah", surah: 114 });
    const cards = [snippet("a", "al-ikhlas"), nas];
    expect(nextZikrCard(cards, { id: "a", slug: "al-ikhlas" })?.id).toBe("nas");
  });

  it("prefers a schedule-due card over an earlier not-due one", () => {
    const nowMs = Date.parse("2026-10-27T10:00:00Z");
    const at = new Date(nowMs + 10 * 60_000); // inside the ±30-minute due window
    const dueTime = `${String(at.getHours()).padStart(2, "0")}:${String(at.getMinutes()).padStart(2, "0")}`;
    const cards = [
      snippet("a", "al-ikhlas"),
      snippet("b", "baqarah-last-two"),
      snippet("c", "al-muawwidhatayn", {
        schedule: [{ id: "s1", label: "Fajr", time: dueTime }],
      }),
    ];
    expect(nextZikrCard(cards, { id: "a", slug: "al-ikhlas" }, nowMs)?.id).toBe("c");
  });

  it("returns null for an unknown current card id when no other card survives", () => {
    expect(nextZikrCard([snippet("a", "al-ikhlas")], { id: "gone", slug: "al-ikhlas" })).toBeNull();
  });
});

describe("next-zikr navigation — guarded builders and entry attrs", () => {
  // Real catalog data (the quran.test.ts precedent): the surah slug and name
  // tileCopyFor renders must be the shipped ones.
  const quranData = QURAN_DATA;

  it("snippet href is /azkar/{slug} for the mushaf and /t/** when pinned", () => {
    const arabic = tileCopyFor(snippet("n", "al-ikhlas"), undefined, "en");
    expect(arabic.href).toBe("/azkar/al-ikhlas");
    const pinned = tileCopyFor(
      snippet("n", "al-ikhlas", { sourceId: VALID_TRANSLATION_ID }),
      undefined,
      "en",
    );
    expect(pinned.href).toBe("/t/en/sahih/azkar/al-ikhlas");
  });

  it("degrades a stale pinned sourceId to the Arabic snippet path", () => {
    const stale = tileCopyFor(snippet("n", "al-ikhlas", { sourceId: "xx.gone" }), undefined, "en");
    expect(stale.href).toBe("/azkar/al-ikhlas");
  });

  it("surah href is the surahPathFor form for the pinned sourceId", () => {
    const view = tileCopyFor(
      card({ id: "nas", kind: "surah", surah: 114, sourceId: VALID_TRANSLATION_ID }),
      quranData,
      "en",
    );
    expect(view.href).toBe("/an-nas/t/en/sahih");
  });

  it("the CTA's entry attrs tag origin=card with the next card's ref and scope", () => {
    expect(cardEntryAttrs(snippet("n", "al-ikhlas"))).toEqual({
      "data-session-origin": "card",
      "data-session-ref": "n",
      "data-session-scope": "snippet:al-ikhlas",
    });
    expect(cardEntryAttrs(card({ id: "nas", kind: "surah", surah: 114 }))).toEqual({
      "data-session-origin": "card",
      "data-session-ref": "nas",
      "data-session-scope": "surah:114",
    });
  });
});

describe("inline add — snippet card lands in the envelope", () => {
  // Mirrors the azkar page's AddCardInline submit: build a SnippetCard, call
  // homeCards.add, flush the 400ms persist debounce, read the envelope back.
  let store: (typeof import("../cards.svelte.js"))["homeCards"];

  beforeEach(async () => {
    vi.resetModules();
    vi.useFakeTimers();
    window.localStorage.clear();
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

  /** The exact card shape AddCardInline builds on submit (id varies per row). */
  function inlineCard(id = "inline-1", target?: number): SnippetCard {
    return {
      id,
      kind: "snippet",
      slug: "al-ikhlas",
      sourceId: null,
      count: 0,
      target,
      createdAt: "2026-10-27T12:00:00.000Z",
    };
  }

  /** One persisted card row, the fields the inline-add assertions read. */
  interface StoredRow {
    id: string;
    kind: string;
    slug: string;
    count: number;
    sourceId?: string | null;
    target?: number;
  }

  function storedRows(): StoredRow[] {
    // SAFETY: the store only ever writes its own { v, cards } envelope under this key.
    const parsed = JSON.parse(window.localStorage.getItem(STORAGE_KEY) ?? '{"cards":[]}') as {
      cards: StoredRow[];
    };
    return parsed.cards;
  }

  it("persists { slug, sourceId, count: 0 } after the debounce flush", () => {
    expect(store.add(inlineCard())).toBe(true);
    flush();
    const stored = storedRows()[0];
    expect(stored).toMatchObject({
      id: "inline-1",
      kind: "snippet",
      slug: "al-ikhlas",
      sourceId: null,
      count: 0,
    });
    expect(stored).not.toHaveProperty("target");
  });

  it("persists the target at both bounds (1 and 10_000)", () => {
    store.add(inlineCard("lo", 1));
    store.add(inlineCard("hi", 10_000));
    flush();
    const byId = new Map(storedRows().map((entry) => [entry.id, entry.target]));
    expect(byId.get("lo")).toBe(1);
    expect(byId.get("hi")).toBe(10_000);
  });

  it("never persists an out-of-bounds target — decode drops it on read-back", () => {
    // The stepper clamps before add(); the envelope decoder is the second gate:
    // 0, 10_001 and non-numeric targets cannot survive a round trip.
    for (const target of [0, 10_001, Number.NaN]) {
      const roundTrip = decodeCards(
        envelope([row({ id: "t", kind: "snippet", slug: "al-ikhlas", target })]),
      );
      expect(roundTrip[0]?.target).toBeUndefined();
    }
  });

  it("refuses at MAX_CARDS and persists nothing", () => {
    for (let i = 0; i < MAX_CARDS; i += 1) {
      expect(store.add(inlineCard(`bulk-${i}`))).toBe(true);
    }
    expect(store.full).toBe(true);
    expect(store.add(inlineCard())).toBe(false);
    flush();
    expect(storedRows()).toHaveLength(MAX_CARDS);
  });
});
