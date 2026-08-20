import { beforeEach, describe, expect, it, vi } from "vite-plus/test";

const { site } = vi.hoisted(() => ({
  site: {
    apiBase: "https://api.test/quran",
    artifactBase: "/_quran",
    scripts: [
      {
        id: "uthmani",
        sizeBytes: 2885632,
        downloadUrl: "/_quran/tanzil/arabic/quran-uthmani.sqlite",
      },
    ],
  },
}));

vi.mock("$lib/config/site", () => ({ QURAN: site }));

describe("bakedManifest", () => {
  beforeEach(() => {
    site.apiBase = "https://api.test/quran";
  });

  it("returns only compile-time script metadata", async () => {
    const fetchSpy = vi.spyOn(globalThis, "fetch");
    const { bakedManifest } = await import("$lib/quran/manifest");

    expect(bakedManifest()).toEqual({ scripts: site.scripts });
    expect(fetchSpy).not.toHaveBeenCalled();
  });

  it("does not consult the API when an API base is configured", async () => {
    const fetchSpy = vi.spyOn(globalThis, "fetch");
    const { bakedManifest } = await import("$lib/quran/manifest");

    bakedManifest();

    expect(site.apiBase).not.toBe("");
    expect(fetchSpy).not.toHaveBeenCalled();
  });
});
