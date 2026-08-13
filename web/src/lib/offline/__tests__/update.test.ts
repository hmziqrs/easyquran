import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

interface FakeRegistration {
  waiting: { postMessage: (message: { type: string }) => void } | null;
  addEventListener: () => void;
  removeEventListener: () => void;
  update: () => void;
}

const { updatedMock, registerSwMock } = vi.hoisted(() => ({
  updatedMock: { current: false, check: vi.fn<() => Promise<void>>() },
  registerSwMock: vi.fn<() => Promise<FakeRegistration | null>>(),
}));

vi.mock("$app/environment", () => ({ browser: true }));
vi.mock("$app/state", () => ({ updated: updatedMock }));
vi.mock("$lib/boot/service-worker", () => ({
  registerServiceWorker: registerSwMock,
}));

import {
  PREPARE_RELOAD,
  PREPARE_RELOAD_EVENT,
  SKIP_WAITING,
  SW_BROADCAST_CHANNEL,
  UPDATE_BROADCAST_CHANNEL,
  UPDATE_TAKEOVER,
} from "$lib/offline/messages";
import { createUpdate } from "$lib/offline/update.svelte";

const RELOAD_GUARD = "easyquran.reload-guard";
const PAINT_KEY = "easyquran.update.waiting";

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
  Object.defineProperty(window.location, "reload", {
    value: vi.fn<() => void>(),
    configurable: true,
    writable: true,
  });
  setServiceWorker(null);
});

afterEach(() => {
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

  it("falls through to #waiting when a service worker is present", () => {
    setServiceWorker({ addEventListener: vi.fn(), removeEventListener: vi.fn() });
    updatedMock.current = true;
    const store = createUpdate();
    expect(store.available).toBe(false);
  });
});

describe("UpdateStore.hydrate", () => {
  it("marks the store waiting when the paint flag is already set in storage", () => {
    localStorage.setItem(PAINT_KEY, "1");
    const store = createUpdate();
    store.hydrate();
    expect(store.waiting).toBe(true);
  });
});

describe("UpdateStore.apply", () => {
  it("arms the reload guard, broadcasts PREPARE_RELOAD, and skips the waiting worker", async () => {
    const waiting = { postMessage: vi.fn() };
    const reg = {
      waiting,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      update: vi.fn(),
    };
    registerSwMock.mockResolvedValue(reg);
    setServiceWorker({
      controller: null,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    });
    const dispatchSpy = vi.spyOn(window, "dispatchEvent");

    const store = createUpdate();
    store.hydrate();
    await flush();
    store.apply();

    expect(waiting.postMessage).toHaveBeenCalledWith({ type: SKIP_WAITING });
    expect(sessionStorage.getItem(RELOAD_GUARD)).toBe("1");
    // SAFETY: dispatchSpy wraps window.dispatchEvent(event: Event), so every recorded call argument is an Event.
    expect(
      dispatchSpy.mock.calls.some(([event]) => (event as Event).type === PREPARE_RELOAD_EVENT),
    ).toBe(true);
    expectNoReload();
  });

  it("reloads directly when there is no waiting worker but updated.current is set", async () => {
    registerSwMock.mockResolvedValue({
      waiting: null,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      update: vi.fn(),
    });
    setServiceWorker({
      controller: null,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    });
    updatedMock.current = true;

    const store = createUpdate();
    store.hydrate();
    await flush();
    store.apply();

    expectReload(1);
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
    const reg = {
      waiting: null,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      update: vi.fn(),
    };
    registerSwMock.mockResolvedValue(reg);
    setServiceWorker({
      controller: null,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    });

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
