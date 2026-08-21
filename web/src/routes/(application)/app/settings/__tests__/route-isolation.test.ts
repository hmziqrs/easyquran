import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vite-plus/test";

const SRC_ROOT = join(process.cwd(), "src");

function collectSources(dir: string, out = new Map<string, string>()): Map<string, string> {
  for (const name of readdirSync(dir)) {
    const full = join(dir, name);
    if (name === "node_modules" || name === ".svelte-kit") continue;
    // SAFETY: entries come from readdirSync of repo-owned directories; statSync follows each one.
    if (statSync(full).isDirectory()) {
      collectSources(full, out);
      continue;
    }
    if (!name.endsWith(".svelte") && !name.endsWith(".ts")) continue;
    out.set(full.slice(SRC_ROOT.length), readFileSync(full, "utf-8"));
  }
  return out;
}

const sources = collectSources(SRC_ROOT);

const SECTION_COMPONENTS = [
  "StorageSection.svelte",
  "AppearanceSection.svelte",
  "ReadingSection.svelte",
  "PrivacySection.svelte",
  "AccountSection.svelte",
] as const;

const SETTINGS_DIR = "app/settings/";

describe("settings route chunk isolation", () => {
  it("section components are referenced only from inside the settings route", () => {
    const outside = [...sources].filter(
      ([path, src]) =>
        !path.includes(SETTINGS_DIR) &&
        !path.includes("__tests__") &&
        SECTION_COMPONENTS.some((name) => src.includes(name)),
    );
    expect(
      outside.map(([path]) => path),
      "settings section components may only be imported by the settings route",
    ).toEqual([]);
  });

  it("the settings shell statically imports every section (route-chunk contract, not per-section loaders)", () => {
    const page = [...sources].find(([path]) => path.endsWith(`${SETTINGS_DIR}+page.svelte`));
    expect(page, "settings +page.svelte should exist").toBeDefined();
    for (const name of SECTION_COMPONENTS) {
      expect(
        page![1],
        `+page.svelte must keep importing ${name} statically`,
      ).toMatch(new RegExp(`import\\s+\\w+\\s+from\\s+"\\./_components/${name}"`, "u"));
    }
  });

  it("shared chrome and the reader tweaks panel never import settings modules", () => {
    const chromePaths = [
      "lib/components/nav/Nav.svelte",
      "lib/components/tweaks/Tweaks.svelte",
      "routes/+layout.svelte",
      "_components/MarketingTweaks.svelte",
    ];
    for (const suffix of chromePaths) {
      const entry = [...sources].find(([path]) => path.endsWith(suffix));
      expect(entry, `${suffix} should exist`).toBeDefined();
      const src = entry![1];
      expect(src, `${suffix} must not statically import a settings route module`).not.toMatch(
        /import[^"']*["'][^"']*app\/settings/,
      );
      expect(src, `${suffix} must not dynamically import a settings route module`).not.toMatch(
        /import\([^)]*app\/settings/,
      );
    }
  });

  it("the settings page resolves copy through the resolver, never raw paraglide", () => {
    const page = [...sources].find(([path]) => path.endsWith(`${SETTINGS_DIR}+page.svelte`));
    expect(page, "settings +page.svelte should exist").toBeDefined();
    expect(page![1]).toContain("getSettingsCopy");
    expect(page![1]).not.toContain("$lib/paraglide");
    expect(page![1]).not.toContain("messages.js");
  });

  it("the settings route never contributes a prerendered /app/**/__data.json", () => {
    const settingsFiles = [...sources.keys()].filter((path) => path.includes(SETTINGS_DIR));
    expect(settingsFiles, "expected the settings route tree to be present").not.toEqual([]);
    expect(
      settingsFiles.filter((path) => path.endsWith("+page.server.ts")),
      "settings route must have no +page.server.ts",
    ).toEqual([]);
    expect(
      settingsFiles.filter(
        (path) =>
          path.endsWith("+page.ts") &&
          /export\s+(?:async\s+)?function\s+load/u.test(sources.get(path) ?? ""),
      ),
      "settings +page.ts must stay a pure route-option module",
    ).toEqual([]);
    const pageOptions = sources.get("routes/(application)/app/settings/+page.ts");
    if (pageOptions !== undefined) {
      expect(pageOptions).toContain("prerender = false");
    }
  });
});
