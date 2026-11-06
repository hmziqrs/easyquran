import { readFileSync } from "node:fs";
import { join } from "node:path";

import { describe, expect, it } from "vite-plus/test";

// Vitest runs from the web/ package root, where static/ lives.
const staticFile = (relative: string): string =>
  readFileSync(join(process.cwd(), "static", relative), "utf8");

describe("robots.txt", () => {
  it("disallows the offline pack path for every crawler", () => {
    const lines = staticFile("robots.txt")
      .split("\n")
      .map((line) => line.trim());
    expect(lines).toContain("Disallow: /offline/");
    const agentIndex = lines.indexOf("User-agent: *");
    expect(agentIndex).toBeGreaterThanOrEqual(0);
    expect(lines.indexOf("Disallow: /offline/")).toBeGreaterThan(agentIndex);
  });
});

describe(".well-known/quran-bulk.json", () => {
  const raw = staticFile(".well-known/quran-bulk.json");
  const bulk: unknown = JSON.parse(raw);

  it("is served as valid JSON with the documented shape", () => {
    expect(bulk).toMatchObject({
      version: 1,
      offline: {
        limits: { downloadsPerDay: 20, burstPerMinute: 5, bytesPerDay: 100 * 1024 * 1024 },
      },
    });
  });

  it("publishes no checksum or hash fields (SHA-256 is manual audit only)", () => {
    expect(raw.toLowerCase()).not.toMatch(/sha-?256|"hash"|"checksum"|"digest"/u);
  });
});
