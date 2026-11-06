import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";

import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

vi.mock("$app/env", () => ({ browser: true }));
vi.mock("$app/env/public", () => ({ PUBLIC_API_BASE_URL: "/api", PUBLIC_FCM_VAPID_KEY: "" }));

import { authClient } from "#lib/auth/auth-client.js";

import { deviceIdFor } from "../identity";
import {
  browserFamily,
  ensureDeviceRegistered,
  forgetRegisteredToken,
  REGISTER_INTERVAL_MS,
  registrationDue,
  registrationFor,
} from "../register";

const requests: { url: string; init: RequestInit }[] = [];

function registerCalls(): { url: string; init: RequestInit }[] {
  return requests.filter((request) => request.url.endsWith("/device/v1/register"));
}

beforeEach(() => {
  window.localStorage.clear();
  authClient.clearCsrf();
  requests.length = 0;
  vi.stubGlobal(
    "fetch",
    vi.fn((url: string, init: RequestInit = {}) => {
      requests.push({ url, init });
      if (url.endsWith("/csrf/v1/generate"))
        return Promise.resolve(Response.json({ token: "csrf-proof" }));
      return Promise.resolve(Response.json({ registered: true }));
    }),
  );
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("browser family", () => {
  it("maps user agents onto the closed set", () => {
    expect(browserFamily("Mozilla/5.0 Chrome/130.0 Safari/537.36 Edg/130.0")).toBe("edge");
    expect(browserFamily("Mozilla/5.0 Firefox/131.0")).toBe("firefox");
    expect(browserFamily("Mozilla/5.0 Chrome/130.0 Safari/537.36")).toBe("chrome");
    expect(browserFamily("Mozilla/5.0 CriOS/130.0 Mobile/15E148 Safari/604.1")).toBe("chrome");
    expect(browserFamily("Mozilla/5.0 Version/17.0 Safari/605.1.15")).toBe("safari");
    expect(browserFamily("curl/8.0")).toBe("other");
  });
});

describe("registration payload", () => {
  it("carries only the closed fields and the per-account device id", () => {
    const body = registrationFor(7);
    expect(Object.keys(body).sort()).toEqual(
      ["browser", "client", "device_id", "expected_user_id", "form_factor", "platform"].sort(),
    );
    expect(body.device_id).toBe(deviceIdFor("7"));
    expect(body.platform).toBe("web");
    expect(body.client).toBe("web");
    expect(["phone", "tablet", "desktop"]).toContain(body.form_factor);
  });

  it("adds the push token only when one is given", () => {
    expect(registrationFor(7, "tok")).toMatchObject({ token: "tok" });
    expect(registrationFor(7)).not.toHaveProperty("token");
  });
});

describe("once per 24 hours", () => {
  it("is due when never registered", () => {
    expect(registrationDue(7, null)).toBe(true);
  });

  it("posts once, then stays quiet for a day", async () => {
    expect(await ensureDeviceRegistered(7)).toBe(true);
    expect(await ensureDeviceRegistered(7)).toBe(true);
    expect(registerCalls()).toHaveLength(1);
    expect(registrationDue(7, null)).toBe(false);
    expect(registrationDue(7, null, Date.now() + REGISTER_INTERVAL_MS + 60_000)).toBe(true);
  });

  it("re-posts as soon as the push token changes", async () => {
    await ensureDeviceRegistered(7, "tok-1");
    expect(await ensureDeviceRegistered(7, "tok-1")).toBe(true);
    expect(registerCalls()).toHaveLength(1);
    expect(await ensureDeviceRegistered(7, "tok-2")).toBe(true);
    expect(registerCalls()).toHaveLength(2);
  });

  it("posts again after a push token is removed on sign-out", async () => {
    await ensureDeviceRegistered(7, "tok-1");
    forgetRegisteredToken(7);
    await ensureDeviceRegistered(7, "tok-1");
    expect(registerCalls()).toHaveLength(2);
  });

  it("reports a failed register as false and records nothing", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn((url: string) =>
        url.endsWith("/csrf/v1/generate")
          ? Promise.resolve(Response.json({ token: "csrf-proof" }))
          : Promise.resolve(Response.json({ message: "limited" }, { status: 429 })),
      ),
    );
    expect(await ensureDeviceRegistered(7)).toBe(false);
    expect(registrationDue(7, null)).toBe(true);
  });

  it("another tab holding the lock means it is registering now: no second post", async () => {
    const request = vi.fn(
      (_name: string, _options: LockOptions, callback: LockGrantedCallback<boolean>) =>
        Promise.resolve(callback(null)),
    );
    vi.stubGlobal("navigator", { locks: { request } });
    expect(await ensureDeviceRegistered(7)).toBe(true);
    expect(request).toHaveBeenCalledWith(
      "easyquran.device.register",
      { ifAvailable: true },
      expect.any(Function),
    );
    expect(registerCalls()).toHaveLength(0);
  });
});

describe("analytics and rate limits never see device identity", () => {
  const root = join(process.cwd(), "src");
  const guarded = (rel: string): boolean =>
    /analytic|performance|rate-?limit|telemetry|firebase\/analytics/u.test(rel);

  function walk(dir: string, out: string[]): void {
    for (const name of readdirSync(dir)) {
      const path = join(dir, name);
      if (statSync(path).isDirectory()) walk(path, out);
      else if (/\.(ts|svelte)$/u.test(name)) out.push(path);
    }
  }

  it("no guarded module imports the device id or the device registry", () => {
    const files: string[] = [];
    walk(root, files);
    const offenders = files
      .map((file) => ({ rel: relative(root, file), text: readFileSync(file, "utf8") }))
      .filter(({ rel }) => guarded(rel))
      .filter(({ text }) =>
        /device\/(identity|register)|deviceIdFor|ensureDeviceRegistered/u.test(text),
      )
      .map(({ rel }) => rel);
    expect(offenders).toEqual([]);
  });
});
