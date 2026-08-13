import { describe, expect, it } from "vite-plus/test";

/**
 * The palette is code-split: only `palette-loader.ts` may reference
 * `GlobalSearchPalette.svelte`, and only through `import()`. A static import
 * from anywhere else pulls the Command primitives, the Quran catalogue and every
 * source back into the initial bundle on every page — a regression that costs
 * real bytes and shows up nowhere else in CI.
 */
// SAFETY: glob uses query "?raw" + import "default", so every matched module's default
// export is its own source text — vite-plus infers the files' default as string.
const sources = import.meta.glob("../../../**/*.{svelte,ts}", {
  query: "?raw",
  import: "default",
  eager: true,
}) as Record<string, string>;

const PALETTE = "GlobalSearchPalette";
// Vite normalizes glob keys to the shortest relative form, so match basenames.
const basename = (path: string): string => path.split("/").pop() ?? path;

const referrers = Object.entries(sources).filter(
  ([path, src]) =>
    basename(path) !== "GlobalSearchPalette.svelte" &&
    !path.includes("__tests__") &&
    src.includes(PALETTE),
);

describe("global search palette stays out of the initial bundle", () => {
  it("is referenced only by the loader module", () => {
    expect(
      referrers.map(([path]) => path),
      "only palette-loader.ts may reference GlobalSearchPalette",
    ).toEqual([expect.stringContaining("palette-loader.ts")]);
  });

  it("is reached through a dynamic import, never a static one", () => {
    for (const [path, src] of referrers) {
      expect(src, `${path} must use import(), not a static import`).toMatch(
        /import\(\s*["'][^"']*GlobalSearchPalette\.svelte["']\s*\)/,
      );
      expect(src, `${path} must not statically import the palette`).not.toMatch(
        /^\s*import\s[^(]*GlobalSearchPalette/m,
      );
    }
  });

  it("keeps the always-mounted entry component free of heavy dependencies", () => {
    const entry = Object.entries(sources).find(
      ([path]) => basename(path) === "GlobalSearch.svelte",
    );
    expect(entry, "GlobalSearch.svelte should exist").toBeDefined();
    // Only import statements count — the file's own comment names these on purpose.
    const imports = (entry![1].match(/^\s*import\s.*$/gm) ?? []).join("\n");
    for (const heavy of ["bits-ui", "$lib/search/palette", "$lib/quran/", "$lib/data/quran"]) {
      expect(imports, `GlobalSearch.svelte must not import ${heavy}`).not.toContain(heavy);
    }
  });
});
