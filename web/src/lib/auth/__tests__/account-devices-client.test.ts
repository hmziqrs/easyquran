/* eslint-disable @typescript-eslint/unbound-method */
import { describe, expect, it, vi } from "vite-plus/test";
vi.mock("$app/env/public", () => ({
  PUBLIC_API_BASE_URL: "https://eq.test/api",
  PUBLIC_QURAN_API_BASE: undefined,
  PUBLIC_ENV: undefined,
  PUBLIC_FCM_VAPID_KEY: undefined,
}));

import {
  createAccountClient,
  decodeDeviceList,
  decodeSessionList,
} from "#lib/auth/account-client.js";
import type { AuthClient, AuthRequestResult } from "#lib/auth/auth-client.js";

function mockClient(): AuthClient & {
  unsafeRequest: ReturnType<typeof vi.fn>;
  get: ReturnType<typeof vi.fn>;
} {
  // SAFETY: test double; AccountClient only calls unsafeRequest and get here.
  return {
    apiBase: "https://eq.test/api",
    unsafeRequest: vi.fn(),
    get: vi.fn(),
    ensureAnonymousSession: vi.fn().mockResolvedValue(undefined),
    refreshCsrf: vi.fn().mockResolvedValue(undefined),
    clearCsrf: vi.fn(),
    getUser: vi.fn(),
  } as never;
}

function ok<T>(data: T): AuthRequestResult<T> {
  return { ok: true, status: 200, data, error: null, rotated: false };
}

function http<T = never>(status: number): AuthRequestResult<T> {
  return { ok: false, status, data: null, error: null, rotated: false };
}

const ROW = {
  id: "dev-1",
  displayName: "Pixel",
  platform: "android",
  client: "react_native",
  formFactor: "phone",
  browser: null,
  osMajor: 14,
  lastSeenAt: "2026-10-12T07:47:12.919710Z",
  isCurrent: true,
};

describe("decodeDeviceList", () => {
  it("decodes the {devices} envelope in camelCase", () => {
    const list = decodeDeviceList({ devices: [ROW] });
    expect(list).toEqual([
      {
        id: "dev-1",
        displayName: "Pixel",
        platform: "android",
        client: "react_native",
        formFactor: "phone",
        browser: null,
        osMajor: 14,
        lastSeenAt: "2026-10-12T07:47:12.919710Z",
        isCurrent: true,
      },
    ]);
  });

  it("drops rows without an id or a closed-enum field, and blanks empty names", () => {
    const list = decodeDeviceList({
      devices: [
        { ...ROW, id: "" },
        { ...ROW, id: 5 },
        { ...ROW, platform: undefined },
        { ...ROW, id: "dev-2", displayName: "" },
      ],
    });
    expect(list.map((d) => d.id)).toEqual(["dev-2"]);
    expect(list[0]?.displayName).toBeNull();
  });

  it("returns empty for malformed payloads", () => {
    expect(decodeDeviceList(null)).toEqual([]);
    expect(decodeDeviceList({ devices: "nope" })).toEqual([]);
  });
});

describe("AccountClient device calls", () => {
  it("POSTs /device/v1/list and decodes the rows", async () => {
    const client = mockClient();
    client.unsafeRequest.mockResolvedValueOnce(ok({ devices: [ROW] }));
    const res = await createAccountClient(client).listDevices();
    expect(res.status).toBe("ok");
    expect(res.data?.map((d) => d.id)).toEqual(["dev-1"]);
    expect(client.unsafeRequest).toHaveBeenCalledWith("/device/v1/list", { method: "POST" });
  });

  it("POSTs the rename with the owner, id, name and stamp, and reads renamed", async () => {
    const client = mockClient();
    client.unsafeRequest.mockResolvedValueOnce(ok({ renamed: false }));
    const res = await createAccountClient(client).renameDevice(
      7,
      "dev-1",
      "Pixel 8",
      "1767225600000-0000-dev-web",
    );
    expect(res.data).toBe(false);
    expect(client.unsafeRequest).toHaveBeenCalledWith("/device/v1/rename", {
      method: "POST",
      body: {
        expected_user_id: 7,
        device_id: "dev-1",
        name: "Pixel 8",
        name_stamp: "1767225600000-0000-dev-web",
      },
    });
  });

  it("POSTs the remote sign-out and maps 401 to anonymous", async () => {
    const client = mockClient();
    client.unsafeRequest.mockResolvedValueOnce(ok({ revoked: true }));
    const res = await createAccountClient(client).revokeDevice(7, "dev-1");
    expect(res.status).toBe("ok");
    expect(client.unsafeRequest).toHaveBeenCalledWith("/device/v1/delete", {
      method: "POST",
      body: { expected_user_id: 7, device_id: "dev-1" },
    });

    client.unsafeRequest.mockResolvedValueOnce(http(401));
    const anon = await createAccountClient(client).revokeDevice(7, "dev-1");
    expect(anon.status).toBe("anonymous");
  });
});

describe("session rows carry their bound device", () => {
  it("decodes device_id and leaves it absent for an unbound session", () => {
    const list = decodeSessionList({
      data: [
        { id: 1, device_id: "dev-1", isCurrent: true },
        { id: 2, device_id: null, isCurrent: false },
      ],
    });
    expect(list[0]?.deviceId).toBe("dev-1");
    expect(list[1]?.deviceId).toBeUndefined();
  });
});
