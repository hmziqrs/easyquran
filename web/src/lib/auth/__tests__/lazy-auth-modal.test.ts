import { describe, expect, it } from "vite-plus/test";

const sources = import.meta.glob("../../../**/*.{svelte,ts}", {
  query: "?raw",
  import: "default",
  eager: true,
}) as Record<string, string>;

const LAZY = "AuthModal.svelte";
const basename = (path: string): string => path.split("/").pop() ?? path;

const referrers = Object.entries(sources).filter(
  ([path, src]) =>
    basename(path) !== LAZY &&
    !path.includes("__tests__") &&
    src.includes(LAZY),
);

describe("auth modal stays out of the initial bundle", () => {
  it("AuthModal.svelte is referenced only by the loader module", () => {
    expect(
      referrers.map(([path]) => path),
      "only auth-modal-loader.ts may reference AuthModal.svelte",
    ).toEqual([expect.stringContaining("auth-modal-loader.ts")]);
  });

  it("AuthModal.svelte is reached through a dynamic import, never a static one", () => {
    for (const [path, src] of referrers) {
      expect(src, `${path} must use import(), not a static import`).toMatch(
        /import\(\s*["'][^"']*AuthModal\.svelte["']\s*\)/,
      );
      expect(src, `${path} must not statically import AuthModal`).not.toMatch(
        /^\s*import\s[^(]*AuthModal\.svelte/m,
      );
    }
  });

  it("the always-mounted shell stays free of heavy dependencies", () => {
    const entry = Object.entries(sources).find(
      ([path]) => basename(path) === "AuthModalShell.svelte",
    );
    expect(entry, "AuthModalShell.svelte should exist").toBeDefined();
    const imports = (entry![1].match(/^\s*import\s.*$/gm) ?? []).join("\n");
    for (const heavy of [
      "bits-ui",
      "/flows.svelte",
      "AuthModal.svelte",
      "$lib/auth/components",
    ]) {
      expect(imports, `AuthModalShell.svelte must not import ${heavy}`).not.toContain(heavy);
    }
  });

  it("auth field inputs render at h-11 to match the submit button", () => {
    const field = Object.entries(sources).find(
      ([path]) => basename(path) === "AuthField.svelte",
    );
    expect(field, "AuthField.svelte should exist").toBeDefined();
    expect(field![1], "AuthField must pass h-11 to its Input").toContain("h-11");
  });
});
