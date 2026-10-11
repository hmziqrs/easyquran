import { mount, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";
vi.mock("$app/env", () => ({ browser: true }));
vi.mock("$app/env/public", () => ({
  PUBLIC_API_BASE_URL: "https://eq.test/api",
  PUBLIC_QURAN_API_BASE: undefined,
  PUBLIC_ENV: undefined,
  PUBLIC_FCM_VAPID_KEY: undefined,
}));

import type {
  AccountClient,
  AccountResult,
  IdentityProvider,
  IdentitySummary,
  LinkedIdentity,
} from "#lib/auth/account-client.js";
import { GENERIC_TRY_AGAIN } from "#lib/auth/auth-copy.js";
import { getAuthCopy } from "#lib/i18n/auth-copy.js";

import ConnectedAccounts from "../ConnectedAccounts.svelte";
import RefreshHarness from "./connected-accounts-refresh-harness.svelte";

const copy = getAuthCopy("en");
const API_BASE = "https://eq.test/api";

function identity(provider: IdentityProvider): LinkedIdentity {
  return { provider, providerUserId: `${provider}-1`, createdAt: "2026-01-01T00:00:00Z" };
}

function summary(over: Partial<IdentitySummary>): IdentitySummary {
  return {
    identities: [],
    hasPassword: false,
    passkeyCount: 0,
    canUnlinkAny: true,
    linkableProviders: [],
    ...over,
  };
}

function ok<T>(data: T): AccountResult<T> {
  return { status: "ok", httpStatus: 200, data, error: null };
}

function anonymous<T>(): AccountResult<T> {
  return { status: "anonymous", httpStatus: 401, data: null, error: null };
}

function conflict(message: string): AccountResult<never> {
  return { status: "error", httpStatus: 409, data: null, error: { message } };
}

function mockClient(
  getIdentities: ReturnType<typeof vi.fn>,
  unlinkIdentity: ReturnType<typeof vi.fn>,
): AccountClient {
  // SAFETY: test double — ConnectedAccounts only calls getIdentities/unlinkIdentity and reads client.auth.apiBase.
  return {
    auth: { apiBase: API_BASE },
    getIdentities,
    unlinkIdentity,
  } as never;
}

interface MountProps {
  navigate?: (url: string) => void;
  onAnonymous?: () => void | Promise<void>;
  refreshKey?: string;
}

interface Deferred<T> {
  promise: Promise<T>;
  resolve: (value: T) => void;
}

function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((res) => {
    resolve = res;
  });
  return { promise, resolve };
}

let target: HTMLElement;
let unmountComponent: () => void = () => {};

async function settle(): Promise<void> {
  for (let i = 0; i < 5; i++) await Promise.resolve();
  await new Promise((resolve) => setTimeout(resolve, 0));
}

function buttons(): HTMLButtonElement[] {
  return [...target.querySelectorAll("button")];
}

function button(text: string): HTMLButtonElement {
  const el = buttons().find((b) => b.textContent?.trim() === text);
  if (!el) throw new Error(`missing button "${text}"`);
  return el;
}

function errorTexts(): Array<string> {
  return [...target.querySelectorAll("[role='alert']")].map((el) => el.textContent?.trim() ?? "");
}

function focusedUnlinkProvider(): string | null {
  return document.activeElement?.getAttribute("data-unlink-provider") ?? null;
}

async function mountWith(
  getIdentities: ReturnType<typeof vi.fn>,
  unlinkIdentity: ReturnType<typeof vi.fn>,
  props: MountProps = {},
) {
  const client = mockClient(getIdentities, unlinkIdentity);
  const instance = mount(ConnectedAccounts, { target, props: { client, ...props } });
  unmountComponent = () => {
    void unmount(instance);
  };
  await settle();
  return { getIdentities, unlinkIdentity };
}

async function mountComponent(
  data: IdentitySummary,
  navigate = vi.fn(),
  unlinkResponse?: AccountResult<IdentitySummary>,
) {
  const getIdentities = vi.fn().mockResolvedValue(ok(data));
  const unlinkIdentity = vi.fn();
  if (unlinkResponse !== undefined) unlinkIdentity.mockResolvedValue(unlinkResponse);
  const mounted = await mountWith(getIdentities, unlinkIdentity, { navigate });
  return { ...mounted, navigate };
}

beforeEach(() => {
  target = document.createElement("div");
  document.body.appendChild(target);
});

afterEach(() => {
  unmountComponent();
  target.remove();
  sessionStorage.clear();
});

describe("ConnectedAccounts list + last-method protection", () => {
  it("lists linked identities with provider label and linked date", async () => {
    await mountComponent(summary({ identities: [identity("github")], passkeyCount: 1 }));
    expect(target.textContent).toContain("GitHub");
    expect(target.textContent).toContain("Linked");
  });

  it("disables unlink on the only sign-in method and shows the hint", async () => {
    await mountComponent(summary({ identities: [identity("github")] }));
    expect(button("Unlink").disabled).toBe(true);
    expect(target.textContent).toContain("before you can unlink");
  });

  it("enables unlink once a password backs the account", async () => {
    await mountComponent(summary({ identities: [identity("github")], hasPassword: true }));
    expect(button("Unlink").disabled).toBe(false);
    expect(target.textContent).not.toContain("before you can unlink");
  });

  it("enables unlink once a passkey backs the account", async () => {
    await mountComponent(summary({ identities: [identity("github")], passkeyCount: 1 }));
    expect(button("Unlink").disabled).toBe(false);
  });

  it("names the provider in the row action aria-labels", async () => {
    await mountComponent(summary({ identities: [identity("github")], hasPassword: true }));
    expect(button("Unlink").getAttribute("aria-label")).toBe("Unlink GitHub");
    button("Unlink").click();
    await settle();
    expect(button("Confirm unlink").getAttribute("aria-label")).toBe("Confirm unlink GitHub");
    expect(button("Cancel").getAttribute("aria-label")).toBe("Cancel GitHub");
  });

  it("reloads the summary when refreshKey changes (passkey add/remove elsewhere)", async () => {
    const getIdentities = vi
      .fn()
      .mockResolvedValue(ok(summary({ identities: [identity("github")] })));
    const unlinkIdentity = vi.fn();
    const client = mockClient(getIdentities, unlinkIdentity);
    const instance = mount(RefreshHarness, { target, props: { client } });
    unmountComponent = () => {
      void unmount(instance);
    };
    await settle();
    expect(getIdentities).toHaveBeenCalledTimes(1);

    button("bump").click();
    await settle();
    expect(getIdentities).toHaveBeenCalledTimes(2);
  });

  it("drops a stale load response that resolves after a newer one", async () => {
    const older = deferred<AccountResult<IdentitySummary>>();
    const newer = deferred<AccountResult<IdentitySummary>>();
    const getIdentities = vi
      .fn()
      .mockReturnValueOnce(older.promise)
      .mockReturnValueOnce(newer.promise);
    const client = mockClient(getIdentities, vi.fn());
    const instance = mount(RefreshHarness, { target, props: { client } });
    unmountComponent = () => {
      void unmount(instance);
    };
    await settle();
    expect(getIdentities).toHaveBeenCalledTimes(1);

    button("bump").click();
    await settle();
    expect(getIdentities).toHaveBeenCalledTimes(2);

    newer.resolve(ok(summary({ identities: [identity("github"), identity("google")] })));
    await settle();
    expect(button("Unlink").disabled).toBe(false);

    // The older response lands last and must not clobber the newer summary
    // (which would wrongly re-disable Unlink and re-show the hint).
    older.resolve(ok(summary({ identities: [identity("github")] })));
    await settle();
    expect(button("Unlink").disabled).toBe(false);
    expect(target.textContent).toContain("Google");
    expect(target.textContent).not.toContain("before you can unlink");
    expect(errorTexts()).toEqual([]);
  });
});

describe("ConnectedAccounts unlink flow", () => {
  it("requires a confirm step before calling the API, then refreshes from the response", async () => {
    const next = summary({ identities: [identity("google")], hasPassword: true });
    const { unlinkIdentity } = await mountComponent(
      summary({ identities: [identity("github"), identity("google")] }),
      vi.fn(),
      ok(next),
    );

    button("Unlink").click();
    await settle();
    expect(unlinkIdentity).not.toHaveBeenCalled();
    expect(buttons().some((b) => b.textContent?.trim() === "Confirm unlink")).toBe(true);

    button("Confirm unlink").click();
    await settle();
    expect(unlinkIdentity).toHaveBeenCalledWith("github");
    expect(target.textContent).not.toContain("GitHub");
    expect(target.textContent).toContain("Google");
  });

  it("keeps the post-unlink summary when an older load resolves last", async () => {
    const stale = deferred<AccountResult<IdentitySummary>>();
    const preUnlink = summary({ identities: [identity("github"), identity("google")] });
    const postUnlink = summary({ identities: [identity("google")] });
    const getIdentities = vi
      .fn()
      .mockResolvedValueOnce(ok(preUnlink))
      .mockReturnValueOnce(stale.promise);
    const unlinkIdentity = vi.fn().mockResolvedValue(ok(postUnlink));
    const client = mockClient(getIdentities, unlinkIdentity);
    const instance = mount(RefreshHarness, { target, props: { client } });
    unmountComponent = () => {
      void unmount(instance);
    };
    await settle();
    expect(getIdentities).toHaveBeenCalledTimes(1);

    // Refresh starts while the unlink is about to land; its response is stale.
    button("bump").click();
    await settle();
    expect(getIdentities).toHaveBeenCalledTimes(2);

    button("Unlink").click();
    await settle();
    button("Confirm unlink").click();
    await settle();
    expect(unlinkIdentity).toHaveBeenCalledWith("github");
    expect(target.textContent).not.toContain("GitHub");

    stale.resolve(ok(preUnlink));
    await settle();
    expect(target.textContent).not.toContain("GitHub");
    expect(target.textContent).toContain("Google");
  });

  it("clears a refreshKey load error when a successful unlink response lands", async () => {
    const failedLoad = deferred<AccountResult<IdentitySummary>>();
    const unlinkResult = deferred<AccountResult<IdentitySummary>>();
    const preUnlink = summary({ identities: [identity("github"), identity("google")] });
    const postUnlink = summary({ identities: [identity("google")] });
    const getIdentities = vi
      .fn()
      .mockResolvedValueOnce(ok(preUnlink))
      .mockReturnValueOnce(failedLoad.promise);
    const unlinkIdentity = vi.fn().mockReturnValue(unlinkResult.promise);
    const client = mockClient(getIdentities, unlinkIdentity);
    const instance = mount(RefreshHarness, { target, props: { client } });
    unmountComponent = () => {
      void unmount(instance);
    };
    await settle();
    expect(getIdentities).toHaveBeenCalledTimes(1);

    button("Unlink").click();
    await settle();
    button("bump").click();
    await settle();
    expect(getIdentities).toHaveBeenCalledTimes(2);

    button("Confirm unlink").click();
    await settle();
    expect(unlinkIdentity).toHaveBeenCalledWith("github");

    // The refresh fails while the unlink is in flight; the error affordance must
    // not outlive the newer unlink snapshot.
    failedLoad.resolve({ status: "error", httpStatus: 500, data: null, error: null });
    await settle();
    expect(errorTexts()).toContain(GENERIC_TRY_AGAIN);

    unlinkResult.resolve(ok(postUnlink));
    await settle();
    expect(errorTexts()).toEqual([]);
    expect(buttons().some((b) => b.textContent?.trim() === "Retry")).toBe(false);
    expect(target.textContent).toContain("Google");
    expect(target.textContent).not.toContain("GitHub");
  });

  it("moves focus to the confirm button on open and back to the row button on cancel", async () => {
    await mountComponent(
      summary({ identities: [identity("github"), identity("google")] }),
      vi.fn(),
      ok(summary({ identities: [identity("google")], hasPassword: true })),
    );

    button("Unlink").click();
    await settle();
    expect(document.activeElement).toBe(button("Confirm unlink"));

    button("Cancel").click();
    await settle();
    expect(focusedUnlinkProvider()).toBe("github");
  });

  it("cancelling the confirm step leaves the identity linked", async () => {
    const { unlinkIdentity } = await mountComponent(
      summary({ identities: [identity("github"), identity("google")] }),
    );
    button("Unlink").click();
    await settle();
    button("Cancel").click();
    await settle();
    expect(unlinkIdentity).not.toHaveBeenCalled();
    expect(button("Unlink").disabled).toBe(false);
  });

  it("shows the server 409 message inline and restores focus to the row button", async () => {
    const { unlinkIdentity } = await mountComponent(
      summary({ identities: [identity("github"), identity("google")] }),
      vi.fn(),
      conflict("Add a password or another sign-in method first"),
    );
    button("Unlink").click();
    await settle();
    button("Confirm unlink").click();
    await settle();
    expect(unlinkIdentity).toHaveBeenCalled();
    expect(errorTexts()).toContain("Add a password or another sign-in method first");
    expect(focusedUnlinkProvider()).toBe("github");
  });

  it("fires onAnonymous on an unlink 401", async () => {
    const onAnonymous = vi.fn();
    const getIdentities = vi
      .fn()
      .mockResolvedValue(ok(summary({ identities: [identity("github"), identity("google")] })));
    const unlinkIdentity = vi.fn().mockResolvedValue(anonymous<IdentitySummary>());
    await mountWith(getIdentities, unlinkIdentity, { onAnonymous });

    button("Unlink").click();
    await settle();
    button("Confirm unlink").click();
    await settle();
    expect(unlinkIdentity).toHaveBeenCalledWith("github");
    expect(onAnonymous).toHaveBeenCalledTimes(1);
  });
});

describe("ConnectedAccounts anonymous sessions", () => {
  it("fires onAnonymous on a load 401", async () => {
    const onAnonymous = vi.fn();
    const getIdentities = vi.fn().mockResolvedValue(anonymous<IdentitySummary>());
    await mountWith(getIdentities, vi.fn(), { onAnonymous });
    expect(onAnonymous).toHaveBeenCalledTimes(1);
    expect(errorTexts()).toEqual([]);
  });

  it("surfaces session-expired copy on a load 401 without a handler", async () => {
    const getIdentities = vi.fn().mockResolvedValue(anonymous<IdentitySummary>());
    await mountWith(getIdentities, vi.fn());
    expect(errorTexts()).toContain(copy.connectedSessionExpiredError);
  });

  it("surfaces session-expired copy on an unlink 401 without a handler", async () => {
    const getIdentities = vi
      .fn()
      .mockResolvedValue(ok(summary({ identities: [identity("github"), identity("google")] })));
    const unlinkIdentity = vi.fn().mockResolvedValue(anonymous<IdentitySummary>());
    await mountWith(getIdentities, unlinkIdentity);

    button("Unlink").click();
    await settle();
    button("Confirm unlink").click();
    await settle();
    expect(errorTexts()).toContain(copy.connectedSessionExpiredError);
  });
});

describe("ConnectedAccounts link flow", () => {
  it("starts the full-page link flow for each unlinked provider", async () => {
    const navigate = vi.fn();
    await mountComponent(
      summary({
        identities: [identity("github")],
        hasPassword: true,
        linkableProviders: ["github", "google", "discord"],
      }),
      navigate,
    );

    expect(buttons().some((b) => b.textContent?.trim() === "Link GitHub")).toBe(false);
    button("Link Google").click();
    expect(navigate).toHaveBeenCalledWith(`${API_BASE}/auth/google/v1/link`);
    button("Link Discord").click();
    expect(navigate).toHaveBeenCalledWith(`${API_BASE}/auth/discord/v1/link`);
  });
});

describe("ConnectedAccounts transport failures", () => {
  it("recovers from a thrown load via Retry instead of spinning forever", async () => {
    const getIdentities = vi
      .fn()
      .mockRejectedValueOnce(new Error("transport"))
      .mockResolvedValueOnce(ok(summary({ identities: [identity("github")], passkeyCount: 1 })));
    await mountWith(getIdentities, vi.fn());

    expect(errorTexts()).toContain(GENERIC_TRY_AGAIN);
    expect(target.textContent).not.toContain(copy.connectedLoading);

    button("Retry").click();
    await settle();
    expect(getIdentities).toHaveBeenCalledTimes(2);
    expect(errorTexts()).toEqual([]);
    expect(target.textContent).toContain("GitHub");
  });

  it("settles the confirm state when unlink throws", async () => {
    const getIdentities = vi
      .fn()
      .mockResolvedValue(ok(summary({ identities: [identity("github"), identity("google")] })));
    const unlinkIdentity = vi.fn().mockRejectedValueOnce(new Error("transport"));
    await mountWith(getIdentities, unlinkIdentity);

    button("Unlink").click();
    await settle();
    button("Confirm unlink").click();
    await settle();

    expect(unlinkIdentity).toHaveBeenCalledWith("github");
    expect(errorTexts()).toContain(copy.connectedUnlinkError);
    expect(buttons().some((b) => b.textContent?.trim() === "Confirm unlink")).toBe(false);
    expect(target.textContent).not.toContain(copy.pleaseWait);
    expect(button("Unlink").disabled).toBe(false);
    expect(focusedUnlinkProvider()).toBe("github");
  });
});
