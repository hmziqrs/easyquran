import { describe, expect, it, vi } from "vite-plus/test";

vi.mock("$app/env/private", () => ({
  INTERNAL_QURAN_API_BASE: "https://api.test",
  INTERNAL_QURAN_API_TOKEN: undefined,
  OWNER_SOURCE_URL: undefined,
}));

vi.mock("$app/env/public", () => ({
  PUBLIC_API_BASE_URL: undefined,
  PUBLIC_QURAN_API_BASE: "https://public.test",
  PUBLIC_ENV: "local",
  PUBLIC_FCM_VAPID_KEY: undefined,
}));

import { markdownRenderFetcher } from "#lib/server/quran-translation-page.js";

describe("markdownRenderFetcher", () => {
  it("tags every call from a markdown route as render class md", async () => {
    const seen: Headers[] = [];
    const inner = vi.fn(async (_url: string, init?: RequestInit) => {
      seen.push(new Headers(init?.headers));
      return new Response("{}");
    });
    const tagged = markdownRenderFetcher(inner);
    await tagged("https://api.test/quran/v1/sources/en.sahih/range?from=1&to=2", {
      headers: { "x-keep": "1" },
    });
    expect(seen[0]?.get("x-eq-render-class")).toBe("md");
    expect(seen[0]?.get("x-keep")).toBe("1");
  });
});
