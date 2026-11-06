import { beforeEach, describe, expect, it, vi } from "vite-plus/test";

import type { AuthRequestResult, UnsafeRequestInit } from "#lib/auth/auth-client.js";
import { asString, type JsonRecord, type JsonValue } from "#lib/storage/decoders.js";
import type { RegisteredSyncDomain } from "#lib/sync/index.js";
import { SyncPausedError } from "#lib/sync/index.js";

import {
  currentReadingBookmark,
  restoreReadingBookmark,
  setReadingBookmark,
} from "../reading-bookmark.svelte";
import { PreferencesStore, READING_BOOKMARK_KEY } from "../store.svelte";

vi.mock("$app/env", () => ({ browser: true }));
vi.mock("$app/env/public", () => ({
  PUBLIC_API_BASE_URL: "https://eq.test/api",
  PUBLIC_QURAN_API_BASE: undefined,
  PUBLIC_ENV: undefined,
  PUBLIC_FCM_VAPID_KEY: undefined,
}));

/** One server: one register per key, highest stamp wins. */
class FakeServer {
  rev = 0;
  rows = new Map<string, { value: JsonValue; stamp: string; rev: number }>();
  status = 200;

  handle(body: JsonRecord): JsonValue {
    const mutations: JsonRecord[] = JSON.parse(JSON.stringify(body.mutations ?? []));
    const results = mutations.map((m) => {
      const key = asString(m.key) ?? "";
      const stamp = asString(m.stamp) ?? "";
      const held = this.rows.get(key);
      if (held !== undefined && held.stamp >= stamp) {
        return { id: key, status: "superseded", stamps: { row: held.stamp } };
      }
      this.rev += 1;
      this.rows.set(key, { value: m.value ?? null, stamp, rev: this.rev });
      return { id: key, status: "applied", stamps: { row: stamp } };
    });
    const since = Number(body.since ?? 0);
    const preferences = [...this.rows.entries()]
      .filter(([, row]) => row.rev > since)
      .map(([key, row]) => ({ key, ...row }));
    return JSON.parse(
      JSON.stringify({
        serverTime: Date.now(),
        syncEpoch: "1",
        results,
        preferences,
        rev: this.rev,
        latestRev: this.rev,
        more: false,
      }),
    );
  }
}

function device(server: FakeServer, userId: number | null = 7) {
  const local = new Map<string, string>();
  vi.stubGlobal("localStorage", {
    getItem: (key: string) => local.get(key) ?? null,
    setItem: (key: string, value: string) => void local.set(key, value),
    removeItem: (key: string) => void local.delete(key),
  });
  const swap = () =>
    vi.stubGlobal("localStorage", {
      getItem: (key: string) => local.get(key) ?? null,
      setItem: (key: string, value: string) => void local.set(key, value),
      removeItem: (key: string) => void local.delete(key),
    });
  const registered: RegisteredSyncDomain[] = [];
  const auth = {
    authenticated: userId !== null,
    user: userId === null ? null : { id: userId },
  };
  const store = new PreferencesStore({
    engine: { flush: async () => undefined, start: () => () => undefined },
    auth: {
      unsafeRequest: async <T>(_path: string, init: UnsafeRequestInit) => {
        const failed = server.status !== 200;
        // SAFETY: test double — the domain always sends an object body.
        const data = failed ? null : server.handle(init.body as JsonRecord);
        // SAFETY: test double — the store decodes this body itself.
        return { ok: !failed, status: server.status, data: data as T, error: null, rotated: false } as AuthRequestResult<T>;
      },
    },
    state: auth,
    register: (domain) => registered.push(domain),
    unregister: () => undefined,
  });
  const values = new Map<string, JsonValue>();
  const applied: string[] = [];
  store.addSource({
    read: () => values,
    apply: (incoming) => {
      for (const [key, value] of incoming) {
        if (key === READING_BOOKMARK_KEY) continue;
        values.set(key, value);
        applied.push(key);
      }
    },
  });
  store.hydrate();
  const sync = async () => {
    swap();
    const domain = registered.at(-1);
    if (domain?.round === undefined) throw new Error("no row domain");
    return domain.round();
  };
  return { store, sync, values, applied, auth, swap };
}

describe("preference sync", () => {
  beforeEach(() => vi.unstubAllGlobals());

  it("two settings changed on two devices both survive", async () => {
    const server = new FakeServer();
    const a = device(server);
    const b = device(server);
    await a.sync();
    await b.sync();
    a.values.set("settings.reading.fontSize", 40);
    a.swap();
    a.store.capture();
    b.values.set("settings.appearance.theme", "dark");
    b.swap();
    b.store.capture();
    await a.sync();
    await b.sync();
    await a.sync();
    expect(a.values.get("settings.appearance.theme")).toBe("dark");
    expect(b.values.get("settings.reading.fontSize")).toBe(40);
  });

  it("a layout change on one device shows on the other", async () => {
    const server = new FakeServer();
    const a = device(server);
    const b = device(server);
    await a.sync();
    await b.sync();
    a.values.set("home.layout", { widgets: ["notes"] });
    a.swap();
    a.store.capture();
    await a.sync();
    await b.sync();
    expect(b.values.get("home.layout")).toEqual({ widgets: ["notes"] });
  });

  it("a new device takes the account's values, then pushes keys the account lacks", async () => {
    const server = new FakeServer();
    const a = device(server);
    await a.sync();
    a.values.set("settings.appearance.theme", "dark");
    a.swap();
    a.store.capture();
    await a.sync();

    const b = device(server);
    b.values.set("settings.appearance.theme", "light");
    b.values.set("settings.reading.mode", "reading");
    await b.sync();
    expect(b.values.get("settings.appearance.theme")).toBe("dark");
    expect(server.rows.get("settings.appearance.theme")?.value).toBe("dark");
    await b.sync();
    expect(server.rows.get("settings.reading.mode")?.value).toBe("reading");
  });

  it("applying a remote value never echoes it back as a new write", async () => {
    const server = new FakeServer();
    const a = device(server);
    const b = device(server);
    await a.sync();
    await b.sync();
    a.values.set("settings.appearance.theme", "dark");
    a.swap();
    a.store.capture();
    await a.sync();
    await b.sync();
    b.swap();
    b.store.capture();
    expect(b.store.dirtyRows()).toEqual([]);
  });

  it("signed out, nothing syncs and sources are never captured", () => {
    const server = new FakeServer();
    const a = device(server, null);
    a.values.set("settings.appearance.theme", "dark");
    a.store.capture();
    expect(a.store.dirtyRows()).toEqual([]);
    expect(server.rows.size).toBe(0);
  });

  it("an expired session pauses and keeps dirty rows", async () => {
    const server = new FakeServer();
    const a = device(server);
    await a.sync();
    a.values.set("settings.appearance.theme", "dark");
    a.swap();
    a.store.capture();
    server.status = 401;
    await expect(a.sync()).rejects.toBeInstanceOf(SyncPausedError);
    expect(a.store.dirtyRows()).toHaveLength(1);
  });
});

describe("reading bookmark", () => {
  beforeEach(() => vi.unstubAllGlobals());

  it("set, replace and undo", async () => {
    const server = new FakeServer();
    const a = device(server);
    expect(currentReadingBookmark(a.store)).toBeNull();
    setReadingBookmark({ surah: 2, ayah: 255, sourceId: null }, a.store);
    expect(currentReadingBookmark(a.store)).toMatchObject({ surah: 2, ayah: 255 });

    const previous = setReadingBookmark({ surah: 18, ayah: 1, sourceId: "sq.nahi" }, a.store);
    expect(previous).toMatchObject({ surah: 2 });
    expect(currentReadingBookmark(a.store)).toMatchObject({ surah: 18, sourceId: "sq.nahi" });

    restoreReadingBookmark(previous, a.store);
    expect(currentReadingBookmark(a.store)).toMatchObject({ surah: 2, ayah: 255 });
  });

  it("undoing the first set clears it, on other devices too", async () => {
    const server = new FakeServer();
    const a = device(server);
    const b = device(server);
    const previous = setReadingBookmark({ surah: 2, ayah: 5, sourceId: null }, a.store);
    await a.sync();
    await b.sync();
    expect(currentReadingBookmark(b.store)).toMatchObject({ surah: 2 });
    restoreReadingBookmark(previous, a.store);
    await a.sync();
    await b.sync();
    expect(currentReadingBookmark(b.store)).toBeNull();
  });

  it("capturing other preferences never overwrites it", async () => {
    const server = new FakeServer();
    const a = device(server);
    await a.sync();
    setReadingBookmark({ surah: 2, ayah: 5, sourceId: null }, a.store);
    a.values.set("settings.reading.mode", "reading");
    a.swap();
    a.store.capture();
    expect(currentReadingBookmark(a.store)).toMatchObject({ surah: 2, ayah: 5 });
  });

  it("a signed-out bookmark folds into the account with its own time", () => {
    const server = new FakeServer();
    const a = device(server, null);
    setReadingBookmark({ surah: 36, ayah: 1, sourceId: null }, a.store);
    expect(a.store.dirtyRows()).toEqual([]);
    a.auth.authenticated = true;
    a.auth.user = { id: 7 };
    a.store.onAuthTransition();
    expect(currentReadingBookmark(a.store)).toMatchObject({ surah: 36 });
    expect(a.store.dirtyRows()).toHaveLength(1);
  });
});
