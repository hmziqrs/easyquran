import { describe, expect, it, vi } from "vite-plus/test";

vi.mock("$app/env", () => ({ browser: false, dev: false, building: false }));
vi.mock("$app/env/public", () => ({
  PUBLIC_API_BASE_URL: "",
  PUBLIC_FCM_VAPID_KEY: "",
  PUBLIC_QURAN_API_BASE: "",
  PUBLIC_ENV: "local",
}));

import { dailyGlobalIndex } from "#lib/home/daily-ayah.js";
import { loadDailyAyah } from "#lib/server/daily-ayah.js";
import { QURAN_DATA } from "#lib/server/quran-data.js";

describe("loadDailyAyah", () => {
  it("names the verse at the date's global index, with its surah and text", () => {
    const date = "2026-11-06";
    const key = QURAN_DATA.verseKeyAtGlobal(dailyGlobalIndex(date));
    expect(key).toBeDefined();
    const daily = loadDailyAyah(date);
    expect(daily).not.toBeNull();
    const [surah, ayah] = key!.split(":").map(Number);
    expect(daily).toMatchObject({ date, surah, ayah });
    expect(daily!.name).toBe(QURAN_DATA.surahByNum(surah!)?.name);
    expect(daily!.slug).toBe(QURAN_DATA.surahByNum(surah!)?.slug);
    expect(daily!.text.length).toBeGreaterThan(0);
  });

  it("returns the same verse for every call on the same UTC date", () => {
    expect(loadDailyAyah("2026-11-06")).toEqual(loadDailyAyah("2026-11-06"));
  });
});
