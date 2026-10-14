import { describe, expect, it, vi } from "vite-plus/test";

vi.mock("$app/env", () => ({ browser: true }));
vi.mock("$app/env/public", () => ({
  PUBLIC_API_BASE_URL: undefined,
  PUBLIC_QURAN_API_BASE: undefined,
  PUBLIC_ENV: undefined,
  PUBLIC_FCM_VAPID_KEY: undefined,
}));

import {
  IDLE_MS,
  MIN_DWELL_MS,
  ReadTracker,
  arabicLetterCount,
  dwellFloorMs,
} from "../read-tracker";

/** A tracker on a hand-driven clock; every verse is `letters` long. */
function rig(letters = 50) {
  let now = 1_000_000;
  const reads: Array<{ surah: number; ayah: number; ms: number }> = [];
  const noted: string[] = [];
  const tracker = new ReadTracker({
    now: () => now,
    lettersFor: () => letters,
    onRead: (surah, ayah) => {
      noted.push(`${surah}:${ayah}`);
      return { sessionId: "s", sourceId: null };
    },
    onEmit: (event) => reads.push({ surah: event.surah, ayah: event.ayah, ms: event.ms }),
  });
  const els = new Map<string, Element>();
  const el = (key: string): Element => {
    let found = els.get(key);
    if (found === undefined) {
      found = document.createElement("span");
      els.set(key, found);
      tracker.track(found, key);
    }
    return found;
  };
  return {
    tracker,
    reads,
    noted,
    advance: (ms: number) => {
      now += ms;
    },
    show: (key: string) => tracker.intersect(el(key), true, false),
    scrollPast: (key: string) => tracker.intersect(el(key), false, true),
    scrollBack: (key: string) => tracker.intersect(el(key), false, false),
  };
}

describe("dwell floor (hafiz-aware)", () => {
  it("is 10 ms per letter with a 150 ms floor — far below the fastest recitation", () => {
    expect(dwellFloorMs(5)).toBe(MIN_DWELL_MS);
    expect(dwellFloorMs(56)).toBe(560);
    // Ayat al-Kursi is ~ 230 letters: 2.3 s still counts a very fast hafiz.
    expect(dwellFloorMs(230)).toBe(2300);
  });

  it("counts Arabic letters only (no diacritics, marks or digits)", () => {
    expect(arabicLetterCount("بِسْمِ ٱللَّهِ ١")).toBe(7);
  });
});

describe("ReadTracker", () => {
  it("a verse scrolled past after enough dwell counts; a fling does not", () => {
    const r = rig(50);
    r.show("2:1");
    r.advance(600);
    r.scrollPast("2:1");
    r.show("2:2");
    r.advance(40);
    r.scrollPast("2:2");
    r.tracker.flushWindow();
    expect(r.noted).toEqual(["2:1"]);
    expect(r.reads.map((e) => `${e.surah}:${e.ayah}`)).toEqual(["2:1"]);
  });

  it("a fast hafiz pace (well under natural reading time) still counts every verse", () => {
    const r = rig(40);
    for (let ayah = 1; ayah <= 20; ayah += 1) {
      r.show(`36:${ayah}`);
      r.advance(450); // 40 letters in 450 ms ≈ 18 words/s
      r.tracker.interaction();
      r.scrollPast(`36:${ayah}`);
    }
    r.tracker.flushWindow();
    expect(r.reads).toHaveLength(20);
  });

  it("scrolling back (bottom exit) never counts", () => {
    const r = rig(10);
    r.show("1:1");
    r.advance(5000);
    r.scrollBack("1:1");
    r.tracker.flushWindow();
    expect(r.reads).toEqual([]);
  });

  it("verses still on screen count when the reader moves on", () => {
    const r = rig(10);
    r.show("112:1");
    r.show("112:2");
    r.advance(3000);
    r.tracker.flushVisible();
    r.tracker.flushWindow();
    expect(r.noted.sort()).toEqual(["112:1", "112:2"]);
  });

  it("the same verse counts once per repeat gap, so edge jitter is not a re-read", () => {
    const r = rig(10);
    for (let i = 0; i < 3; i += 1) {
      r.show("1:1");
      r.advance(400);
      r.scrollPast("1:1");
    }
    r.tracker.flushWindow();
    expect(r.reads).toHaveLength(1);
    r.advance(31_000);
    r.show("1:1");
    r.advance(400);
    r.scrollPast("1:1");
    r.tracker.flushWindow();
    expect(r.reads).toHaveLength(2);
  });

  it("splits the window's active time across verses by letters and stops at idle", () => {
    const r = rig(10);
    r.show("2:255");
    r.advance(4000);
    r.tracker.interaction();
    r.scrollPast("2:255");
    r.tracker.flushWindow();
    expect(r.reads[0]?.ms).toBeGreaterThan(3000);
    expect(r.reads[0]?.ms).toBeLessThanOrEqual(4000);

    // An idle reader stops accruing: a verse left on screen far past IDLE_MS
    // gets at most the idle allowance.
    r.show("2:256");
    r.advance(IDLE_MS * 3);
    r.scrollPast("2:256");
    r.tracker.flushWindow();
    expect(r.reads[1]?.ms).toBeLessThanOrEqual(IDLE_MS);
  });

  it("a hidden tab pauses dwell; visible markers resume when it returns", () => {
    const r = rig(100);
    r.show("18:1");
    r.advance(200);
    r.tracker.setVisible(false);
    r.advance(60_000);
    r.tracker.setVisible(true);
    r.advance(900);
    r.scrollPast("18:1");
    r.tracker.flushWindow();
    // 200 + 900 ms ≥ the 1000 ms floor; the hidden minute never counted.
    expect(r.reads).toHaveLength(1);
    expect(r.reads[0]?.ms).toBeLessThan(5000);
  });
});
