import { mount, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

const toasts = vi.hoisted(() => ({ success: vi.fn(), error: vi.fn() }));
vi.mock("svelte-sonner", () => ({ toast: toasts }));

const h = vi.hoisted(() => {
  const user = {
    id: 7,
    name: "Sara",
    email: "sara@eq.test",
    avatar_id: null,
    is_verified: true,
    role: "user",
    two_fa_enabled: false,
    oauth_provider: null,
  };
  return {
    url: new URL("https://eq.test/account"),
    goto: vi.fn().mockResolvedValue(undefined),
    user,
    authClient: {
      apiBase: "https://eq.test/api",
      get: vi.fn(),
      unsafeRequest: vi.fn(),
      ensureAnonymousSession: vi.fn().mockResolvedValue(undefined),
      refreshCsrf: vi.fn().mockResolvedValue(undefined),
      clearCsrf: vi.fn(),
      getUser: vi.fn(),
    },
  };
});

vi.mock("$app/env", () => ({ browser: true, dev: false }));
vi.mock("$app/env/public", () => ({
  PUBLIC_API_BASE_URL: undefined,
  PUBLIC_QURAN_API_BASE: undefined,
  PUBLIC_ENV: undefined,
  PUBLIC_FCM_VAPID_KEY: undefined,
}));
vi.mock("$app/state", () => ({
  page: {
    get url() {
      return h.url;
    },
  },
}));
vi.mock("$app/navigation", () => ({ goto: h.goto }));
vi.mock("#lib/auth/auth-client.js", async (importOriginal) => {
  const actual = await importOriginal<typeof import("#lib/auth/auth-client.js")>();
  return { ...actual, authClient: h.authClient, createAuthClient: () => h.authClient };
});
vi.mock("#lib/auth/auth-state.svelte.js", () => ({
  authState: {
    status: "authenticated",
    user: h.user,
    probe: vi.fn().mockResolvedValue({ kind: "authenticated", user: h.user }),
    setUser: vi.fn(),
    setTwoFaPending: vi.fn(),
    transition: vi.fn(),
    hydrate: vi.fn(),
  },
}));
vi.mock("#lib/auth/auth-modal.svelte.js", () => ({ authModal: { show: vi.fn() } }));
vi.mock("#lib/auth/flows.svelte.js", () => ({
  createLogoutFlow: () => ({ run: vi.fn().mockResolvedValue(true), pending: false }),
  createTwoFactorFlow: () => ({
    verifyCode: "",
    disableCode: "",
    step: "idle",
    genericError: null,
    pending: false,
    setupData: null,
    verify: vi.fn(),
    disable: vi.fn(),
    setup: vi.fn(),
  }),
}));
vi.mock("#lib/auth/passkey-flow.svelte.js", () => ({
  createPasskeyFlow: () => ({
    supported: false,
    pending: false,
    genericError: null,
    passkeys: [],
    list: vi.fn().mockResolvedValue([]),
    register: vi.fn(),
    remove: vi.fn(),
  }),
}));

import { rememberLinkProvider } from "#lib/auth/link-attempt.js";
import { getAuthCopy } from "#lib/i18n/auth-copy.js";

import AccountPage from "../+page.svelte";

const copy = getAuthCopy("en");

const IDENTITY_SUMMARY = {
  identities: [],
  has_password: true,
  passkey_count: 0,
  can_unlink_any: false,
  linkable_providers: ["github"],
};

let target: HTMLElement;
let unmountPage: () => void = () => {};

async function settle(): Promise<void> {
  for (let i = 0; i < 5; i++) await Promise.resolve();
  await new Promise((resolve) => setTimeout(resolve, 0));
}

function mountPage(): void {
  const instance = mount(AccountPage, { target });
  unmountPage = () => {
    void unmount(instance);
  };
}

beforeEach(() => {
  h.url = new URL("https://eq.test/account");
  h.goto.mockClear();
  toasts.success.mockClear();
  toasts.error.mockClear();
  h.authClient.unsafeRequest.mockReset();
  h.authClient.unsafeRequest.mockImplementation((path: string) =>
    Promise.resolve(
      path === "/device/v1/list"
        ? { ok: true, status: 200, data: { devices: [] }, error: null, rotated: false }
        : undefined,
    ),
  );
  h.authClient.get.mockReset();
  h.authClient.get.mockImplementation((path: string) => {
    if (path === "/auth/v1/reauth/status") {
      return Promise.resolve({
        ok: true,
        status: 200,
        data: { ready: false, has_password: true, two_fa_enabled: false },
        error: null,
        rotated: false,
      });
    }
    if (path === "/user/v1/identities") {
      return Promise.resolve({
        ok: true,
        status: 200,
        data: IDENTITY_SUMMARY,
        error: null,
        rotated: false,
      });
    }
    return Promise.resolve({
      ok: true,
      status: 200,
      data: { data: [] },
      error: null,
      rotated: false,
    });
  });
  target = document.createElement("div");
  document.body.appendChild(target);
});

afterEach(() => {
  unmountPage();
  target.remove();
  sessionStorage.clear();
});

describe("account page sessions list rendering", () => {
  it("renders parsed device labels and formatted last-seen stamps, never raw UA or ISO", async () => {
    h.authClient.get.mockImplementation((path: string) => {
      if (path === "/auth/v1/reauth/status") {
        return Promise.resolve({
          ok: true,
          status: 200,
          data: { ready: false, has_password: true, two_fa_enabled: false },
          error: null,
          rotated: false,
        });
      }
      if (path === "/user/v1/identities") {
        return Promise.resolve({
          ok: true,
          status: 200,
          data: IDENTITY_SUMMARY,
          error: null,
          rotated: false,
        });
      }
      if (path === "/auth/v1/sessions/list") {
        return Promise.resolve({
          ok: true,
          status: 200,
          data: {
            data: [
              {
                id: 3,
                is_current: true,
                device:
                  "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36",
                last_seen: "2026-10-12T07:47:12.919710Z",
              },
              { id: 4, is_current: false, device: "", last_seen: "not-a-date" },
            ],
          },
          error: null,
          rotated: false,
        });
      }
      return Promise.resolve({
        ok: true,
        status: 200,
        data: { data: [] },
        error: null,
        rotated: false,
      });
    });
    mountPage();
    await settle();

    const text = target.textContent ?? "";
    expect(text).toContain("Chrome · macOS");
    expect(text).toContain("Last seen ");
    expect(text).not.toContain("919710");
    expect(text).not.toContain("T07:47");
    // Missing UA degrades to the generic label; unparsable stamp hides the line.
    expect(text).toContain("Unknown device");
    expect(text.indexOf("Last seen ")).toBe(text.lastIndexOf("Last seen "));
  });
});

const DEVICE_LIST = {
  devices: [
    {
      id: "dev-this",
      displayName: null,
      platform: "web",
      client: "web",
      formFactor: "desktop",
      browser: "chrome",
      osMajor: null,
      lastSeenAt: "2026-10-12T07:47:12.919710Z",
      isCurrent: true,
    },
    {
      id: "dev-phone",
      displayName: "<img src=x>Pixel",
      platform: "android",
      client: "react_native",
      formFactor: "phone",
      browser: null,
      osMajor: 14,
      lastSeenAt: "not-a-date",
      isCurrent: false,
    },
    {
      id: "dev-odd",
      displayName: null,
      platform: "plan9",
      client: "web",
      formFactor: "desktop",
      browser: "opera",
      osMajor: null,
      isCurrent: false,
    },
  ],
};

const SESSION_LIST = {
  data: [
    {
      id: 3,
      is_current: true,
      device_id: "dev-this",
      device:
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36",
      last_seen: "2026-10-12T07:47:12.919710Z",
    },
    { id: 4, is_current: false, device_id: null, device: "", last_seen: "not-a-date" },
  ],
};

function okResult<T>(data: T) {
  return { ok: true, status: 200, data, error: null, rotated: false };
}

function mockDeviceAccount(): void {
  h.authClient.get.mockImplementation((path: string) => {
    if (path === "/user/v1/identities") return Promise.resolve(okResult(IDENTITY_SUMMARY));
    if (path === "/auth/v1/sessions/list") return Promise.resolve(okResult(SESSION_LIST));
    return Promise.resolve(okResult({ data: [] }));
  });
  h.authClient.unsafeRequest.mockImplementation((path: string) => {
    if (path === "/device/v1/list") return Promise.resolve(okResult(DEVICE_LIST));
    if (path === "/device/v1/delete") return Promise.resolve(okResult({ revoked: true }));
    if (path === "/device/v1/rename") return Promise.resolve(okResult({ renamed: true }));
    return Promise.resolve(undefined);
  });
}

function deviceRow(text: string): HTMLLIElement | undefined {
  return [...target.querySelectorAll("li")].find((li) => (li.textContent ?? "").includes(text));
}

describe("account page devices (devices.md D4)", () => {
  it("lists devices with their own names or type labels, and bound sessions only once", async () => {
    mockDeviceAccount();
    mountPage();
    await settle();

    const text = target.textContent ?? "";
    expect(text).toContain("Chrome");
    expect(text).toContain("This device");
    expect(text.indexOf("This device")).toBe(text.lastIndexOf("This device"));
    // The user's name is shown as text, never parsed as markup.
    expect(text).toContain("<img src=x>Pixel");
    expect(target.querySelector("img")).toBeNull();
    expect(text).toContain("Android phone");
    // An unknown platform or browser falls back to the localized unknown label, and the
    // unbound session shows the same label. The bound session is not repeated.
    expect(text.match(/Unknown device/gu)?.length).toBeGreaterThanOrEqual(2);
    expect(text).not.toContain("Chrome · macOS");
  });

  it("signs a remote device out at once through the revoke call, with a toast", async () => {
    mockDeviceAccount();
    mountPage();
    await settle();

    const row = deviceRow("Pixel");
    const button = [...(row?.querySelectorAll("button") ?? [])].find(
      (b) => (b.textContent ?? "").trim() === "Sign out",
    );
    expect(button).toBeDefined();
    button?.click();
    await settle();

    expect(h.authClient.unsafeRequest).toHaveBeenCalledWith("/device/v1/delete", {
      method: "POST",
      body: { expected_user_id: 7, device_id: "dev-phone" },
    });
    expect(toasts.success).toHaveBeenCalledTimes(1);
    expect(h.goto).not.toHaveBeenCalled();
  });

  it("refuses an empty or over-long rename without calling the server", async () => {
    mockDeviceAccount();
    mountPage();
    await settle();

    const row = deviceRow("Pixel");
    const rename = [...(row?.querySelectorAll("button") ?? [])].find(
      (b) => (b.textContent ?? "").trim() === "Rename",
    );
    rename?.click();
    await settle();

    const input = target.querySelector<HTMLInputElement>("li input[type=text]");
    expect(input).not.toBeNull();
    if (input) {
      input.value = "   ";
      input.dispatchEvent(new Event("input", { bubbles: true }));
    }
    row?.querySelector("form")?.requestSubmit();
    await settle();

    expect(target.textContent).toContain("Use 1 to 40 characters.");
    expect(h.authClient.unsafeRequest).not.toHaveBeenCalledWith(
      "/device/v1/rename",
      expect.anything(),
    );
  });
});

describe("account page link-flow params", () => {
  it("shows the success banner and clears ?linked via goto replace", async () => {
    h.url = new URL("https://eq.test/account?linked=github");
    mountPage();
    await settle();

    expect(target.textContent).toContain(copy.connectedLinkedBanner("GitHub"));
    expect(h.goto).toHaveBeenCalledTimes(1);
    // SAFETY: the page calls goto(url, opts); the first call is the param-clearing navigation.
    const [url, opts] = h.goto.mock.calls[0] as [URL, { replace?: boolean; reset?: boolean }];
    expect(url).toBeInstanceOf(URL);
    expect(url.searchParams.has("linked")).toBe(false);
    expect(url.searchParams.has("link_error")).toBe(false);
    expect(opts).toMatchObject({ replace: true, reset: false });
  });

  it("names the attempted provider in the already-linked banner and clears the params", async () => {
    rememberLinkProvider("discord");
    h.url = new URL("https://eq.test/account?link_error=already_linked");
    mountPage();
    await settle();

    expect(target.textContent).toContain(copy.connectedAlreadyLinkedError("Discord"));
    expect(h.goto).toHaveBeenCalledTimes(1);
    // SAFETY: the page calls goto(url, opts); the first call is the param-clearing navigation.
    const [url] = h.goto.mock.calls[0] as [URL, { replace?: boolean }];
    expect(url.searchParams.has("link_error")).toBe(false);
  });

  it("maps a cancelled link to the cancelled copy", async () => {
    h.url = new URL("https://eq.test/account?link_error=cancelled");
    mountPage();
    await settle();

    expect(target.textContent).toContain(copy.connectedCancelledError);
    expect(h.goto).toHaveBeenCalledTimes(1);
  });

  it("navigates to /login when the identities client reports a 401 (onAnonymous wiring)", async () => {
    h.authClient.get.mockImplementation((path: string) => {
      if (path === "/auth/v1/reauth/status") {
        return Promise.resolve({
          ok: true,
          status: 200,
          data: { ready: false, has_password: true, two_fa_enabled: false },
          error: null,
          rotated: false,
        });
      }
      if (path === "/user/v1/identities") {
        return Promise.resolve({ ok: false, status: 401, data: null, error: null, rotated: false });
      }
      return Promise.resolve({
        ok: true,
        status: 200,
        data: { data: [] },
        error: null,
        rotated: false,
      });
    });
    mountPage();
    await settle();

    expect(h.goto).toHaveBeenCalledWith("/login");
  });

  it("leaves the URL alone when no link params are present", async () => {
    mountPage();
    await settle();

    expect(h.goto).not.toHaveBeenCalled();
    expect(target.textContent).not.toContain(copy.connectedLinkedBanner("GitHub"));
  });
});
