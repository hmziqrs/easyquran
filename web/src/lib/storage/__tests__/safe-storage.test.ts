import { describe, it, expect, beforeEach, vi } from "vite-plus/test";

const flag = vi.hoisted(() => ({ value: true }));
vi.mock("$app/environment", () => ({
  get browser() {
    return flag.value;
  },
}));

import {
  isFutureSchema,
  onPageHide,
  onStorageKey,
  readJSON,
  readRaw,
  removeJSON,
  writeJSON,
} from "../safe-storage";

describe("safe-storage JSON read/write", () => {
  beforeEach(() => {
    flag.value = true;
    window.localStorage.clear();
  });

  it("round-trips a JSON value", () => {
    writeJSON("k", { a: 1, b: [2, 3] });
    expect(readJSON("k")).toEqual({ a: 1, b: [2, 3] });
  });

  it("returns undefined for a missing key", () => {
    expect(readJSON("missing")).toBeUndefined();
  });

  it("returns undefined for unparseable JSON", () => {
    window.localStorage.setItem("broken", "{not json");
    expect(readJSON("broken")).toBeUndefined();
  });

  it("removeJSON clears a key", () => {
    writeJSON("k", 1);
    removeJSON("k");
    expect(readJSON("k")).toBeUndefined();
  });

  it("is a no-op on the server (browser=false)", () => {
    flag.value = false;
    writeJSON("k", { x: 1 });
    expect(window.localStorage.getItem("k")).toBeNull();
    expect(readJSON("k")).toBeUndefined();
    expect(removeJSON("k")).toBeUndefined();
  });
});

describe("isFutureSchema", () => {
  it("rejects an explicit mismatched version", () => {
    expect(isFutureSchema({ v: 2, current: 1 }, 1)).toBe(true);
    expect(isFutureSchema({ v: "1" }, 1)).toBe(true);
  });
  it("accepts the current version and legacy (versionless) blobs", () => {
    expect(isFutureSchema({ v: 1, current: 1 }, 1)).toBe(false);
    expect(isFutureSchema({ current: 1, mode: "verse" }, 1)).toBe(false);
    expect(isFutureSchema(null, 1)).toBe(false);
    expect(isFutureSchema("nope", 1)).toBe(false);
  });
});

describe("onStorageKey", () => {
  beforeEach(() => {
    flag.value = true;
  });

  it("fires only for the matching key and detaches on teardown", () => {
    const handler = vi.fn();
    const teardown = onStorageKey("easyquran.reader", handler);
    window.dispatchEvent(new StorageEvent("storage", { key: "easyquran.prefs" }));
    expect(handler).not.toHaveBeenCalled();
    window.dispatchEvent(new StorageEvent("storage", { key: "easyquran.reader" }));
    expect(handler).toHaveBeenCalledTimes(1);
    teardown();
    window.dispatchEvent(new StorageEvent("storage", { key: "easyquran.reader" }));
    expect(handler).toHaveBeenCalledTimes(1);
  });

  it("returns a noop teardown on the server", () => {
    flag.value = false;
    const teardown = onStorageKey("x", () => {});
    expect(teardown()).toBeUndefined();
  });
});

describe("onPageHide", () => {
  beforeEach(() => {
    flag.value = true;
  });

  it("fires on pagehide and detaches on teardown", () => {
    const handler = vi.fn();
    const teardown = onPageHide(handler);
    window.dispatchEvent(new PageTransitionEvent("pagehide"));
    expect(handler).toHaveBeenCalledTimes(1);
    teardown();
    window.dispatchEvent(new PageTransitionEvent("pagehide"));
    expect(handler).toHaveBeenCalledTimes(1);
  });
});

describe("readRaw", () => {
  beforeEach(() => {
    flag.value = true;
    window.localStorage.clear();
    window.sessionStorage.clear();
  });

  it("reads a raw value from each area", () => {
    window.localStorage.setItem("k", "1");
    window.sessionStorage.setItem("k", "2");
    expect(readRaw("local", "k")).toBe("1");
    expect(readRaw("session", "k")).toBe("2");
  });

  it("returns null for a missing key", () => {
    expect(readRaw("local", "nope")).toBeNull();
  });

  it("returns null on the server", () => {
    flag.value = false;
    expect(readRaw("local", "k")).toBeNull();
  });

  it("returns null instead of throwing when the browser blocks site data", () => {
    const descriptor = Object.getOwnPropertyDescriptor(Storage.prototype, "getItem");
    Object.defineProperty(Storage.prototype, "getItem", {
      configurable: true,
      value: () => {
        throw new DOMException("The operation is insecure.", "SecurityError");
      },
    });
    try {
      expect(() => readRaw("local", "k")).not.toThrow();
      expect(readRaw("session", "k")).toBeNull();
    } finally {
      if (descriptor) Object.defineProperty(Storage.prototype, "getItem", descriptor);
    }
  });
});
