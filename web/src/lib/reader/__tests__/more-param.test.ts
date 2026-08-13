import { describe, it, expect } from "vite-plus/test";
import { uniq } from "es-toolkit";

import {
  READER_MORE_PARAM,
  moreParamMatches,
  parseMoreParam,
  withMoreParam,
} from "../more-param";
import { STACKED_MAX_EXTRAS } from "$lib/stores/stacked-translations.svelte";

const url = (pathAndSearch: string): URL => new URL(`https://easyquran.local${pathAndSearch}`);

describe("parseMoreParam", () => {
  it("returns [] when the param is absent", () => {
    expect(parseMoreParam(url("/app/al-baqarah"))).toEqual([]);
  });

  it("splits, trims, and drops empty parts", () => {
    expect(parseMoreParam(url("/app/1?more=en.sahih,, ,ur.jalandhry"))).toEqual([
      "en.sahih",
      "ur.jalandhry",
    ]);
  });

  it("dedupes preserving first occurrence", () => {
    expect(parseMoreParam(url("/app/1?more=en.sahih,ur.jalandhry,en.sahih"))).toEqual([
      "en.sahih",
      "ur.jalandhry",
    ]);
  });
});

describe("withMoreParam", () => {
  it("sets the csv when ids are non-empty", () => {
    const out = withMoreParam(url("/app/1"), ["en.sahih", "ur.jalandhry"]);
    expect(out.searchParams.get(READER_MORE_PARAM)).toBe("en.sahih,ur.jalandhry");
  });

  it("deletes the param when ids are empty", () => {
    const out = withMoreParam(url("/app/1?more=en.sahih"), []);
    expect(out.searchParams.has(READER_MORE_PARAM)).toBe(false);
  });

  it("round-trips through parseMoreParam", () => {
    const ids = ["en.sahih", "ur.jalandhry", "fr.hamid"];
    expect(parseMoreParam(withMoreParam(url("/app/1?mode=verse"), ids))).toEqual(ids);
    // preserves unrelated params
    expect(withMoreParam(url("/app/1?mode=verse"), ids).searchParams.get("mode")).toBe("verse");
  });
});

describe("moreParamMatches", () => {
  it("is order-insensitive", () => {
    const u = url("/app/1?more=en.sahih,ur.jalandhry");
    expect(moreParamMatches(u, ["ur.jalandhry", "en.sahih"])).toBe(true);
    expect(moreParamMatches(u, ["en.sahih", "ur.jalandhry"])).toBe(true);
  });

  it("rejects length mismatch and foreign ids", () => {
    const u = url("/app/1?more=en.sahih,ur.jalandhry");
    expect(moreParamMatches(u, ["en.sahih"])).toBe(false);
    expect(moreParamMatches(u, ["en.sahih", "fr.hamid"])).toBe(false);
  });

  it("matches an empty store against a bare url", () => {
    expect(moreParamMatches(url("/app/1"), [])).toBe(true);
  });
});

// Mirrors the adopt/sync decision tree of the +layout.svelte backstop $effect so the
// convergence contract is unit-testable without mounting Svelte. Sequential (NOT else-if):
// adopt runs first (capping the store), then sync re-reads the store so an over-cap
// ?more=a,..,f normalizes to the capped 5 in the URL.
function reconcile(u: URL, ids: string[]) {
  const parsed = parseMoreParam(u);
  let next = ids;
  if (parsed.length > 0 && !moreParamMatches(u, next)) {
    next = uniq(parsed).slice(0, STACKED_MAX_EXTRAS);
  }
  let out = u;
  if (!moreParamMatches(u, next)) {
    out = withMoreParam(u, next);
  }
  return { ids: next, url: out };
}

describe("cold deep-link convergence (adopts ?more without flicker)", () => {
  it("adopts ?more into an empty store and leaves the bar stable", () => {
    let u = url("/app/al-baqarah?more=en.sahih");
    let ids: string[] = [];
    for (let i = 0; i < 2; i++) {
      const r = reconcile(u, ids);
      ids = r.ids;
      u = r.url;
    }
    expect(ids).toEqual(["en.sahih"]);
    expect(parseMoreParam(u)).toEqual(["en.sahih"]);
  });

  it("normalizes an over-cap ?more down to STACKED_MAX_EXTRAS in the bar", () => {
    let u = url("/app/1?more=a,b,c,d,e,f");
    let ids: string[] = [];
    for (let i = 0; i < 4; i++) {
      const r = reconcile(u, ids);
      ids = r.ids;
      u = r.url;
    }
    expect(ids).toEqual(["a", "b", "c", "d", "e"]);
    expect(parseMoreParam(u)).toEqual(["a", "b", "c", "d", "e"]);
  });

  it("re-adds ?more after internal nav dropped it (store -> url sync)", () => {
    let u = url("/app/1");
    const ids = ["en.sahih"];
    for (let i = 0; i < 2; i++) {
      const r = reconcile(u, ids);
      u = r.url;
    }
    expect(parseMoreParam(u)).toEqual(["en.sahih"]);
  });
});
