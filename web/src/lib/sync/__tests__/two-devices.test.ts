import { beforeEach, describe, expect, it, vi } from "vite-plus/test";

import type { UnsafeRequestInit } from "#lib/auth/auth-client.js";
import { asString, type JsonRecord } from "#lib/storage/decoders.js";
import { noteLeaving } from "#lib/sessions/entry.js";
import { SessionsStore } from "#lib/sessions/store.svelte.js";
import type { RegisteredSyncDomain } from "#lib/sync/index.js";

import { mergeThread, type JsonObject } from "../merge";

vi.mock("$app/env", () => ({ browser: true }));
vi.mock("$app/env/public", () => ({
  PUBLIC_API_BASE_URL: "https://eq.test/api",
  PUBLIC_QURAN_API_BASE: undefined,
  PUBLIC_ENV: undefined,
  PUBLIC_FCM_VAPID_KEY: undefined,
}));

type Row = JsonRecord;

/** One server holding rows as registers, merged by the same core the clients use. */
class FakeServer {
  rev = 0;
  rows = new Map<string, { registers: JsonObject; updatedAt: string; rev: number }>();

  handle(body: Row): Row {
    const mutations: Row[] = JSON.parse(JSON.stringify(body.mutations ?? []));
    const results = mutations.map((m) => this.#apply(m));
    const since = Number(body.since ?? 0);
    const rows = [...this.rows.entries()]
      .filter(([, row]) => row.rev > since)
      .map(([id, row]) => wireOf(id, row));
    return JSON.parse(
      JSON.stringify({
        serverTime: Date.now(),
        syncEpoch: "1",
        results,
        sessions: rows,
        progress: [],
        rev: this.rev,
        latestRev: this.rev,
        more: false,
        tombstoneFloor: 0,
      }),
    );
  }

  #apply(m: Row) {
    if (m.kind !== "session.upsert") return { id: `progress:${JSON.stringify(m.surah)}`, status: "applied" };
    const id = asString(m.id) ?? "";
    const incoming: JsonObject = {
      id,
      identity: {
        stamp: m.identityStamp ?? null,
        value: {
          origin: m.origin ?? null,
          originRef: m.originRef ?? null,
          scope: m.scope ?? null,
          boundEnd: m.boundEnd ?? null,
          startedAt: m.startedAt ?? null,
        },
      },
      cursor: {
        stamp: m.cursorStamp ?? null,
        value: {
          surah: m.surah ?? null,
          ayah: m.ayah ?? null,
          localPage: m.localPage ?? null,
          ratio: m.ratio ?? null,
          sourceId: m.sourceId ?? null,
          device: m.device ?? null,
          deviceId: m.deviceId ?? null,
        },
      },
      lifecycle: {
        stamp: m.lifecycleStamp ?? null,
        value: { state: m.state ?? null, endedAt: m.endedAt ?? null, endReason: m.endReason ?? null },
      },
    };
    const current = this.rows.get(id);
    const registers = current === undefined ? incoming : mergeThread(current.registers, incoming);
    this.rev += 1;
    this.rows.set(id, { registers, updatedAt: asString(m.updatedAt) ?? "", rev: this.rev });
    const stampOf = (name: string) => JSON.parse(JSON.stringify(registers[name])).stamp;
    return {
      id,
      status: "applied",
      stamps: {
        identity: stampOf("identity"),
        cursor: stampOf("cursor"),
        lifecycle: stampOf("lifecycle"),
      },
    };
  }
}

function wireOf(id: string, row: { registers: JsonObject; updatedAt: string; rev: number }) {
  const parts = JSON.parse(JSON.stringify(row.registers));
  return {
    id,
    rev: row.rev,
    updatedAt: row.updatedAt,
    ...parts.identity.value,
    ...parts.cursor.value,
    ...parts.lifecycle.value,
    identityStamp: parts.identity.stamp,
    cursorStamp: parts.cursor.stamp,
    lifecycleStamp: parts.lifecycle.stamp,
  };
}

/** A device: its own browser storage, and a real store on top of it. */
function device(server: FakeServer) {
  const local = new Map<string, string>();
  const session = new Map<string, string>();
  const area = (map: Map<string, string>) => ({
    getItem: (key: string) => map.get(key) ?? null,
    setItem: (key: string, value: string) => void map.set(key, value),
    removeItem: (key: string) => void map.delete(key),
    clear: () => map.clear(),
    key: (index: number) => [...map.keys()][index] ?? null,
    get length() {
      return map.size;
    },
  });
  const inside = <T>(run: () => T): T => {
    vi.stubGlobal("localStorage", area(local));
    vi.stubGlobal("sessionStorage", area(session));
    return run();
  };
  const registered: RegisteredSyncDomain[] = [];
  const store = inside(() => {
    const created = new SessionsStore({
      engine: { flush: async () => undefined, start: () => () => undefined },
      auth: {
        unsafeRequest: async <T>(_path: string, init: UnsafeRequestInit) => {
          // SAFETY: test double — the domain always sends an object body.
          const data = server.handle(init.body as Row);
          // SAFETY: test double — the domain decodes this body as unknown.
          const res = { ok: true, status: 200, data: data as T, error: null, rotated: false };
          return res;
        },
        get: async <T>(): Promise<{ ok: boolean; data?: T | null }> => ({ ok: false }),
      },
      state: { authenticated: true, user: { id: 7 } },
      register: (domain) => registered.push(domain),
      unregister: () => undefined,
    });
    created.onAuthTransition();
    return created;
  });
  const sync = async () => {
    const domain = registered.at(-1);
    if (domain?.round === undefined) throw new Error("no row domain");
    const round = domain.round.bind(domain);
    await inside(() => round());
  };
  return { store, inside, sync };
}

const SCOPE = { scope: "surah:2", surah: 2, ayah: 1, exact: false, sourceId: null };

describe("two real stores against one server", () => {
  beforeEach(() => noteLeaving(false));

  async function shared(server: FakeServer) {
    const a = device(server);
    const b = device(server);
    const id = a.inside(() => {
      const { thread } = a.store.enter(SCOPE);
      a.store.cursor(2, 5, null);
      return thread.id;
    });
    await a.sync();
    await b.sync();
    expect(b.store.open.map((t) => t.id)).toEqual([id]);
    return { a, b, id };
  }

  it("a thread ended on one device stays ended when a stale cursor lands", async () => {
    const { a, b, id } = await shared(new FakeServer());
    a.inside(() => a.store.dismiss(id));
    await a.sync();
    b.inside(() => {
      b.store.enter(SCOPE);
      b.store.cursor(2, 40, null);
    });
    await b.sync();
    await a.sync();
    await b.sync();
    expect(a.inside(() => a.store.open)).toEqual([]);
    expect(b.inside(() => b.store.open)).toEqual([]);
  });

  it("the later cursor wins on both devices, and neither stays dirty", async () => {
    const { a, b, id } = await shared(new FakeServer());
    a.inside(() => a.store.enter(SCOPE));
    a.inside(() => a.store.cursor(2, 7, null));
    await a.sync();
    await b.sync();
    b.inside(() => b.store.enter(SCOPE));
    b.inside(() => b.store.cursor(2, 9, null));
    await b.sync();
    await a.sync();
    expect(a.store.open.find((t) => t.id === id)?.ayah).toBe(9);
    expect(b.store.open.find((t) => t.id === id)?.ayah).toBe(9);
    expect(a.inside(() => a.store.dirtyRows())).toEqual([]);
    expect(b.inside(() => b.store.dirtyRows())).toEqual([]);
  });
});
