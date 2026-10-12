import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

vi.mock("$app/env", () => ({ browser: true }));
vi.mock("$app/env/public", () => ({ PUBLIC_API_BASE_URL: "/api", PUBLIC_FCM_VAPID_KEY: "" }));

// Named contract for the authState mock: the real authState is a class with
// private fields, so the mock stands in structurally with just these members.
type MockAuthState = { status: string; user: { id: number } | null };

const auth = vi.hoisted((): MockAuthState => {
  const user: { id: number } | null = null;
  return { status: "anonymous", user };
});
vi.mock("#lib/auth/auth-state.svelte.js", () => ({ authState: auth }));

const transport = vi.hoisted(() => ({
  unsafeRequest: vi.fn(),
  get: vi.fn(),
}));
vi.mock("#lib/auth/auth-client.js", async (importOriginal) => {
  const actual = await importOriginal<typeof import("#lib/auth/auth-client.js")>();
  return { ...actual, authClient: transport };
});

import { todayLocal } from "#lib/quran/engagement-state.js";
import { consent, decodeConsent } from "#lib/stores/consent.svelte.js";
import { scrubReaderPath } from "#lib/firebase/analytics.js";
import {
  decodeLocalRollup,
  decodeSummary,
  readingAnalytics,
  summarizeRollup,
} from "../reading-analytics";

const ROLLUP_KEY = "easyquran.analytics.local";
const QUEUE_KEY = "easyquran.analytics.queue.7";

// SAFETY: readKey parses whatever is under the key; tests only ever write valid envelopes.
const readKey = (key: string) => JSON.parse(window.localStorage.getItem(key) ?? "null");

describe("reading analytics consent gate (§11.4: opt-in, OFF by default)", () => {
  it("decodes readingAnalytics as explicit-true only", () => {
    expect(decodeConsent(undefined).readingAnalytics).toBe(false);
    expect(decodeConsent({ analytics: true }).readingAnalytics).toBe(false);
    expect(decodeConsent({ readingAnalytics: true }).readingAnalytics).toBe(true);
  });

  it("emit writes nothing while opted out, regardless of auth", () => {
    auth.status = "authenticated";
    auth.user = { id: 7 };
    readingAnalytics.emit({
      kind: "ayah.read",
      surah: 2,
      ayah: 255,
      sourceId: null,
      seconds: null,
    });
    expect(readKey(ROLLUP_KEY)).toBeNull();
    expect(readKey(QUEUE_KEY)).toBeNull();
    auth.status = "anonymous";
    auth.user = null;
  });
});

describe("reading analytics anonymous local rollup", () => {
  beforeEach(() => {
    window.localStorage.clear();
    consent.setReadingAnalytics(true);
  });

  it("counts per day/surah/ayah and re-reads increment", () => {
    readingAnalytics.emit({
      kind: "ayah.read",
      surah: 2,
      ayah: 255,
      sourceId: "en.sahih",
      seconds: null,
    });
    readingAnalytics.emit({
      kind: "ayah.read",
      surah: 2,
      ayah: 255,
      sourceId: "en.sahih",
      seconds: null,
    });
    readingAnalytics.emit({ kind: "ayah.read", surah: 3, ayah: 1, sourceId: null, seconds: null });
    const day = todayLocal();
    expect(readKey(ROLLUP_KEY)).toEqual({
      v: 1,
      days: { [day]: { "2": { "255": 2 }, "3": { "1": 1 } } },
    });
    expect(summarizeRollup(decodeLocalRollup(readKey(ROLLUP_KEY))).topSurahs).toEqual([
      { surah: 2, reads: 2 },
      { surah: 3, reads: 1 },
    ]);
  });

  it("rejects out-of-range verses and bad source ids without writing", () => {
    readingAnalytics.emit({ kind: "ayah.read", surah: 0, ayah: 1, sourceId: null, seconds: null });
    readingAnalytics.emit({
      kind: "ayah.read",
      surah: 1,
      ayah: 999,
      sourceId: null,
      seconds: null,
    });
    readingAnalytics.emit({
      kind: "ayah.read",
      surah: 1,
      ayah: 1,
      sourceId: "BAD ID",
      seconds: null,
    });
    expect(readKey(ROLLUP_KEY)).toBeNull();
  });
});

describe("reading analytics signed-in queue + flush", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    window.localStorage.clear();
    consent.setReadingAnalytics(true);
    auth.status = "authenticated";
    auth.user = { id: 7 };
    transport.unsafeRequest.mockReset();
  });
  afterEach(() => vi.useRealTimers());

  it("queues per-owner and flushes a camelCase batch with expectedUserId", async () => {
    readingAnalytics.emit({
      kind: "ayah.read",
      surah: 2,
      ayah: 255,
      sourceId: "en.sahih",
      seconds: 12,
    });
    transport.unsafeRequest.mockResolvedValue({
      ok: true,
      status: 200,
      data: { accepted: 1 },
      error: null,
      rotated: false,
    });
    await vi.advanceTimersByTimeAsync(4_100);
    expect(transport.unsafeRequest).toHaveBeenCalledWith("/user/analytics/v1/events", {
      method: "POST",
      body: {
        expectedUserId: 7,
        events: [
          expect.objectContaining({
            kind: "ayah.read",
            surah: 2,
            ayah: 255,
            sourceId: "en.sahih",
            seconds: 12,
            localDay: todayLocal(),
          }),
        ],
      },
    });
    expect(readKey(QUEUE_KEY)).toBeNull();
  });

  it("keeps the queue on network failure, drops a 400 batch (EC-29 rule)", async () => {
    readingAnalytics.emit({ kind: "ayah.read", surah: 2, ayah: 1, sourceId: null, seconds: null });
    transport.unsafeRequest.mockRejectedValueOnce(new Error("offline"));
    await vi.advanceTimersByTimeAsync(4_100);
    expect(readKey(QUEUE_KEY)).not.toBeNull();
    transport.unsafeRequest.mockResolvedValue({
      ok: false,
      status: 400,
      data: null,
      error: {},
      rotated: false,
    });
    await readingAnalytics.flushNow();
    expect(readKey(QUEUE_KEY)).toBeNull();
  });

  it("opting out drops the queued events and the local rollup", () => {
    readingAnalytics.emit({ kind: "ayah.read", surah: 2, ayah: 1, sourceId: null, seconds: null });
    expect(readKey(QUEUE_KEY)).not.toBeNull();
    consent.setReadingAnalytics(false);
    expect(readKey(QUEUE_KEY)).toBeNull();
    expect(readKey(ROLLUP_KEY)).toBeNull();
    auth.status = "anonymous";
    auth.user = null;
  });

  it("a signed-out opt-out also drops another owner's queued batch (device-global revocation)", () => {
    window.localStorage.setItem(
      "easyquran.analytics.queue.42",
      JSON.stringify({ v: 1, events: [] }),
    );
    window.localStorage.setItem("easyquran.bookmarks", JSON.stringify({ untouched: true }));
    auth.status = "anonymous";
    auth.user = null;
    consent.setReadingAnalytics(false);
    expect(window.localStorage.getItem("easyquran.analytics.queue.42")).toBeNull();
    expect(window.localStorage.getItem("easyquran.bookmarks")).not.toBeNull();
  });

  it("a cross-tab opt-out storage event drops this tab's queue off the stored flag", () => {
    readingAnalytics.emit({ kind: "ayah.read", surah: 2, ayah: 1, sourceId: null, seconds: null });
    expect(readKey(QUEUE_KEY)).not.toBeNull();
    // Other tab writes the shared consent key; this tab's in-memory flag stays
    // stale-true, so the drop must decode the stored value, not read `consent`.
    window.localStorage.setItem(
      "easyquran.consent",
      JSON.stringify({
        analytics: true,
        performance: true,
        advertising: false,
        readingAnalytics: false,
      }),
    );
    window.dispatchEvent(new StorageEvent("storage", { key: "easyquran.consent" }));
    expect(readKey(QUEUE_KEY)).toBeNull();
    auth.status = "anonymous";
    auth.user = null;
  });
});

describe("reading analytics summary + GA4 scrub", () => {
  it("decodes the server summary shape", () => {
    expect(
      decodeSummary({
        totals: { events: 3, seconds: 30 },
        topSurahs: [{ surah: 2, reads: 3 }],
        topAyahs: [{ surah: 2, ayah: 255, reads: 2 }],
        days: [{ localDay: "2026-10-12", ayahsRead: 3, seconds: 30 }],
      }),
    ).toEqual({
      totals: { events: 3, seconds: 30 },
      topSurahs: [{ surah: 2, reads: 3 }],
      topAyahs: [{ surah: 2, ayah: 255, reads: 2 }],
      days: [{ localDay: "2026-10-12", ayahsRead: 3, seconds: 30 }],
    });
  });

  it("GA4 never sees surah paths (SCR-05): reader surfaces report as /reader", () => {
    for (const path of [
      "/al-baqarah",
      "/page/5",
      "/juz/30",
      "/al-baqarah/t/en/sahih",
      "/t/en/sahih/page/5",
      "/t/en/sahih/azkar/ayat-al-kursi",
      "/azkar/ayat-al-kursi",
      "/ar/al-baqarah",
    ]) {
      expect(scrubReaderPath(path)).toBe("/reader");
    }
    for (const path of ["/", "/bookmarks", "/settings", "/yours", "/ar/settings", "/search"]) {
      expect(scrubReaderPath(path)).toBe(path);
    }
  });
});
