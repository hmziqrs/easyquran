import { browser } from "$app/env";

import { authClient, type AuthRequestResult, type UnsafeRequestInit } from "#lib/auth/auth-client.js";
import { authState } from "#lib/auth/auth-state.svelte.js";
import { deviceIdFor } from "#lib/device/identity.js";
import {
  asArray,
  asNumber,
  asObject,
  asString,
  readJSON,
  trailingDebounce,
  tryWriteJSON,
} from "#lib/storage/index.js";
import type { JsonValue } from "#lib/storage/decoders.js";
import { createClock, localClockStorage, type SyncClock } from "#lib/sync/clock.js";
import {
  registerDomain,
  syncEngine,
  unregisterDomain,
  SyncPausedError,
  type RegisteredSyncDomain,
  type RowRoundResult,
} from "#lib/sync/index.js";
import { canonicalJson, compareStamps } from "#lib/sync/merge.js";
import { SYNC_CLIENT, throwForFailure } from "#lib/sync/round-status.js";

/**
 * Synced preferences: one register per key (contracts/sync.md), so two settings
 * changed on two devices both survive. A source owns its live values and this store
 * owns the registers. A first full pull lands before local values are captured, so
 * the account's values win on a new device, and keys the account lacks are pushed.
 * Signed out, nothing syncs and only the reading bookmark is held here.
 */

export const READING_BOOKMARK_KEY = "reading.bookmark";
const SYNC_PATH = "/preference/v1/sync";
const SYNC_PAGE = 200;
const LOCAL_OWNER = "local";
const STORE_VERSION = 1;
const CAPTURE_DEBOUNCE_MS = 1_000;

export interface PreferenceRow {
  readonly key: string;
  readonly value: JsonValue;
  readonly stamp: string | null;
  readonly dirty: boolean;
  readonly rejected: boolean;
}

/** A store that owns some keys: settings, the home layout. */
export interface PreferenceSource {
  /** Current values by key. Reading reactive state here lets an effect re-capture. */
  read(): ReadonlyMap<string, JsonValue>;
  /** Show values that arrived from another device. */
  apply(values: ReadonlyMap<string, JsonValue>): void;
}

export interface PreferencesAuthLike {
  unsafeRequest<T>(path: string, init: UnsafeRequestInit): Promise<AuthRequestResult<T>>;
}

export interface PreferencesStateLike {
  readonly authenticated: boolean;
  readonly user: { readonly id: number } | null;
}

export interface PreferencesDeps {
  readonly engine?: {
    flush(domain?: string): Promise<void>;
    start(): () => void;
    setStorageDegraded?(on: boolean): void;
  };
  readonly auth?: PreferencesAuthLike;
  readonly state?: PreferencesStateLike;
  readonly register?: (domain: RegisteredSyncDomain) => void;
  readonly unregister?: (name: string) => void;
}

interface RoundRow {
  readonly key: string;
  readonly value: JsonValue;
  readonly stamp: string | null;
}

interface RoundResult {
  readonly id: string;
  readonly status: string;
  readonly stamp: string | null;
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is one unparsed row (wire or storage); this decoder is its boundary parser.
function decodeRow(raw: unknown): PreferenceRow | undefined {
  const obj = asObject(raw);
  const key = asString(obj?.key);
  if (obj === undefined || key === undefined) return undefined;
  return {
    key,
    value: obj.value ?? null,
    stamp: asString(obj.stamp) ?? null,
    dirty: obj.dirty === true,
    rejected: obj.rejected === true,
  };
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is one unparsed per-row result; this decoder is its boundary parser.
function decodeResult(raw: unknown): RoundResult | undefined {
  const obj = asObject(raw);
  const id = asString(obj?.id);
  const status = asString(obj?.status);
  if (id === undefined || status === undefined) return undefined;
  return { id, status, stamp: asString(asObject(obj?.stamps)?.row) ?? null };
}

function storageKey(owner: string): string {
  return `easyquran.preferences.${owner}`;
}

function sameValue(a: JsonValue, b: JsonValue): boolean {
  return canonicalJson(a) === canonicalJson(b);
}

export class PreferencesStore {
  readonly #engine: NonNullable<PreferencesDeps["engine"]>;
  readonly #auth: PreferencesAuthLike;
  readonly #state: PreferencesStateLike;
  readonly #register: (domain: RegisteredSyncDomain) => void;
  readonly #unregister: (name: string) => void;

  #rows = $state.raw<ReadonlyMap<string, PreferenceRow>>(new Map());
  #owner = LOCAL_OWNER;
  #since = 0;
  #epoch: string | null = null;
  /** The account's values have landed here, so local values may now be captured. */
  #baselined = false;
  #clock: SyncClock | null = null;
  #registered: string | null = null;
  #sources: PreferenceSource[] = [];
  #applying = false;
  #capture = trailingDebounce(() => this.capture(), CAPTURE_DEBOUNCE_MS);

  constructor(deps: PreferencesDeps = {}) {
    this.#engine = deps.engine ?? syncEngine;
    this.#auth = deps.auth ?? authClient;
    this.#state = deps.state ?? authState;
    this.#register = deps.register ?? registerDomain;
    this.#unregister = deps.unregister ?? unregisterDomain;
  }

  addSource(source: PreferenceSource): void {
    this.#sources.push(source);
  }

  /** A source's values changed; capture once they settle. */
  noteChanged(): void {
    if (this.#applying || this.#accountId() === null) return;
    this.#capture.schedule();
  }

  get(key: string): JsonValue {
    return this.#rows.get(key)?.value ?? null;
  }

  /** An explicit write from a store that keeps its value here. */
  set(key: string, value: JsonValue): void {
    this.#put(key, value);
    this.#afterWrite();
  }

  /** Login, logout, expiry: swap owners, and fold the signed-out rows into the account. */
  onAuthTransition(): void {
    if (!browser) return;
    const next = this.#resolveOwner();
    if (next === this.#owner && (next === LOCAL_OWNER || this.#registered !== null)) return;
    const absorbing = this.#owner === LOCAL_OWNER && next !== LOCAL_OWNER ? this.#rows : null;
    this.#load(next);
    if (absorbing !== null) this.#absorb(absorbing);
    const accountId = this.#accountId();
    if (accountId !== null) this.#registerFor(accountId);
  }

  hydrate(): void {
    this.#load(this.#resolveOwner());
    this.onAuthTransition();
  }

  /** Rows the next round sends. */
  dirtyRows(): readonly RoundRow[] {
    return [...this.#rows.values()]
      .filter((row) => row.dirty && !row.rejected)
      .slice(0, SYNC_PAGE)
      .map(({ key, value, stamp }) => ({ key, value, stamp }));
  }

  /** Stamp every source value that differs from its register. */
  capture(): void {
    if (this.#accountId() === null || !this.#baselined) return;
    let changed = false;
    for (const source of this.#sources) {
      for (const [key, value] of source.read()) {
        const row = this.#rows.get(key);
        if (row !== undefined && sameValue(row.value, value)) continue;
        this.#put(key, value);
        changed = true;
      }
    }
    if (changed) this.#afterWrite();
  }

  applyRound(raw: JsonValue, sent: readonly RoundRow[]): RowRoundResult {
    const obj = asObject(raw);
    if (obj === undefined || !Array.isArray(obj.preferences)) {
      throw new Error("preference sync returned an unreadable round");
    }
    const serverTime = asNumber(obj.serverTime, 0, Number.MAX_SAFE_INTEGER);
    if (serverTime !== undefined) this.#clock?.onServerTime(serverTime);
    const rows = new Map(this.#rows);
    const sentByKey = new Map(sent.map((row) => [row.key, row]));

    for (const result of asArray(obj.results, decodeResult)) {
      this.#applyResult(result, sentByKey.get(result.id), rows);
    }
    const arrived = new Map<string, JsonValue>();
    for (const incoming of asArray(obj.preferences, decodeRow)) {
      this.#clock?.observe(incoming.stamp);
      const mine = rows.get(incoming.key);
      if (mine !== undefined && compareStamps(mine.stamp, incoming.stamp) >= 0) continue;
      rows.set(incoming.key, { ...incoming, dirty: false, rejected: false });
      arrived.set(incoming.key, incoming.value);
    }

    const latest = asNumber(obj.latestRev, 0, Number.MAX_SAFE_INTEGER) ?? 0;
    const epoch = asString(obj.syncEpoch) ?? null;
    const restored = (this.#epoch !== null && epoch !== this.#epoch) || this.#since > latest;
    if (restored) {
      // Rows and `rev` went backwards: push everything this device holds again.
      for (const [key, row] of rows) rows.set(key, { ...row, dirty: row.stamp !== null });
    }
    const more = obj.more === true;
    this.#since = restored ? 0 : (asNumber(obj.rev, 0, Number.MAX_SAFE_INTEGER) ?? this.#since);
    this.#epoch = epoch ?? this.#epoch;
    this.#rows = rows;
    this.#persist();
    this.#applyToSources(arrived);
    if (!more && !restored && !this.#baselined) {
      this.#baselined = true;
      this.capture();
    }
    const progressed = sent.length >= SYNC_PAGE && this.dirtyRows().length > 0;
    return { more: more || restored || progressed };
  }

  #applyResult(
    result: RoundResult,
    sent: RoundRow | undefined,
    rows: Map<string, PreferenceRow>,
  ): void {
    const mine = rows.get(result.id);
    if (mine === undefined || sent === undefined || mine.stamp !== sent.stamp) return;
    if (result.status === "rejected") {
      rows.set(result.id, { ...mine, rejected: true });
      return;
    }
    if (result.status === "capped") return;
    // A stamp the server rewrote is below ours: adopt it. One it beat arrives with the row.
    const lower = result.stamp !== null && mine.stamp !== null && result.stamp < mine.stamp;
    rows.set(result.id, { ...mine, stamp: lower ? result.stamp : mine.stamp, dirty: false });
  }

  #applyToSources(values: ReadonlyMap<string, JsonValue>): void {
    if (values.size === 0) return;
    this.#applying = true;
    try {
      for (const source of this.#sources) source.apply(values);
    } finally {
      this.#applying = false;
    }
  }

  #put(key: string, value: JsonValue): void {
    const account = this.#accountId() !== null;
    const row: PreferenceRow = {
      key,
      value,
      stamp: account ? (this.#clock?.stamp() ?? null) : null,
      dirty: account,
      rejected: false,
    };
    this.#rows = new Map(this.#rows).set(key, row);
  }

  #afterWrite(): void {
    this.#persist();
    const accountId = this.#accountId();
    if (accountId === null || this.#registered === null) return;
    void this.#engine.flush(this.#registered).catch(() => undefined);
  }

  #persist(): void {
    if (!browser) return;
    const landed = tryWriteJSON(storageKey(this.#owner), {
      v: STORE_VERSION,
      rows: [...this.#rows.values()],
      since: this.#since,
      epoch: this.#epoch,
      baselined: this.#baselined,
    });
    this.#engine.setStorageDegraded?.(!landed);
  }

  #load(owner: string): void {
    this.#unregisterCurrent();
    this.#owner = owner;
    this.#clock =
      owner === LOCAL_OWNER ? null : createClock(deviceIdFor(owner), localClockStorage(owner));
    const stored = asObject(readJSON(storageKey(owner)));
    const rows = asArray(stored?.rows, decodeRow);
    this.#rows = new Map(rows.map((row) => [row.key, row]));
    this.#since = asNumber(stored?.since, 0, Number.MAX_SAFE_INTEGER) ?? 0;
    this.#epoch = asString(stored?.epoch) ?? null;
    this.#baselined = stored?.baselined === true;
  }

  /** The signed-out reading bookmark joins the account with its own time. */
  #absorb(local: ReadonlyMap<string, PreferenceRow>): void {
    const bookmark = local.get(READING_BOOKMARK_KEY);
    if (bookmark === undefined || bookmark.value === null) return;
    const setAt = asNumber(asObject(bookmark.value)?.setAt, 0, Number.MAX_SAFE_INTEGER) ?? 0;
    const row: PreferenceRow = {
      ...bookmark,
      stamp: this.#clock?.stampAt(setAt) ?? null,
      dirty: true,
    };
    const mine = this.#rows.get(READING_BOOKMARK_KEY);
    if (mine === undefined || compareStamps(mine.stamp, row.stamp) < 0) {
      this.#rows = new Map(this.#rows).set(READING_BOOKMARK_KEY, row);
    }
    tryWriteJSON(storageKey(LOCAL_OWNER), { v: STORE_VERSION, rows: [] });
    this.#persist();
  }

  #resolveOwner(): string {
    if (this.#state.authenticated && this.#state.user !== null) return String(this.#state.user.id);
    return LOCAL_OWNER;
  }

  #accountId(): number | null {
    if (this.#owner === LOCAL_OWNER) return null;
    const id = Number(this.#owner);
    return Number.isSafeInteger(id) && id > 0 ? id : null;
  }

  #unregisterCurrent(): void {
    if (this.#registered === null) return;
    this.#unregister(this.#registered);
    this.#registered = null;
  }

  #registerFor(ownerId: number): void {
    const name = `preferences.user:${ownerId}`;
    this.#registered = name;
    const isCurrent = (): boolean => this.#registered === name && this.#accountId() === ownerId;
    this.#register({
      name,
      sync: async () => {
        throw new Error("preferences sync through rounds, not the outbox");
      },
      applyServer: () => undefined,
      round: async (signal) => {
        if (!isCurrent() || signal?.aborted) throw new SyncPausedError("preference sync paused");
        const sent = this.dirtyRows();
        const res = await this.#auth.unsafeRequest<JsonValue>(SYNC_PATH, {
          method: "POST",
          body: { expectedUserId: ownerId, client: SYNC_CLIENT, since: this.#since, mutations: sent },
          signal,
        });
        if (!isCurrent()) throw new SyncPausedError("preference sync paused");
        if (!res.ok) {
          throwForFailure("preference sync", res, {
            mismatchMessage: "Preference sync account changed",
          });
        }
        return this.applyRound(res.data, sent);
      },
    });
    this.#engine.start();
    void this.#engine.flush(name).catch(() => undefined);
  }
}

export const preferences: PreferencesStore = new PreferencesStore();
