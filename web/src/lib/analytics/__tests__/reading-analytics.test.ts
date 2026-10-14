import { beforeEach, describe, expect, it, vi } from "vite-plus/test";

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

import { scrubReaderPath } from "#lib/firebase/analytics.js";
import { todayLocal } from "#lib/quran/engagement-state.js";
import {
  decodeLocalRollup,
  decodeSummary,
  readingAnalytics,
  summarizeRollup,
} from "../reading-analytics";

const ROLLUP_KEY = "easyquran.analytics.local";
const LOCAL_QUEUE = "easyquran.analytics.queue.local";
const ACCOUNT_QUEUE = "easyquran.analytics.queue.7";

// SAFETY: readKey parses whatever is under the key; tests only ever write valid envelopes.
const readKey = (key: string) => JSON.parse(window.localStorage.getItem(key) ?? "null");

const read = (surah: number, ayah: number, ms = 1000) => ({
  sessionId: "c1c1c1c1-c1c1-4c1c-8c1c-c1c1c1c1c1c1",
  surah,
  ayah,
  sourceId: null,
  ms,
});

function signIn(): void {
  auth.status = "authenticated";
  auth.user = { id: 7 };
}

function signOut(): void {
  auth.status = "anonymous";
  auth.user = null;
}

beforeEach(() => {
  window.localStorage.clear();
  transport.unsafeRequest.mockReset();
  transport.get.mockReset();
  signOut();
});

describe("reading insights are always on", () => {
  it("signed out: every read queues and rolls up locally with its time", () => {
    readingAnalytics.emit(read(2, 255, 4000));
    readingAnalytics.emit(read(2, 255, 2000));
    readingAnalytics.emit(read(1, 1, 500));
    expect(readKey(LOCAL_QUEUE).events).toHaveLength(3);
    const summary = summarizeRollup(decodeLocalRollup(readKey(ROLLUP_KEY)));
    expect(summary.totals).toEqual({ ayahs: 3, ms: 6500, days: 1 });
    expect(summary.topAyahs[0]).toEqual({ surah: 2, ayah: 255, reads: 2, ms: 6000 });
    expect(summary.days[0]).toEqual({ localDay: todayLocal(), ayahsRead: 3, ms: 6500 });
  });

  it("drops out-of-range verses and clamps durations", () => {
    readingAnalytics.emit(read(115, 1));
    readingAnalytics.emit(read(1, 0));
    readingAnalytics.emit(read(1, 1, 99_999_999));
    const events = readKey(LOCAL_QUEUE).events;
    expect(events).toHaveLength(1);
    expect(events[0].ms).toBe(3_600_000);
    expect(events[0].id).toMatch(/^[0-9a-f-]{36}$/u);
  });

  it("signed in: the device queue joins the account and uploads in one idempotent batch", async () => {
    readingAnalytics.emit(read(36, 1));
    signIn();
    transport.unsafeRequest.mockResolvedValue({ ok: true, status: 200, data: { accepted: 2 } });
    readingAnalytics.emit(read(36, 2));
    await readingAnalytics.flushNow();
    expect(transport.unsafeRequest).toHaveBeenCalledTimes(1);
    const [path, init] = transport.unsafeRequest.mock.calls[0] ?? [];
    expect(path).toBe("/user/analytics/v1/events");
    expect(init.body.expectedUserId).toBe(7);
    expect(init.body.events.map((e: { ayah: number }) => e.ayah).sort()).toEqual([1, 2]);
    expect(readKey(LOCAL_QUEUE)).toBeNull();
    expect(readKey(ROLLUP_KEY)).toBeNull();
    expect(readKey(ACCOUNT_QUEUE)).toBeNull();
  });

  it("keeps the queue on network failure, drops a rejected (400) batch", async () => {
    signIn();
    readingAnalytics.emit(read(2, 1));
    transport.unsafeRequest.mockRejectedValueOnce(new Error("offline"));
    await readingAnalytics.flushNow();
    expect(readKey(ACCOUNT_QUEUE).events).toHaveLength(1);
    transport.unsafeRequest.mockResolvedValueOnce({ ok: false, status: 400 });
    await readingAnalytics.flushNow();
    expect(readKey(ACCOUNT_QUEUE)).toBeNull();
  });

  it("decodes the server summary shape", () => {
    expect(
      decodeSummary({
        totals: { ayahs: 3, ms: 9000, days: 2 },
        topSurahs: [{ surah: 2, reads: 3, ms: 9000 }, { surah: 999 }],
        topAyahs: [{ surah: 2, ayah: 255, reads: 2, ms: 6000 }],
        days: [{ localDay: "2026-10-12", ayahsRead: 3, ms: 9000 }],
      }),
    ).toEqual({
      totals: { ayahs: 3, ms: 9000, days: 2 },
      topSurahs: [{ surah: 2, reads: 3, ms: 9000 }],
      topAyahs: [{ surah: 2, ayah: 255, reads: 2, ms: 6000 }],
      days: [{ localDay: "2026-10-12", ayahsRead: 3, ms: 9000 }],
    });
  });

  it("GA4 never sees surah paths: reader surfaces report as /reader", () => {
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
    expect(scrubReaderPath("/yours/cards/abc")).toBe("/yours/cards");
    expect(scrubReaderPath("/ar/yours/cards/abc")).toBe("/yours/cards");
  });
});
