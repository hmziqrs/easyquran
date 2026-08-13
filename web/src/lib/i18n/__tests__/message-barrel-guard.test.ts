import { readFileSync, readdirSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vite-plus/test";

const SELF = fileURLToPath(import.meta.url);
const SRC = resolve(dirname(SELF), "../../..");
const GENERATED_BARRELS = join(SRC, "lib/i18n/m");

// `$lib/paraglide/messages.js` re-exports every message. A barrel imported from more than one route
// is a tree-shaking pinch point: the bundler hoists the union of all app-wide-used messages into one
// shared chunk that every page downloads (paraglide-js#668). Only the generated per-namespace
// barrels may reach into the compiled message modules; everything else imports a namespace.
const FORBIDDEN = [
  '$lib/paraglide/messages.js"',
  '$lib/paraglide/messages"',
  "$lib/paraglide/messages/_index",
];

function walk(dir: string, found: string[] = []): string[] {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (entry.name === "paraglide") continue;
      walk(path, found);
    } else if (/\.(ts|svelte)$/.test(entry.name) && path !== SELF) {
      // This guard names the forbidden specifiers in order to search for them.
      found.push(path);
    }
  }
  return found;
}

describe("message barrel boundaries", () => {
  const files = walk(SRC);

  it("finds source to check", () => {
    expect(files.length).toBeGreaterThan(100);
  });

  it("never imports the global message barrel", () => {
    const offenders = files.filter((file) => {
      const source = readFileSync(file, "utf8");
      return FORBIDDEN.some((forbidden) => source.includes(forbidden));
    });

    expect(
      offenders.map((file) => relative(SRC, file)),
      "import a namespace from $lib/i18n/m/* instead — see docs/quran-system.md (Part 2, Message chunking)",
    ).toEqual([]);
  });

  it("reaches compiled message modules only from generated namespace barrels", () => {
    const offenders = files.filter((file) => {
      if (file.startsWith(GENERATED_BARRELS)) return false;
      return readFileSync(file, "utf8").includes("$lib/paraglide/messages/");
    });

    expect(offenders.map((file) => relative(SRC, file))).toEqual([]);
  });

  it("keeps every generated barrel machine-written", () => {
    for (const file of readdirSync(GENERATED_BARRELS)) {
      const source = readFileSync(join(GENERATED_BARRELS, file), "utf8");
      expect(source, `${file} must stay generated`).toContain("AUTO-GENERATED");
      expect(source).toMatch(/^export \{ \w+ \} from "\$lib\/paraglide\/messages\/\w+\.js";$/m);
    }
  });
});
