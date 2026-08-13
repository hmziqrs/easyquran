/* eslint-disable @typescript-eslint/unbound-method */
import { afterEach, describe, expect, it, vi } from "vite-plus/test";
vi.mock("$env/dynamic/public", () => ({ env: { PUBLIC_API_BASE_URL: "https://eq.test/api" } }));

import { createAccountClient, decodeSessionList } from "$lib/auth/account-client";
import type { AuthClient, AuthRequestResult } from "$lib/auth/auth-client";

function mockClient() {
  return {
    apiBase: "https://eq.test/api",
    unsafeRequest: vi.fn(),
    get: vi.fn(),
    ensureAnonymousSession: vi.fn().mockResolvedValue(undefined),
    refreshCsrf: vi.fn().mockResolvedValue(undefined),
    clearCsrf: vi.fn(),
    getUser: vi.fn(),
  } as unknown as AuthClient & {
    unsafeRequest: ReturnType<typeof vi.fn>;
    get: ReturnType<typeof vi.fn>;
  };
}

function ok<T>(data: T, rotated = false): AuthRequestResult<T> {
  return { ok: true, status: 200, data, error: null, rotated };
}
function http<T = never>(status: number, data: T | null = null): AuthRequestResult<T> {
  return { ok: status >= 200 && status < 300, status, data, error: null, rotated: false };
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("decodeSessionList isCurrent decode", () => {
  it("decodes the real {data:[...]} envelope with numeric ids + isCurrent", () => {
    const list = decodeSessionList({
      data: [
        {
          id: 1,
          user_id: 100,
          device: "MacBook/Safari",
          ip_address: null,
          last_seen: "2026-01-01T00:00:00Z",
          revoked_at: null,
          isCurrent: true,
        },
        { id: 2, device: null, last_seen: "2026-02-01T00:00:00Z", isCurrent: false },
      ],
      total: 2,
      page: 1,
    });
    expect(list).toHaveLength(2);
    expect(list[0]!.id).toBe("1");
    expect(list[0]!.isCurrent).toBe(true);
    expect(list[0]!.userAgent).toBe("MacBook/Safari");
    expect(list[0]!.lastSeenAt).toBe("2026-01-01T00:00:00Z");
    expect(list[1]!.id).toBe("2");
    expect(list[1]!.isCurrent).toBe(false);
  });

  it("tolerates snake_case is_current + numeric-string id", () => {
    const list = decodeSessionList({
      data: [{ id: "42", is_current: true, last_seen: "2026-02-01T00:00:00Z" }],
    });
    expect(list[0]!.id).toBe("42");
    expect(list[0]!.isCurrent).toBe(true);
    expect(list[0]!.lastSeenAt).toBe("2026-02-01T00:00:00Z");
  });

  it("drops entries missing any id, keeps numeric + string ids", () => {
    const list = decodeSessionList({
      data: [{ is_current: true }, { id: null }, { id: 7 }, { id: "ok" }],
    });
    expect(list.map((s) => s.id)).toEqual(["7", "ok"]);
  });

  it("accepts a bare array response", () => {
    const list = decodeSessionList([{ id: 5, isCurrent: true }]);
    expect(list.map((s) => s.id)).toEqual(["5"]);
  });

  it("returns empty for malformed payloads", () => {
    expect(decodeSessionList(null)).toEqual([]);
    expect(decodeSessionList({})).toEqual([]);
    expect(decodeSessionList({ data: "nope" })).toEqual([]);
    expect(decodeSessionList({ sessions: [{ id: 1 }] })).toEqual([]);
  });
});

describe("AccountClient.listSessions", () => {
  it("GETs /auth/v1/sessions/list and decodes isCurrent from the {data} envelope", async () => {
    const client = mockClient();
    client.get.mockResolvedValueOnce(
      ok({
        data: [
          { id: 1, isCurrent: true },
          { id: 2, isCurrent: false },
        ],
      }),
    );
    const ac = createAccountClient(client);
    const res = await ac.listSessions();
    expect(res.status).toBe("ok");
    expect(res.data?.map((s) => s.id)).toEqual(["1", "2"]);
    expect(res.data?.map((s) => s.isCurrent)).toEqual([true, false]);
    expect(client.get).toHaveBeenCalledWith("/auth/v1/sessions/list");
  });

  it("401 -> anonymous (not error)", async () => {
    const client = mockClient();
    client.get.mockResolvedValueOnce(http(401));
    const ac = createAccountClient(client);
    const res = await ac.listSessions();
    expect(res.status).toBe("anonymous");
    expect(res.data).toBeNull();
  });

  it("403 -> anonymous", async () => {
    const client = mockClient();
    client.get.mockResolvedValueOnce(http(403));
    const ac = createAccountClient(client);
    const res = await ac.listSessions();
    expect(res.status).toBe("anonymous");
  });
});

describe("AccountClient.terminateSession", () => {
  it("DELETEs /auth/v1/sessions/terminate/{id} and returns ok for the real response shape", async () => {
    const client = mockClient();
    client.unsafeRequest.mockResolvedValueOnce(ok({ message: "Session terminated" }));
    const ac = createAccountClient(client);
    const res = await ac.terminateSession("s-xyz");
    expect(res.status).toBe("ok");
    expect(res.httpStatus).toBe(200);
    expect(client.unsafeRequest).toHaveBeenCalledWith(
      "/auth/v1/sessions/terminate/s-xyz",
      expect.objectContaining({ method: "DELETE" }),
    );
  });

  it("does not derive isCurrent from the terminate response (list is the source of truth)", async () => {
    const client = mockClient();
    client.unsafeRequest.mockResolvedValueOnce(ok({ message: "Session terminated" }));
    const ac = createAccountClient(client);
    const res = await ac.terminateSession("me");
    expect(res.isCurrent).toBe(false);
  });

  it("encodes the id into the path segment safely", async () => {
    const client = mockClient();
    client.unsafeRequest.mockResolvedValueOnce(ok({ message: "Session terminated" }));
    const ac = createAccountClient(client);
    await ac.terminateSession("a b/c");
    const path = (client.unsafeRequest.mock.calls[0] as [string, unknown])[0];
    expect(path).toBe("/auth/v1/sessions/terminate/a%20b%2Fc");
  });

  it("401 on terminate -> anonymous", async () => {
    const client = mockClient();
    client.unsafeRequest.mockResolvedValueOnce(http(401));
    const ac = createAccountClient(client);
    const res = await ac.terminateSession("x");
    expect(res.status).toBe("anonymous");
  });
});

describe("AccountClient.updateProfile", () => {
  it("POSTs /user/v1/update with snake_case body and decodes the profile", async () => {
    const client = mockClient();
    client.unsafeRequest.mockResolvedValueOnce(
      ok({
        id: 3,
        name: "New",
        email: "n@eq.test",
        avatar_id: 4,
        is_verified: true,
        role: "user",
        two_fa_enabled: false,
        oauth_provider: null,
      }),
    );
    const ac = createAccountClient(client);
    const res = await ac.updateProfile({ name: "New", avatarId: 4 });
    expect(res.status).toBe("ok");
    expect(res.data?.name).toBe("New");
    expect(res.data?.avatar_id).toBe(4);
    const [, init] = client.unsafeRequest.mock.calls[0] as [string, { body: unknown }];
    expect(init.body).toEqual({ name: "New", avatar_id: 4 });
  });

  it("omits unset fields from the body", async () => {
    const client = mockClient();
    client.unsafeRequest.mockResolvedValueOnce(ok({ id: 3, email: "n@eq.test" }));
    const ac = createAccountClient(client);
    await ac.updateProfile({ name: "Only" });
    const [, init] = client.unsafeRequest.mock.calls[0] as [string, { body: unknown }];
    expect(init.body).toEqual({ name: "Only" });
  });

  it("401 -> anonymous", async () => {
    const client = mockClient();
    client.unsafeRequest.mockResolvedValueOnce(http(401));
    const ac = createAccountClient(client);
    const res = await ac.updateProfile({ name: "X" });
    expect(res.status).toBe("anonymous");
    expect(res.data).toBeNull();
  });
});
