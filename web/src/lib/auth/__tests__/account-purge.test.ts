import { beforeEach, describe, expect, it, vi } from "vite-plus/test";

vi.mock("#lib/sync/outbox.js", () => ({
  destroyAllSyncRecords: vi.fn().mockResolvedValue(undefined),
}));
vi.mock("#lib/device/key-store.js", () => ({
  deleteDeviceKey: vi.fn().mockResolvedValue(undefined),
}));

import { ACCOUNT_KEY_PREFIXES, purgeAccountLocalData } from "#lib/auth/account-purge.js";
import { deleteDeviceKey } from "#lib/device/key-store.js";
import { destroyAllSyncRecords } from "#lib/sync/outbox.js";

describe("purgeAccountLocalData", () => {
  beforeEach(() => {
    localStorage.clear();
    vi.clearAllMocks();
  });

  it("removes the deleted account's keys and the outbox and device key", async () => {
    localStorage.setItem("easyquran.preferences.42", "{}");
    localStorage.setItem("easyquran.sessions.42", "{}");
    localStorage.setItem("easyquran.bookmarks.user.42", "[]");
    localStorage.setItem("easyquran.sync.clock.42", "{}");

    await purgeAccountLocalData(42);

    expect(localStorage.getItem("easyquran.preferences.42")).toBeNull();
    expect(localStorage.getItem("easyquran.sessions.42")).toBeNull();
    expect(localStorage.getItem("easyquran.bookmarks.user.42")).toBeNull();
    expect(localStorage.getItem("easyquran.sync.clock.42")).toBeNull();
    expect(destroyAllSyncRecords).toHaveBeenCalledTimes(1);
    expect(deleteDeviceKey).toHaveBeenCalledWith("42");
  });

  it("keeps device-only reading data and another account's keys", async () => {
    localStorage.setItem("easyquran.prefs", "{}");
    localStorage.setItem("easyquran.reader", "{}");
    localStorage.setItem("easyquran.bookmarks.local", "[]");
    localStorage.setItem("easyquran.reading.bookmark", "{}");
    localStorage.setItem("easyquran.preferences.local", "{}");
    localStorage.setItem("easyquran.sessions.7", "{}");

    await purgeAccountLocalData(42);

    expect(localStorage.getItem("easyquran.prefs")).toBe("{}");
    expect(localStorage.getItem("easyquran.reader")).toBe("{}");
    expect(localStorage.getItem("easyquran.bookmarks.local")).toBe("[]");
    expect(localStorage.getItem("easyquran.preferences.local")).toBe("{}");
    expect(localStorage.getItem("easyquran.sessions.7")).toBe("{}");
  });

  it("keeps going when one store fails", async () => {
    vi.mocked(destroyAllSyncRecords).mockRejectedValueOnce(new Error("blocked"));
    localStorage.setItem("easyquran.goals.7", "{}");

    await expect(purgeAccountLocalData(7)).resolves.toBeUndefined();

    expect(deleteDeviceKey).toHaveBeenCalledWith("7");
    expect(localStorage.getItem("easyquran.goals.7")).toBeNull();
  });

  it("lists only account-keyed prefixes", () => {
    expect(ACCOUNT_KEY_PREFIXES.every((prefix) => prefix.endsWith("."))).toBe(true);
    expect(ACCOUNT_KEY_PREFIXES).not.toContain("easyquran.prefs");
    expect(ACCOUNT_KEY_PREFIXES).not.toContain("easyquran.reader");
  });
});
