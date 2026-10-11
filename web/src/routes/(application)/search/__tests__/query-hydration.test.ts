import { mount, unmount } from "svelte";
import { afterEach, describe, expect, it, vi } from "vite-plus/test";

const h = vi.hoisted(() => ({
  url: new URL("https://easyquran.app/search"),
  replaceState: vi.fn(),
  search: vi.fn(async () => ({ results: [], total: 0 })),
}));

vi.mock("$app/env/public", () => ({
  PUBLIC_API_BASE_URL: undefined,
  PUBLIC_QURAN_API_BASE: undefined,
  PUBLIC_ENV: undefined,
  PUBLIC_FCM_VAPID_KEY: undefined,
}));
vi.mock("$app/state", () => ({
  page: {
    get url() {
      return h.url;
    },
    state: {},
  },
}));
vi.mock("$app/navigation", () => ({ replaceState: h.replaceState }));
vi.mock("#lib/data/quran-data-client.js", () => ({
  loadQuranData: () => Promise.reject(new Error("Metadata unavailable in isolated search test")),
}));
vi.mock("#lib/quran/search.js", () => ({ quranSearch: h.search }));
vi.mock("#lib/quran/worker-client.js", () => ({
  quranWorker: { onProgress: () => () => undefined },
}));
vi.mock("#lib/stores/storage-report.svelte.js", () => ({
  storageReport: { artifacts: [], hydrate: () => undefined },
}));

import { searchSelection } from "#lib/stores/search-selection.svelte.js";

import SearchPage from "../+page.svelte";

let component: ReturnType<typeof mount> | undefined;
let target: HTMLDivElement | undefined;

afterEach(async () => {
  if (component) await unmount(component);
  target?.remove();
  component = undefined;
  target = undefined;
  searchSelection.clear();
});

describe("search query hydration", () => {
  it.each([
    { query: "الحمد", translations: "" },
    { query: "mercy", translations: "en.sahih" },
  ])("keeps $query in the URL during initial debounce", async ({ query, translations }) => {
    h.url = new URL("https://easyquran.app/search");
    h.url.searchParams.set("q", query);
    if (translations) h.url.searchParams.set("t", translations);
    h.replaceState.mockReset();
    h.search.mockClear();
    target = document.createElement("div");
    document.body.append(target);
    component = mount(SearchPage, { target });
    await new Promise((resolve) => setTimeout(resolve, 200));
    expect(h.search).toHaveBeenCalledWith(query, expect.any(Object));
    for (const [href] of h.replaceState.mock.calls) {
      const written = new URL(String(href), h.url);
      expect(written.searchParams.get("q")).toBe(query);
      if (translations) expect(written.searchParams.get("t")).toBe(translations);
    }
    expect(target.querySelector<HTMLInputElement>("#search-query")?.value).toBe(query);
  });
});
