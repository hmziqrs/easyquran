import { describe, expect, it } from "vite-plus/test";

/**
 * F15: the microphone is open to this origin only, so voice search works. Every
 * web-tier place that sets Permissions-Policy must agree, and camera and location
 * stay closed. The API tier (security_headers.rs) keeps `microphone=()`; its own
 * test in rust/backend/api/tests/security_tests.rs pins that.
 */
// SAFETY: glob uses query "?raw" + import "default", so every matched module's default export is its own source text.
const sources = import.meta.glob(
  [
    "../../../hooks.server.ts",
    "../../../../server.ts",
    "../../../../vite-plugin-preview-headers.ts",
    "../../../../static/_headers",
  ],
  { query: "?raw", import: "default", eager: true },
) as Record<string, string>;

describe("web-tier Permissions-Policy", () => {
  it("finds every web-tier header source", () => {
    expect(Object.keys(sources)).toHaveLength(4);
  });

  it.each(Object.entries(sources))("%s opens microphone to self only", (_path, text) => {
    expect(text).toContain("microphone=(self)");
    expect(text).not.toContain("microphone=()");
    expect(text).not.toMatch(/microphone=\*/u);
    expect(text).toContain("camera=()");
    expect(text).toContain("geolocation=()");
  });
});
