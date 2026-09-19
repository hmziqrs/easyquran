// SAFETY: Svelte 5 does not export `effect` from the public 'svelte' entry;
// the internal client effect_root + effect pair is the reactive observer
// available to vitest (no component harness in this repo) — root scope is
// required or a detached effect never flushes. flushSync forces synchronous
// flushes so assertions see deterministic effect reruns.
// SAFETY: svelte/internal/client ships without type declarations; the import
// is test-only and typed structurally by usage below.
// @ts-expect-error no declaration file for svelte/internal/client
import { effect, effect_root } from "svelte/internal/client";
import { flushSync } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

import {
  PREPARE_RELOAD,
  PREPARE_RELOAD_EVENT,
  SKIP_WAITING,
  SW_BROADCAST_CHANNEL,
  UPDATE_BROADCAST_CHANNEL,
  UPDATE_TAKEOVER,
  VERSION_QUERY,
  VERSION_RESULT,
} from "$lib/offline/messages";
import { createUpdate } from "$lib/offline/update.svelte";

const RELOAD_GUARD = "easyquran.reload-guard";
const PAINT_KEY = "easyquran.update.waiting";
const DISMISS_KEY = "easyquran.update.dismissed";

const { updatedMock, registerSwMock } = vi.hoisted(() => ({
  updatedMock: { current: false, check: vi.fn<() => Promise<void>>() },
  registerSwMock: vi.fn<() => Promise<ServiceWorkerRegistration | null>>(),
}));

vi.mock("$app/environment", () => ({ browser: true }));
vi.mock("$app/state", () => ({ updated: updatedMock }));
vi.mock("$lib/boot/service-worker", () => ({
  registerServiceWorker: registerSwMock,
}));

interface FakePort {
  // eslint-disable-next-line anti-slop/no-unknown-parameters -- FakePort mirrors the MessagePort surface requestWorkerVersion exercises; payloads are heterogeneous SW replies by design
  postMessage: (msg: unknown) => void;
  close(): void;
  onmessage: ((ev: { data: unknown }) => void) | null;
  start(): void;
}

// Worker double whose postMessage auto-answers VERSION_QUERY on the
// transferred MessageChannel port (same transport as production
// requestWorkerVersion). autoReply=false captures the port so a test can
// deliver a manual (possibly late/stale) VERSION_RESULT.
class FakeWorker {
  readonly posted: Array<{ type: string }> = [];
  lastPort: FakePort | null = null;
  throws = false;

  constructor(
    readonly version: string,
    readonly autoReply = true,
  ) {}

  postMessage(msg: { type: string }, transfer?: Transferable[]): void {
    this.posted.push(msg);
    if (this.throws) throw new Error("worker gone");
    if (msg.type !== VERSION_QUERY) return;
    // SAFETY: requestWorkerVersion transfers [channel.port2]; under the fake
    // MessageChannel below that port is a FakePort instance.
    const port = (transfer?.[0] as FakePort | undefined) ?? null;
    this.lastPort = port;
    if (!this.autoReply) return;
    port?.postMessage({ type: VERSION_RESULT, version: this.version });
  }

  replyWith(version: string): void {
    this.lastPort?.postMessage({ type: VERSION_RESULT, version });
  }
}

class FakeRegistration {
  active: FakeWorker | null = null;
  waiting: FakeWorker | null = null;
  installing: FakeWorker | null = null;
  update = vi.fn<() => Promise<void>>(async () => {});
  readonly #updateFoundListeners = new Set<() => void>();

  addEventListener = (type: string, listener: () => void): void => {
    if (type === "updatefound") this.#updateFoundListeners.add(listener);
  };
  removeEventListener = (type: string, listener: () => void): void => {
    void type;
    this.#updateFoundListeners.delete(listener);
  };
  fireUpdateFound(): void {
    for (const listener of this.#updateFoundListeners) listener();
  }
  // SAFETY: the store only touches waiting/active/installing/(add/remove)
  // EventListener/update; no double can satisfy the full DOM registration
  // interface structurally.
  toRegistration(): ServiceWorkerRegistration {
    // SAFETY: the double implements exactly the registration surface the store touches; the full DOM interface cannot be replicated structurally.
    // eslint-disable-next-line anti-slop/no-chained-type-assertions, anti-slop/require-safety-comment-for-type-assertion -- widening the minimal double through unknown because ServiceWorkerRegistration is a DOM interface
    return this as unknown as ServiceWorkerRegistration;
  }
}

class FakeServiceWorkerContainer {
  controller: FakeWorker | null;
  readonly #controllerChangeListeners = new Set<() => void>();

  constructor(controller: FakeWorker | null = null) {
    this.controller = controller;
  }

  addEventListener = (type: string, listener: () => void): void => {
    if (type === "controllerchange") this.#controllerChangeListeners.add(listener);
  };
  removeEventListener = (type: string, listener: () => void): void => {
    void type;
    this.#controllerChangeListeners.delete(listener);
  };
  fireControllerChange(): void {
    for (const listener of this.#controllerChangeListeners) listener();
  }
  // SAFETY: the store only touches controller + (add/remove)EventListener on
  // the container; the full ServiceWorkerContainer surface is not exercised.
  toContainer(): Partial<ServiceWorkerContainer> {
    // SAFETY: the double implements exactly the container surface the store touches (controller + listeners); the DOM interface cannot be replicated structurally.
    // eslint-disable-next-line anti-slop/no-chained-type-assertions, anti-slop/require-safety-comment-for-type-assertion -- widening the minimal double through unknown because ServiceWorkerContainer is a DOM interface
    return this as unknown as Partial<ServiceWorkerContainer>;
  }
}

class MockBC {
  static byName = new Map<string, Set<MockBC>>();
  readonly name: string;
  closed = false;
  private readonly listeners = new Map<string, Set<(e: { data: unknown }) => void>>();

  constructor(name: string) {
    this.name = name;
    let set = MockBC.byName.get(name);
    if (!set) {
      set = new Set();
      MockBC.byName.set(name, set);
    }
    set.add(this);
  }
  addEventListener(type: string, listener: (e: { data: unknown }) => void): void {
    let set = this.listeners.get(type);
    if (!set) {
      set = new Set();
      this.listeners.set(type, set);
    }
    set.add(listener);
  }
  removeEventListener(type: string, listener: (e: { data: unknown }) => void): void {
    this.listeners.get(type)?.delete(listener);
  }
  postMessage(data: { type: string }): void {
    const set = MockBC.byName.get(this.name);
    if (!set) return;
    for (const other of set) {
      if (other === this || other.closed) continue;
      for (const listener of other.listeners.get("message") ?? []) listener({ data });
    }
  }
  close(): void {
    this.closed = true;
    MockBC.byName.get(this.name)?.delete(this);
  }
}

function setServiceWorker(value: Partial<ServiceWorkerContainer> | null): void {
  // SAFETY: the DOM lib types navigator.serviceWorker as readonly and non-optional; this test-only cast names it optional so delete/defineProperty injection below can toggle it.
  const nav = navigator as { serviceWorker?: unknown };
  if (value === null) {
    try {
      delete nav.serviceWorker;
    } catch {
      return;
    }
    return;
  }
  try {
    Object.defineProperty(navigator, "serviceWorker", {
      value,
      configurable: true,
    });
  } catch {
    return;
  }
}

async function flush(count = 10): Promise<void> {
  for (let i = 0; i < count; i++) await Promise.resolve();
}

function interact(): void {
  window.dispatchEvent(new Event("pointerdown"));
}

interface Harness {
  reg: FakeRegistration;
  container: FakeServiceWorkerContainer;
}

function installHarness(active: FakeWorker | null, waiting: FakeWorker | null): Harness {
  const reg = new FakeRegistration();
  reg.active = active;
  reg.waiting = waiting;
  const container = new FakeServiceWorkerContainer(active);
  registerSwMock.mockResolvedValue(reg.toRegistration());
  setServiceWorker(container.toContainer());
  return { reg, container };
}

// Runs the store's async wiring to completion and hands back the harness
// registration so tests can mutate the live SW graph afterwards.
async function startHarness(
  active: FakeWorker | null,
  waiting: FakeWorker | null,
): Promise<Harness> {
  const harness = installHarness(active, waiting);
  await flush();
  return harness;
}

beforeEach(() => {
  vi.resetModules();
  localStorage.clear();
  sessionStorage.clear();
  MockBC.byName.clear();
  updatedMock.current = false;
  updatedMock.check.mockReset();
  updatedMock.check.mockResolvedValue(undefined);
  registerSwMock.mockReset();
  registerSwMock.mockResolvedValue(null);
  vi.stubGlobal("BroadcastChannel", MockBC);
  vi.stubGlobal("MessageChannel", function FakeMessageChannel() {
    const port1: FakePort = {
      postMessage: () => {},
      close: () => {},
      onmessage: null,
      start: () => {},
    };
    const port2: FakePort = {
      // eslint-disable-next-line anti-slop/no-unknown-parameters -- fake port forwards raw SW reply payloads verbatim; the SUT's parser validates them
      postMessage: (msg: unknown) => {
        port1.onmessage?.({ data: msg });
      },
      close: () => {},
      onmessage: null,
      start: () => {},
    };
    return { port1, port2 };
  });
  Object.defineProperty(window.location, "reload", {
    value: vi.fn<() => void>(),
    configurable: true,
    writable: true,
  });
  setServiceWorker(null);
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  setServiceWorker(null);
});

function reloadCalls(): unknown[] {
  // SAFETY: erasing Location first is required because its reload() method type does not overlap the spy shape below.
  const loc = window.location as unknown;
  // SAFETY: beforeEach replaces window.location.reload with a vi.fn(); this reads that mock's recorded calls.
  return (loc as { reload: { mock?: { calls: unknown[] } } }).reload.mock?.calls ?? [];
}

function expectReload(count = 1): void {
  expect(reloadCalls()).toHaveLength(count);
}

function expectNoReload(): void {
  expect(reloadCalls()).toHaveLength(0);
}

describe("UpdateStore.available getter", () => {
  it("reports availability when there is no service worker and updated.current is set", () => {
    setServiceWorker(null);
    updatedMock.current = true;
    const store = createUpdate();
    expect(store.available).toBe(true);
  });

  it("stays false when a service worker is present but no worker is waiting", async () => {
    installHarness(new FakeWorker("app-1"), null);
    updatedMock.current = true;
    const store = createUpdate();
    store.hydrate();
    await flush();
    expect(store.available).toBe(false);
    store.dispose();
  });
});

describe("UpdateStore.hydrate", () => {
  it("legacy-cleans the stale paint key instead of trusting it", async () => {
    localStorage.setItem(PAINT_KEY, "1");
    const store = createUpdate();
    store.hydrate();
    await flush();
    expect(localStorage.getItem(PAINT_KEY)).toBeNull();
    expect(store.waiting).toBe(false);
    expect(store.available).toBe(false);
    store.dispose();
  });

  it("uses SvelteKit version checks only when service workers are unavailable", async () => {
    const store = createUpdate();
    store.hydrate();
    await flush();

    expect(updatedMock.check).toHaveBeenCalledTimes(1);
    expect(registerSwMock).not.toHaveBeenCalled();

    document.dispatchEvent(new Event("visibilitychange"));
    await flush();
    expect(updatedMock.check).toHaveBeenCalledTimes(2);
    store.dispose();
  });

  it("throttles foreground service-worker update checks to five minutes", async () => {
    let now = 1_000;
    vi.spyOn(Date, "now").mockImplementation(() => now);
    const { reg } = await startHarness(null, null);

    const store = createUpdate();
    store.hydrate();
    await flush();
    document.dispatchEvent(new Event("visibilitychange"));
    await flush();
    expect(reg.update).not.toHaveBeenCalled();
    expect(updatedMock.check).not.toHaveBeenCalled();

    now += 5 * 60_000;
    document.dispatchEvent(new Event("visibilitychange"));
    await flush();
    expect(reg.update).toHaveBeenCalledTimes(1);
    expect(updatedMock.check).not.toHaveBeenCalled();
    store.dispose();
  });

  it("syncs the live waiting graph on focus-return even inside the throttle window", async () => {
    const { reg } = await startHarness(new FakeWorker("app-1"), null);
    const store = createUpdate();
    store.hydrate();
    await flush();
    expect(store.waiting).toBe(false);

    reg.waiting = new FakeWorker("app-2");
    document.dispatchEvent(new Event("visibilitychange"));
    await flush();
    expect(store.waiting).toBe(true);
    store.dispose();
  });
});

describe("UpdateStore version handshake", () => {
  it("queries only the active and waiting workers with VERSION_QUERY", async () => {
    const active = new FakeWorker("app-1");
    const waiting = new FakeWorker("app-2");
    installHarness(active, waiting);
    const store = createUpdate();
    store.hydrate();
    // Interacted tab: the fresh-tab silent adoption path is covered separately.
    interact();
    await flush();

    expect(active.posted).toContainEqual({ type: VERSION_QUERY });
    expect(waiting.posted).toContainEqual({ type: VERSION_QUERY });
    expect(store.available).toBe(true);
    store.dispose();
  });

  it("keeps the banner when the waiting worker's version differs", async () => {
    installHarness(new FakeWorker("app-1"), new FakeWorker("app-2"));
    const store = createUpdate();
    store.hydrate();
    // Interacted tab: banner-flow test, not the silent path.
    interact();
    await flush();
    expect(store.available).toBe(true);
    store.dispose();
  });

  it("hides the banner when the waiting worker's version matches the controller", async () => {
    installHarness(new FakeWorker("app-1"), new FakeWorker("app-1"));
    const store = createUpdate();
    store.hydrate();
    // Interacted tab: banner-flow test, not the silent path.
    interact();
    await flush();
    expect(store.waiting).toBe(true);
    expect(store.available).toBe(false);
    store.dispose();
  });

  it("falls back to the banner while the handshake is indeterminate, including on timeout", async () => {
    vi.useFakeTimers();
    installHarness(new FakeWorker("app-1", false), new FakeWorker("app-2", false));
    const store = createUpdate();
    store.hydrate();
    // Interacted tab: banner-flow test, not the silent path.
    interact();
    await flush();
    expect(store.available).toBe(true);

    await vi.advanceTimersByTimeAsync(2000);
    expect(store.available).toBe(true);
    store.dispose();
  });

  it("downgrades to no-banner and clears dismissal on a late same-version answer", async () => {
    const waiting = new FakeWorker("app-1", false);
    installHarness(new FakeWorker("app-1"), waiting);
    const store = createUpdate();
    store.hydrate();
    // Interacted tab: banner-flow test, not the silent path.
    interact();
    await flush();

    // Handshake pending -> indeterminate -> banner; user dismisses under the
    // unknown-version sentinel.
    expect(store.available).toBe(true);
    store.dismiss();
    expect(store.dismissed).toBe(true);
    expect(sessionStorage.getItem(DISMISS_KEY)).toBe("*");

    waiting.replyWith("app-1");
    await flush();
    expect(store.available).toBe(false);
    expect(store.dismissed).toBe(false);
    expect(sessionStorage.getItem(DISMISS_KEY)).toBeNull();
    store.dispose();
  });

  it("discards a stale VERSION_RESULT from a superseded waiting worker", async () => {
    const active = new FakeWorker("app-1");
    const w1 = new FakeWorker("app-9", false);
    const { reg } = installHarness(active, w1);
    const store = createUpdate();
    store.hydrate();
    // Interacted tab: banner-flow test, not the silent path.
    interact();
    await flush();
    expect(store.available).toBe(true);

    const w2 = new FakeWorker("app-9", false);
    reg.waiting = w2;
    reg.fireUpdateFound();
    await flush();

    // w1's late answer matches the controller version and would hide the
    // banner; the identity guard must discard it and recompute from the
    // live graph (w2 still unanswered -> indeterminate -> banner).
    w1.replyWith("app-1");
    await flush();
    expect(store.available).toBe(true);
    store.dispose();
  });

  it("re-queries the waiting worker when it is replaced and resets dismissal", async () => {
    const active = new FakeWorker("app-1");
    const { reg } = installHarness(active, new FakeWorker("app-2"));
    const store = createUpdate();
    store.hydrate();
    // Interacted tab: banner-flow test, not the silent path.
    interact();
    await flush();
    store.dismiss();
    expect(store.dismissed).toBe(true);
    expect(sessionStorage.getItem(DISMISS_KEY)).toBe("v:app-2");

    const w2 = new FakeWorker("app-3");
    reg.waiting = w2;
    reg.fireUpdateFound();
    await flush();

    expect(store.dismissed).toBe(false);
    expect(sessionStorage.getItem(DISMISS_KEY)).toBeNull();
    expect(w2.posted).toContainEqual({ type: VERSION_QUERY });
    expect(store.available).toBe(true);
    store.dispose();
  });

  it("keeps a per-version dismissal across a re-hydrate in the same session", async () => {
    installHarness(new FakeWorker("app-1"), new FakeWorker("app-2"));
    const first = createUpdate();
    first.hydrate();
    // Interacted tab: banner-flow test, not the silent path.
    interact();
    await flush();
    first.dismiss();
    expect(first.dismissed).toBe(true);
    expect(sessionStorage.getItem(DISMISS_KEY)).toBe("v:app-2");
    first.dispose();

    const second = createUpdate();
    second.hydrate();
    interact();
    await flush();
    expect(second.available).toBe(true);
    expect(second.dismissed).toBe(true);
    second.dispose();
  });
});

describe("UpdateStore fresh-tab silent adoption", () => {
  function noBroadcastReceiver() {
    const received: unknown[] = [];
    const spy = new MockBC(UPDATE_BROADCAST_CHANNEL);
    spy.addEventListener("message", (e) => received.push(e.data));
    return { received };
  }

  it("silently reloads a fresh tab once with its own guard and NO broadcast", async () => {
    vi.useFakeTimers();
    const waiting = new FakeWorker("app-2");
    installHarness(new FakeWorker("app-1"), waiting);
    const { received } = noBroadcastReceiver();

    const store = createUpdate();
    store.hydrate();

    // Pending attempt window: suppressed banner, guard armed, skip sent.
    await flush();
    expectNoReload();
    expect(store.available).toBe(false);
    expect(waiting.posted).toContainEqual({ type: SKIP_WAITING });
    expect(sessionStorage.getItem(RELOAD_GUARD)).toBe("1");
    expect(received).toHaveLength(0);

    await vi.runAllTimersAsync();
    expectReload(1);
    expect(received).toHaveLength(0);
    store.dispose();
  });

  it("aborts the silent attempt when the tab already interacted (banner instead)", async () => {
    const waiting = new FakeWorker("app-2");
    installHarness(new FakeWorker("app-1"), waiting);
    const store = createUpdate();
    store.hydrate();
    interact();
    await flush();

    expectNoReload();
    expect(waiting.posted).not.toContainEqual({ type: SKIP_WAITING });
    expect(sessionStorage.getItem(RELOAD_GUARD)).toBeNull();
    expect(store.available).toBe(true);
    store.dispose();
  });

  it("aborts on a ghost click during the final recheck: disarms and shows the banner", async () => {
    vi.useFakeTimers();
    installHarness(new FakeWorker("app-1"), new FakeWorker("app-2"));
    const store = createUpdate();
    store.hydrate();
    await flush();
    expect(sessionStorage.getItem(RELOAD_GUARD)).toBe("1");

    interact();
    await vi.runAllTimersAsync();

    expectNoReload();
    expect(sessionStorage.getItem(RELOAD_GUARD)).toBeNull();
    expect(store.available).toBe(true);
    store.dispose();
  });

  // Regression (R6 suppression-leak): the banner must come back for a
  // REACTIVE consumer ($derived/effect), not just an imperative getter call.
  // `#silentTarget` used to be a plain field read inside `available`, so
  // nulling it on the ghost-click abort path never invalidated the derived —
  // imperative `store.available` reads passed while real UI stayed hidden.
  it("reactively un-hides the banner when a ghost click aborts the silent attempt", async () => {
    vi.useFakeTimers();
    installHarness(new FakeWorker("app-1"), new FakeWorker("app-2"));
    const store = createUpdate();
    const seen: boolean[] = [];
    const stop = effect_root(() => {
      effect(() => {
        seen.push(store.available);
      });
    });
    flushSync();
    // Fresh-tab suppressed state settled (timers frozen): banner hidden for
    // the reactive consumer while the silent attempt is pending.
    store.hydrate();
    await flush();
    flushSync();
    expect(seen.at(-1)).toBe(false);

    interact();
    await flush();
    await vi.runAllTimersAsync();
    flushSync();
    expectNoReload();
    // The reactive consumer itself must now see the banner.
    expect(seen.at(-1)).toBe(true);
    stop();
    store.dispose();
  });

  it("releases silent suppression when the targeted waiting worker is replaced", async () => {
    vi.useFakeTimers();
    const active = new FakeWorker("app-1");
    const { reg } = installHarness(active, new FakeWorker("app-2"));
    const store = createUpdate();
    store.hydrate();
    await flush();
    expect(store.available).toBe(false);

    reg.waiting = new FakeWorker("app-3", false);
    reg.fireUpdateFound();
    expect(store.available).toBe(true);

    await vi.runAllTimersAsync();
    expectNoReload();
    store.dispose();
  });

  it("dispose() aborts an in-flight silent attempt without reloading", async () => {
    vi.useFakeTimers();
    installHarness(new FakeWorker("app-1"), new FakeWorker("app-2"));
    const store = createUpdate();
    store.hydrate();
    await flush();
    expect(sessionStorage.getItem(RELOAD_GUARD)).toBe("1");

    store.dispose();
    await vi.runAllTimersAsync();

    expectNoReload();
    expect(sessionStorage.getItem(RELOAD_GUARD)).toBeNull();
  });

  it("never silent-reloads once the guard was already armed at hydrate", async () => {
    vi.useFakeTimers();
    sessionStorage.setItem(RELOAD_GUARD, "1");
    const waiting = new FakeWorker("app-2");
    installHarness(new FakeWorker("app-1"), waiting);
    const store = createUpdate();
    store.hydrate();
    await flush();
    await vi.runAllTimersAsync();

    expectNoReload();
    expect(waiting.posted).not.toContainEqual({ type: SKIP_WAITING });
    expect(store.available).toBe(true);
    store.dispose();
  });

  it("silently reloads the no-SW fallback once and arms the same guard (F6)", async () => {
    setServiceWorker(null);
    updatedMock.current = true;
    const store = createUpdate();
    store.hydrate();
    await flush();

    expectReload(1);
    expect(sessionStorage.getItem(RELOAD_GUARD)).toBe("1");
    store.dispose();

    const second = createUpdate();
    second.hydrate();
    await flush();
    expectReload(1);
    second.dispose();
  });
});

describe("UpdateStore.apply", () => {
  it("arms the reload guard, broadcasts PREPARE_RELOAD, and skips the waiting worker", async () => {
    const waiting = new FakeWorker("app-2");
    installHarness(new FakeWorker("app-1"), waiting);
    const dispatchSpy = vi.spyOn(window, "dispatchEvent");

    const store = createUpdate();
    store.hydrate();
    interact();
    await flush();
    store.apply();

    expect(waiting.posted).toContainEqual({ type: SKIP_WAITING });
    expect(sessionStorage.getItem(RELOAD_GUARD)).toBe("1");
    // SAFETY: dispatchSpy wraps window.dispatchEvent(event: Event), so every recorded call argument is an Event.
    expect(
      dispatchSpy.mock.calls.some(([event]) => (event as Event).type === PREPARE_RELOAD_EVENT),
    ).toBe(true);
    expectNoReload();
    store.dispose();
  });

  it("does not let SvelteKit version state override service-worker ownership", async () => {
    installHarness(new FakeWorker("app-1"), null);
    updatedMock.current = true;

    const store = createUpdate();
    store.hydrate();
    interact();
    await flush();
    store.apply();

    expectNoReload();
    store.dispose();
  });

  it("reloads directly from SvelteKit version state without service-worker support", async () => {
    setServiceWorker(null);
    updatedMock.current = true;
    const store = createUpdate();
    store.hydrate();
    interact();
    await flush();
    store.apply();
    expectReload(1);
    // apply() is consented user intent: it must NOT arm the silent guard.
    expect(sessionStorage.getItem(RELOAD_GUARD)).toBeNull();
    store.dispose();
  });
});

describe("UpdateStore reload-guard via BroadcastChannel", () => {
  it("reloads when PREPARE_RELOAD arms the guard and UPDATE_TAKEOVER then fires", () => {
    setServiceWorker(null);
    const store = createUpdate();
    store.hydrate();

    const updateSender = new MockBC(UPDATE_BROADCAST_CHANNEL);
    updateSender.postMessage({ type: PREPARE_RELOAD });
    const swSender = new MockBC(SW_BROADCAST_CHANNEL);
    swSender.postMessage({ type: UPDATE_TAKEOVER });

    expectReload(1);
    expect(store.waiting).toBe(false);
  });

  it("ignores UPDATE_TAKEOVER when the guard was never armed (no reload loop)", () => {
    setServiceWorker(null);
    const store = createUpdate();
    store.hydrate();

    const swSender = new MockBC(SW_BROADCAST_CHANNEL);
    swSender.postMessage({ type: UPDATE_TAKEOVER });

    expectNoReload();
  });
});

describe("UpdateStore.dispose", () => {
  it("releases the registration so the instance can be re-hydrated", async () => {
    installHarness(null, null);

    const store = createUpdate();
    store.hydrate();
    await flush();
    store.dispose();

    expect(registerSwMock).toHaveBeenCalledTimes(1);
    const second = createUpdate();
    second.hydrate();
    await flush();
    expect(registerSwMock).toHaveBeenCalledTimes(2);
  });
});

describe("UpdateStore controllerchange", () => {
  it("resets dismissal on a new controller and evaluates the armed guard", async () => {
    const { container } = installHarness(new FakeWorker("app-1"), new FakeWorker("app-2"));
    const store = createUpdate();
    store.hydrate();
    interact();
    await flush();
    store.apply();
    expect(sessionStorage.getItem(RELOAD_GUARD)).toBe("1");

    // Activation lands: the page is claimed by the new controller.
    container.controller = new FakeWorker("app-2");
    container.fireControllerChange();

    expectReload(1);
    store.dispose();
  });
});
