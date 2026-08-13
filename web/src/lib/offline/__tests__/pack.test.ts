import { decodePack } from "$lib/offline/pack";
import { describe, expect, it } from "vite-plus/test";

function buildPack(map: Record<string, string>): string {
  const keys = Object.keys(map).sort();
  const entries: Record<string, number> = {};
  const bodies: string[] = [];
  for (const key of keys) {
    entries[key] = bodies.length;
    bodies.push(map[key]!);
  }
  return JSON.stringify({ version: 1, entries, bodies });
}

function tamper(
  text: string,
  mutate: (root: { version: unknown; entries: unknown; bodies: unknown }) => void,
): string {
  const root = JSON.parse(text) as { version: unknown; entries: unknown; bodies: unknown };
  mutate(root);
  return JSON.stringify(root);
}

describe("decodePack", () => {
  it("round-trips a valid pack and recovers entries/bodies", () => {
    const text = buildPack({
      "/app/al-kahf/__data.json": '{"node":1}',
      "/__data.json": '{"node":0}',
      "/app/juz/30/__data.json": '{"node":2}',
    });
    const pack = decodePack(text);
    expect(Object.keys(pack.entries).sort()).toEqual([
      "/__data.json",
      "/app/al-kahf/__data.json",
      "/app/juz/30/__data.json",
    ]);
    expect(pack.bodies[pack.entries["/app/al-kahf/__data.json"]!]).toBe('{"node":1}');
  });

  it("rejects an unsupported version", () => {
    const text = buildPack({ "/a/__data.json": "0" });
    const bad = tamper(text, (root) => {
      root.version = 2;
    });
    expect(() => decodePack(bad)).toThrow(/unsupported/);
  });

  it("rejects entries that is not an object", () => {
    const text = buildPack({ "/a/__data.json": "0" });
    const bad = tamper(text, (root) => {
      root.entries = [];
    });
    expect(() => decodePack(bad)).toThrow(/entries is not an object/);
  });

  it("rejects bodies that is not an array", () => {
    const text = buildPack({ "/a/__data.json": "0" });
    const bad = tamper(text, (root) => {
      root.bodies = {};
    });
    expect(() => decodePack(bad)).toThrow(/bodies is not an array/);
  });

  it("rejects a non-string body", () => {
    const text = buildPack({ "/a/__data.json": "0", "/b/__data.json": "1" });
    const bad = tamper(text, (root) => {
      (root.bodies as unknown[])[0] = 123;
    });
    expect(() => decodePack(bad)).toThrow(/body 0 is not a string/);
  });

  it("rejects an out-of-range entry index", () => {
    const text = buildPack({ "/a/__data.json": "0", "/b/__data.json": "1" });
    const bad = tamper(text, (root) => {
      (root.entries as Record<string, number>)["/a/__data.json"] = 99;
    });
    expect(() => decodePack(bad)).toThrow(/invalid index/);
  });

  it("rejects an entries/bodies count mismatch", () => {
    const text = buildPack({ "/a/__data.json": "0" });
    const bad = tamper(text, (root) => {
      (root.bodies as string[]).push("extra");
    });
    expect(() => decodePack(bad)).toThrow(/length mismatch/);
  });
});
