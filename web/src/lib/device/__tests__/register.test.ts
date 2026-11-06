import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";

import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

vi.mock("$app/env", () => ({ browser: true }));
vi.mock("$app/env/public", () => ({ PUBLIC_API_BASE_URL: "/api", PUBLIC_FCM_VAPID_KEY: "" }));

import { authClient } from "#lib/auth/auth-client.js";

import { deviceIdFor } from "../identity";
import { memoryKeyStore } from "../key-store";
import { base64UrlDecode, proofMessage, verifyProof } from "../keys";
import {
  type DeviceRegistration,
  browserFamily,
  ensureDeviceRegistered,
  forgetRegisteredToken,
  REGISTER_INTERVAL_MS,
  registrationDue,
  registrationFor,
  setDeviceKeyStore,
} from "../register";

interface Recorded {
  url: string;
  init: RequestInit;
}

/** A 32-byte value, base64url. Every nonce and salt in these tests. */
const NONCE = "A".repeat(43);

const requests: Recorded[] = [];
/** The id the server reports back. Defaults to echoing the sent id. */
let serverId: ((sent: string) => string) | null = null;
/** Status for the next register. Defaults to 200. */
let registerStatus = 200;
let challengeOk = true;

/** The JSON body of a recorded request. */
function bodyOf(init: RequestInit): Partial<DeviceRegistration> {
  // SAFETY: the auth client sends JSON text, so every recorded register body is a string.
  const text = init.body as string;
  // SAFETY: this client serialises the body itself from `DeviceRegistration`, so the shape holds.
  return JSON.parse(text) as Partial<DeviceRegistration>;
}

function registerCalls(): Recorded[] {
  return requests.filter((request) => request.url.endsWith("/device/v1/register"));
}

function challengeCalls(): Recorded[] {
  return requests.filter((request) => request.url.endsWith("/device/v1/challenge"));
}

function stubFetch(): void {
  vi.stubGlobal(
    "fetch",
    vi.fn((url: string, init: RequestInit = {}) => {
      requests.push({ url, init });
      if (url.endsWith("/csrf/v1/generate"))
        return Promise.resolve(Response.json({ token: "csrf-proof" }));
      if (url.endsWith("/device/v1/challenge")) {
        if (!challengeOk) return Promise.resolve(Response.json({}, { status: 500 }));
        return Promise.resolve(Response.json({ nonce: NONCE, salt: NONCE }));
      }
      if (url.endsWith("/device/v1/register")) {
        if (registerStatus !== 200) {
          return Promise.resolve(Response.json({ message: "refused" }, { status: registerStatus }));
        }
        const sent = String(bodyOf(init).device_id);
        const id = serverId === null ? sent : serverId(sent);
        return Promise.resolve(Response.json({ registered: true, device_id: id }));
      }
      return Promise.resolve(Response.json({ registered: true }));
    }),
  );
}

beforeEach(() => {
  window.localStorage.clear();
  authClient.clearCsrf();
  requests.length = 0;
  serverId = null;
  registerStatus = 200;
  challengeOk = true;
  setDeviceKeyStore(memoryKeyStore());
  stubFetch();
});

afterEach(() => {
  setDeviceKeyStore(null);
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
      [
        "browser",
        "client",
        "device_id",
        "expected_user_id",
        "form_factor",
        "key_store",
        "platform",
      ].sort(),
    );
    expect(body.device_id).toBe(deviceIdFor("7"));
    expect(body.platform).toBe("web");
    expect(body.client).toBe("web");
    expect(body.key_store).toBe("none");
    expect(["phone", "tablet", "desktop"]).toContain(body.form_factor);
  });

  it("adds the push token only when one is given", () => {
    expect(registrationFor(7, deviceIdFor("7"), "tok")).toMatchObject({ token: "tok" });
    expect(registrationFor(7)).not.toHaveProperty("token");
  });
});

describe("device key proof", () => {
  it("registers with the browser key, a fresh nonce and a signature the server can check", async () => {
    expect(await ensureDeviceRegistered(7)).toBe(true);
    const [call] = registerCalls();
    expect(call).toBeDefined();
    const body = bodyOf(call!.init);
    expect(body.key_store).toBe("browser");
    expect(body).not.toHaveProperty("continuity_hint");
    const publicJwk = body.public_key;
    if (publicJwk === undefined) throw new Error("keyed register sent no public_key");
    expect(Object.keys(publicJwk).sort()).toEqual(["crv", "kty", "x", "y"]);

    const signature = base64UrlDecode(String(body.signature));
    expect(signature?.length).toBe(64);
    const message = proofMessage(String(body.nonce), deviceIdFor("7"), 7);
    expect(await verifyProof(publicJwk, message, signature ?? new Uint8Array())).toBe(true);
  });

  it("keeps the same key across registers and mints a new one on a 409", async () => {
    await ensureDeviceRegistered(7, "tok-1");
    await ensureDeviceRegistered(7, "tok-2");
    const [first, second] = registerCalls().map((call) => bodyOf(call.init));
    expect(second?.public_key).toEqual(first?.public_key);

    registerStatus = 409;
    const before = deviceIdFor("7");
    const ok = await ensureDeviceRegistered(7, "tok-3");
    expect(ok).toBe(false);
    const attempts = registerCalls()
      .slice(2)
      .map((call) => bodyOf(call.init));
    expect(attempts).toHaveLength(2);
    expect(attempts[0]?.device_id).toBe(before);
    expect(attempts[1]?.device_id).not.toBe(before);
    expect(attempts[1]?.public_key).not.toEqual(first?.public_key);
    expect(deviceIdFor("7")).toBe(attempts[1]?.device_id);
  });

  it("adopts the id the server kept, not the one it sent", async () => {
    serverId = () => "0190f2c4-7b1a-7c3d-9e4f-0123456789ab";
    expect(await ensureDeviceRegistered(7)).toBe(true);
    expect(deviceIdFor("7")).toBe("0190f2c4-7b1a-7c3d-9e4f-0123456789ab");
  });

  it("registers without a key when the key store is unusable, and still registers", async () => {
    setDeviceKeyStore({
      get: () => Promise.reject(new Error("idb blocked")),
      putIfAbsent: () => Promise.reject(new Error("idb blocked")),
      put: () => Promise.reject(new Error("idb blocked")),
    });
    expect(await ensureDeviceRegistered(7)).toBe(true);
    const body = bodyOf(registerCalls()[0]!.init);
    expect(body.key_store).toBe("none");
    expect(body).not.toHaveProperty("signature");
    expect(challengeCalls()).toHaveLength(0);
  });

  it("does not register when the challenge fails, and records nothing", async () => {
    challengeOk = false;
    expect(await ensureDeviceRegistered(7)).toBe(false);
    expect(registerCalls()).toHaveLength(0);
    expect(registrationDue(7, null)).toBe(true);
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
    registerStatus = 429;
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
        /device\/(identity|register|keys|key-store)|deviceIdFor|ensureDeviceRegistered/u.test(text),
      )
      .map(({ rel }) => rel);
    expect(offenders).toEqual([]);
  });
});
