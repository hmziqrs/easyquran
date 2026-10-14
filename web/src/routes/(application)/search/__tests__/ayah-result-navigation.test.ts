import { readFileSync } from "node:fs";
import path from "node:path";

import { mount, unmount } from "svelte";
import { afterEach, describe, expect, it, vi } from "vite-plus/test";

import { createQuranData } from "#lib/data/quran-data.js";

const h = vi.hoisted(() => ({ opened: vi.fn(), factory: vi.fn() }));
vi.mock("#lib/search/page/navigate.js", () => ({
  ayahHrefFor: () => "/al-baqarah#ayah-2-64",
  openVerse: (surah: number) => {
    h.factory(surah);
    return h.opened;
  },
}));
vi.mock("$app/env/public", () => ({
  PUBLIC_API_BASE_URL: undefined,
  PUBLIC_QURAN_API_BASE: undefined,
  PUBLIC_ENV: undefined,
  PUBLIC_FCM_VAPID_KEY: undefined,
}));

import AyahResult from "../_components/AyahResult.svelte";

const quranData = createQuranData(
  JSON.parse(
    readFileSync(path.resolve(process.cwd(), "static/quran-meta/quran-data.json"), "utf8"),
  ),
);

let component: ReturnType<typeof mount> | undefined;
let target: HTMLDivElement | undefined;

afterEach(async () => {
  if (component) await unmount(component);
  target?.remove();
  component = undefined;
  target = undefined;
});

describe("search result navigation", () => {
  it.each([
    { id: "arabic", kind: "arabic" as const },
    { id: "en.sahih", kind: "translation" as const },
  ])("opens the $id verse as a new reading thread", async ({ id, kind }) => {
    h.opened.mockClear();
    h.factory.mockClear();
    target = document.createElement("div");
    document.body.append(target);
    component = mount(AyahResult, {
      target,
      props: {
        hit: { surah: 2, ayah: 64, text: "", highlights: [] },
        section: { id, kind, phase: "done", gate: null, hits: [], total: 1, offset: 0, limit: 20 },
        quranData,
      },
    });
    const link = target.querySelector("a");
    expect(link).not.toBeNull();
    link?.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
    expect(h.factory).toHaveBeenCalledWith(2);
    expect(h.opened).toHaveBeenCalledTimes(1);
  });
});
