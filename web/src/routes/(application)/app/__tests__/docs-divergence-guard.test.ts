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

// W9 divergence #7 — unsupported artifact-safety claim. A sentence affirmatively
// asserting that remote catalogue/metadata (or eq-data-v1 metadata) SELECTS Quran
// bytes/size/path/delivery-origin, or that a partially-written DB REACHES active
// state. The doc's legitimate W10 wording pairs each with a negation ("never
// selects", "can never become active", "may describe availability but never …"),
// and the baked-id wording ("reconstructed from baked maps", "stages into an
// id-scoped temp file") carries no selecting/becoming-active verb — both are
// excluded by the negation guard and the marker shapes.
const REMOTE_META =
  /\b(?:remote\s+catalogue|remote\s+metadata|catalogue\s+metadata|eq-data-v1\s+metadata)\b/i;
const PARTIAL_DB =
  /\b(?:partially[\s-]written|partial\s+(?:db|database|file|download)|incomplete\s+(?:db|database|file|download|write))\b/i;
// A selecting/determining verb co-occurring with a bytes/size/path/origin noun.
const SELECTS_BYTES =
  /\b(?:selects?|determines?|chooses?|picks?|drives?|controls?|directs?|sources?)\b[^.;]*(?:bytes?|\bsizes?\b|\bpaths?\b|r2Path|delivery(?:\s+origin)?|\borigins?\b)/i;
// A "reaches active state" verb for a partially-written DB.
const ACTIVE_STATE =
  /\b(?:becomes?|can\s+become|go(?:es)?\s+(?:live|active)|activat\w+|(?:is|stays?|remains?)\s+active|made\s+active)\b/i;

function artifactSafetyClaims(text: string): string[] {
  return sentences(text).filter(
    (s) =>
      !NEGATION.test(s) &&
      ((REMOTE_META.test(s) && SELECTS_BYTES.test(s)) ||
        (PARTIAL_DB.test(s) && ACTIVE_STATE.test(s))),
  );
}

const MD_LINK = /\[([^\]]+)\]\(([^)]+)\)/g;

function repoFileLinks(
  text: string,
  base: string,
): { label: string; target: string; path: string }[] {
  const out: { label: string; target: string; path: string }[] = [];
  for (const m of text.matchAll(MD_LINK)) {
    const target = m[2]!.trim();
    if (/^(?:https?:|mailto:|#)/.test(target)) continue;
    out.push({ label: m[1]!, target, path: resolve(base, target) });
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

  it("docs/quran-system.md never asserts an unsupported artifact-safety claim (W9 #7)", () => {
    expect(
      artifactSafetyClaims(QURAN_SYSTEM),
      "unsupported artifact-safety claim (remote metadata selecting bytes / partial DB becoming active) leaked into quran-system.md",
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

    // Banned artifact-safety claims (W9 #7).
    expect(artifactSafetyClaims("Remote catalogue metadata selects Quran bytes for each download.")).toHaveLength(1);
    expect(artifactSafetyClaims("The remote catalogue determines the delivery path.")).toHaveLength(1);
    expect(artifactSafetyClaims("Remote metadata controls delivery.")).toHaveLength(1);
    expect(artifactSafetyClaims("A partially written DB becomes active.")).toHaveLength(1);
    expect(artifactSafetyClaims("Partially-written databases can become active before validation.")).toHaveLength(1);

    // Legitimate W10 wording — must NOT trip.
    expect(
      artifactSafetyClaims(
        "remote catalogue metadata may describe availability but never selects Quran bytes, size, or a delivery origin, and eq-data-v1 metadata is bounded the same way.",
      ),
    ).toHaveLength(0);
    expect(artifactSafetyClaims("A partially written DB can never become active.")).toHaveLength(0);
    expect(
      artifactSafetyClaims(
        "Production download specs are reconstructed from baked {id, sizeBytes, r2Path, sameOriginDeliveryPath} maps.",
      ),
    ).toHaveLength(0);
    expect(
      artifactSafetyClaims(
        "Downloads stage into an id-scoped temp file before the {sourceId, activeFile} pointer switches atomically.",
      ),
    ).toHaveLength(0);
    expect(artifactSafetyClaims("The web's baked catalogue is a flat positional array.")).toHaveLength(0);
  });
});
