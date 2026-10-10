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

import ConnectedAccounts from "../ConnectedAccounts.svelte";

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

async function mountComponent(
  data: IdentitySummary,
  navigate = vi.fn(),
  unlinkResponse?: AccountResult<IdentitySummary>,
) {
  const getIdentities = vi.fn().mockResolvedValue(ok(data));
  const unlinkIdentity = vi.fn();
  if (unlinkResponse !== undefined) unlinkIdentity.mockResolvedValue(unlinkResponse);
  const client = mockClient(getIdentities, unlinkIdentity);
  const instance = mount(ConnectedAccounts, { target, props: { client, navigate } });
  unmountComponent = () => {
    void unmount(instance);
  };
  await settle();
  return { getIdentities, unlinkIdentity, navigate };
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

  it("shows the server 409 message inline", async () => {
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
