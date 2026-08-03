import { describe, expect, it } from "vite-plus/test";
import { decodePack, sha256Hex } from "$lib/offline/pack";

async function buildPack(
  map: Record<string, string>,
): Promise<{ text: string; hash: string }> {
  const keys = Object.keys(map).sort();
  const entries: Record<string, number> = {};
  const bodies: string[] = [];
  for (const k of keys) {
    entries[k] = bodies.length;
    bodies.push(map[k]);
  }
  const text = JSON.stringify({ version: 1, entries, bodies });
  const hash = (await sha256Hex(text)).slice(0, 12);
  return { text, hash };
}

async function tamper(
  text: string,
  mutate: (root: { version: unknown; entries: unknown; bodies: unknown }) => void,
): Promise<{ text: string; hash: string }> {
  const root = JSON.parse(text) as { version: unknown; entries: unknown; bodies: unknown };
  mutate(root);
  const next = JSON.stringify(root);
  return { text: next, hash: (await sha256Hex(next)).slice(0, 12) };
}

describe("decodePack", () => {
  it("round-trips a valid pack and recovers entries/bodies", async () => {
    const { text, hash } = await buildPack({
      "/app/al-kahf/__data.json": '{"node":1}',
      "/__data.json": '{"node":0}',
      "/app/juz/30/__data.json": '{"node":2}',
    });
    const pack = await decodePack(text, hash);
    expect(Object.keys(pack.entries).sort()).toEqual([
      "/__data.json",
      "/app/al-kahf/__data.json",
      "/app/juz/30/__data.json",
    ]);
    expect(pack.bodies[pack.entries["/app/al-kahf/__data.json"]]).toBe('{"node":1}');
  });

  it("is deterministic: insertion order does not change the hash", async () => {
    const a = await buildPack({ "/b/__data.json": "1", "/a/__data.json": "0", "/c/__data.json": "2" });
    const b = await buildPack({ "/c/__data.json": "2", "/a/__data.json": "0", "/b/__data.json": "1" });
    expect(a.hash).toBe(b.hash);
  });

  it("rejects a hash mismatch", async () => {
    const { text } = await buildPack({ "/a/__data.json": "0" });
    await expect(decodePack(text, "000000000000")).rejects.toThrow(/hash mismatch/);
  });

  it("rejects an unsupported version", async () => {
    const { text } = await buildPack({ "/a/__data.json": "0" });
    const bad = await tamper(text, (r) => {
      r.version = 2;
    });
    await expect(decodePack(bad.text, bad.hash)).rejects.toThrow(/unsupported/);
  });

  it("rejects entries that is not an object", async () => {
    const { text } = await buildPack({ "/a/__data.json": "0" });
    const bad = await tamper(text, (r) => {
      r.entries = [];
    });
    await expect(decodePack(bad.text, bad.hash)).rejects.toThrow(/entries is not an object/);
  });

  it("rejects bodies that is not an array", async () => {
    const { text } = await buildPack({ "/a/__data.json": "0" });
    const bad = await tamper(text, (r) => {
      r.bodies = {};
    });
    await expect(decodePack(bad.text, bad.hash)).rejects.toThrow(/bodies is not an array/);
  });

  it("rejects a non-string body", async () => {
    const { text } = await buildPack({ "/a/__data.json": "0", "/b/__data.json": "1" });
    const bad = await tamper(text, (r) => {
      (r.bodies as unknown[])[0] = 123;
    });
    await expect(decodePack(bad.text, bad.hash)).rejects.toThrow(/body 0 is not a string/);
  });

  it("rejects an out-of-range entry index", async () => {
    const { text } = await buildPack({ "/a/__data.json": "0", "/b/__data.json": "1" });
    const bad = await tamper(text, (r) => {
      (r.entries as Record<string, number>)["/a/__data.json"] = 99;
    });
    await expect(decodePack(bad.text, bad.hash)).rejects.toThrow(/invalid index/);
  });

  it("rejects an entries/bodies count mismatch", async () => {
    const { text } = await buildPack({ "/a/__data.json": "0" });
    const bad = await tamper(text, (r) => {
      (r.bodies as string[]).push("extra");
    });
    await expect(decodePack(bad.text, bad.hash)).rejects.toThrow(/length mismatch/);
  });
});
