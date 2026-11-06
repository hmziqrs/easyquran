import { describe, expect, it } from "vite-plus/test";

import { memoryKeyStore, ensureDeviceKey, repairDeviceKey } from "../key-store";

describe("device key store", () => {
  it("two tabs racing a first sign-in end with one key", async () => {
    const store = memoryKeyStore();
    const [a, b] = await Promise.all([
      ensureDeviceKey(store, "7", "dev-race"),
      ensureDeviceKey(store, "7", "dev-race"),
    ]);
    expect(a.privateKey).toBe(b.privateKey);
    expect(a.publicJwk).toEqual(b.publicJwk);
    expect(a.deviceId).toBe("dev-race");
  });

  it("reuses the stored key for the same id", async () => {
    const store = memoryKeyStore();
    const first = await ensureDeviceKey(store, "7", "dev-same");
    const again = await ensureDeviceKey(store, "7", "dev-same");
    expect(again.privateKey).toBe(first.privateKey);
  });

  it("treats a record for another id as lost and replaces it", async () => {
    const store = memoryKeyStore();
    const first = await ensureDeviceKey(store, "7", "dev-one");
    const second = await ensureDeviceKey(store, "7", "dev-two");
    expect(second.deviceId).toBe("dev-two");
    expect(second.publicJwk).not.toEqual(first.publicJwk);
    expect((await store.get("7"))?.deviceId).toBe("dev-two");
  });

  it("accounts never share a key", async () => {
    const store = memoryKeyStore();
    const a = await ensureDeviceKey(store, "7", "dev-shared");
    const b = await ensureDeviceKey(store, "9", "dev-shared");
    expect(b.publicJwk).not.toEqual(a.publicJwk);
  });

  it("re-pairs a stored key with the id the server kept", async () => {
    const store = memoryKeyStore();
    const key = await ensureDeviceKey(store, "7", "dev-old");
    const repaired = await repairDeviceKey(store, "7", key, "dev-kept");
    expect(repaired.deviceId).toBe("dev-kept");
    expect(repaired.privateKey).toBe(key.privateKey);
    expect((await store.get("7"))?.deviceId).toBe("dev-kept");
  });
});
