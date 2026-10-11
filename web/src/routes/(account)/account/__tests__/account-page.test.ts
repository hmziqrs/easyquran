import { mount, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

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

vi.mock("$app/env", () => ({ browser: true }));
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
