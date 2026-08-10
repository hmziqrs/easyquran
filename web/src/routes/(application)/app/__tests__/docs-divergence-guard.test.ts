import { describe, expect, it } from "vite-plus/test";
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

function findRepoRoot(): string {
  let dir = process.cwd();
  for (let i = 0; i < 12; i += 1) {
    if (existsSync(resolve(dir, "docs/quran-system.md")) && existsSync(resolve(dir, "AGENTS.MD"))) {
      return dir;
    }
    dir = resolve(dir, "..");
  }
  throw new Error("docs-divergence-guard: could not locate repo root from " + process.cwd());
}

const ROOT = findRepoRoot();
const QURAN_SYSTEM = readFileSync(resolve(ROOT, "docs/quran-system.md"), "utf8");
const AGENTS = readFileSync(resolve(ROOT, "AGENTS.MD"), "utf8");

const TRANSLATED_MARKER = /translated|\/app\/t\/|\/app\/\[surah\]\/t\//i;
const SSG_MARKER = /\bSSG\b|prerender/i;
const NEGATION = /\b(?:never|not|no|without|cannot|may\s+not|isn't|aren't|doesn't|don't|won't)\b/i;
const ARABIC_SCOPE = /\bArabic\b/i;

function sentences(text: string): string[] {
  return text.split(/(?<=[.!?])\s+|\n+/);
}

// Banned claim = a sentence that asserts a translated route IS SSG/prerendered.
// The doc's legitimate wording ("Arabic = SSG", "translated pages render SSR",
// "never SSG", "no /app/t/** may be described as SSG") is excluded by the
// negation guard and the Arabic-scope guard.
function translatedSsgClaims(text: string): string[] {
  return sentences(text).filter(
    (s) =>
      TRANSLATED_MARKER.test(s) &&
      SSG_MARKER.test(s) &&
      !NEGATION.test(s) &&
      !ARABIC_SCOPE.test(s),
  );
}

// Bare /app/** (the whole reader tree) described as prerendered/SSG.
// Scoped forms — /app/t/**, /app/[surah]/t/**, /app/<surah> — are not matched.
const BARE_APP_GLOB = /\/app\/\*\*(?!\*)/;

function bareAppPrerenderClaims(text: string): string[] {
  return sentences(text).filter((s) => BARE_APP_GLOB.test(s) && SSG_MARKER.test(s) && !NEGATION.test(s));
}

const MD_LINK = /\[([^\]]+)\]\(([^)]+)\)/g;

function repoFileLinks(
  text: string,
  base: string,
): { label: string; target: string; path: string }[] {
  const out: { label: string; target: string; path: string }[] = [];
  for (const m of text.matchAll(MD_LINK)) {
    const target = m[2].trim();
    if (/^(?:https?:|mailto:|#)/.test(target)) continue;
    out.push({ label: m[1], target, path: resolve(base, target) });
  }
  return out;
}

describe("docs divergence guard (W9)", () => {
  it("docs/quran-system.md never claims translated routes are SSG/prerendered", () => {
    expect(
      translatedSsgClaims(QURAN_SYSTEM),
      "translated-as-SSG claim leaked into quran-system.md",
    ).toEqual([]);
  });

  it("docs/quran-system.md never claims bare /app/** is prerendered (unscoped)", () => {
    expect(
      bareAppPrerenderClaims(QURAN_SYSTEM),
      "unscoped /app/** prerender claim leaked into quran-system.md",
    ).toEqual([]);
  });

  it("AGENTS.MD markdown links to repo files resolve", () => {
    const links = repoFileLinks(AGENTS, ROOT);
    for (const l of links) {
      expect(
        existsSync(l.path),
        `broken AGENTS.MD link [${l.label}](${l.target}) -> ${l.path}`,
      ).toBe(true);
    }
  });

  it("docs/quran-system.md markdown links to repo files resolve", () => {
    const links = repoFileLinks(QURAN_SYSTEM, resolve(ROOT, "docs"));
    for (const l of links) {
      expect(
        existsSync(l.path),
        `broken quran-system.md link [${l.label}](${l.target}) -> ${l.path}`,
      ).toBe(true);
    }
  });

  it("detectors flag the banned claims (the guard is not toothless)", () => {
    expect(translatedSsgClaims("Translated reader routes are SSG.")).toHaveLength(1);
    expect(translatedSsgClaims("The /app/t/** family is prerendered.")).toHaveLength(1);
    expect(translatedSsgClaims("Translated pages are prerendered.")).toHaveLength(1);

    expect(
      translatedSsgClaims("Translated pages render SSR + disk-TTL. Never SSG."),
    ).toHaveLength(0);
    expect(translatedSsgClaims("Arabic = SSG. Translated pages = SSR on Bun.")).toHaveLength(0);
    expect(translatedSsgClaims("no /app/t/** path may be described as SSG.")).toHaveLength(0);

    expect(bareAppPrerenderClaims("All of /app/** is prerendered.")).toHaveLength(1);
    expect(bareAppPrerenderClaims("/app/<surah> is prerendered.")).toHaveLength(0);
  });
});
