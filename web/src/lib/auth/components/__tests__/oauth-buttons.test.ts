import { mount, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
vi.mock("$app/env", () => ({ browser: true }));
vi.mock("$app/env/public", () => ({
  PUBLIC_API_BASE_URL: "https://eq.test/api",
  PUBLIC_QURAN_API_BASE: undefined,
  PUBLIC_ENV: undefined,
  PUBLIC_FCM_VAPID_KEY: undefined,
}));

const { gotoMock } = vi.hoisted(() => ({ gotoMock: vi.fn().mockResolvedValue(undefined) }));
vi.mock("$app/navigation", () => ({ goto: gotoMock }));

import { authModal } from "#lib/auth/auth-modal.svelte.js";
import type { OAuthFlow } from "#lib/auth/oauth-flow.svelte.js";
import { getAuthCopy } from "#lib/i18n/auth-copy.js";

import OAuthButtons from "../OAuthButtons.svelte";

const copy = getAuthCopy("en");

interface FlowOverrides {
  beginPopup?: ReturnType<typeof vi.fn>;
  begin?: ReturnType<typeof vi.fn>;
}

function fakeFlow(over: FlowOverrides = {}): OAuthFlow {
  // SAFETY: test double — OAuthButtons reads pending/lastErrorCode and calls begin/beginPopup only.
  return {
    pending: false,
    lastErrorCode: null,
    begin: over.begin ?? vi.fn().mockResolvedValue(true),
    beginPopup: over.beginPopup ?? vi.fn().mockResolvedValue("cancelled"),
  } as never;
}

interface FakeFlows {
  google: OAuthFlow;
  github: OAuthFlow;
  discord: OAuthFlow;
}

function fakeFlows(google: FlowOverrides): FakeFlows {
  return { google: fakeFlow(google), github: fakeFlow(), discord: fakeFlow() };
}

let target: HTMLElement;
let unmountComponent: () => void = () => {};

async function settle(): Promise<void> {
  for (let i = 0; i < 5; i++) await Promise.resolve();
  await new Promise((resolve) => setTimeout(resolve, 0));
}

function providerButton(label: string): HTMLButtonElement {
  const el = target.querySelector<HTMLButtonElement>(
    `button[aria-label="${copy.continueWithProvider(label)}"]`,
  );
  if (!el) throw new Error(`missing ${label} button`);
  return el;
}

function mountButtons(flows: FakeFlows): void {
  const instance = mount(OAuthButtons, { target, props: { flows } });
  unmountComponent = () => {
    void unmount(instance);
  };
}

beforeEach(() => {
  gotoMock.mockClear();
  target = document.createElement("div");
  document.body.appendChild(target);
});

afterEach(() => {
  unmountComponent();
  target.remove();
  vi.restoreAllMocks();
});

describe("OAuthButtons.begin outcome mapping", () => {
  it("completed closes the modal and navigates to the success route", async () => {
    const beginPopup = vi.fn().mockResolvedValue("completed");
    const begin = vi.fn().mockResolvedValue(true);
    mountButtons(fakeFlows({ beginPopup, begin }));
    const close = vi.spyOn(authModal, "close");

    providerButton("Google").click();
    await settle();

    expect(beginPopup).toHaveBeenCalledTimes(1);
    expect(close).toHaveBeenCalledTimes(1);
    expect(gotoMock).toHaveBeenCalledWith("/auth/google/success");
    expect(begin).not.toHaveBeenCalled();
  });

  for (const outcome of ["blocked", "cancelled", "failed", "unavailable"]) {
    it(`${outcome} falls back to the full-page begin()`, async () => {
      const beginPopup = vi.fn().mockResolvedValue(outcome);
      const begin = vi.fn().mockResolvedValue(true);
      mountButtons(fakeFlows({ beginPopup, begin }));

      providerButton("Google").click();
      await settle();

      expect(beginPopup).toHaveBeenCalledTimes(1);
      expect(begin).toHaveBeenCalledTimes(1);
      expect(gotoMock).not.toHaveBeenCalled();
    });
  }

  it("in_flight is a no-op (no fallback, no navigation)", async () => {
    const beginPopup = vi.fn().mockResolvedValue("in_flight");
    const begin = vi.fn().mockResolvedValue(true);
    mountButtons(fakeFlows({ beginPopup, begin }));

    providerButton("Google").click();
    await settle();

    expect(beginPopup).toHaveBeenCalledTimes(1);
    expect(begin).not.toHaveBeenCalled();
    expect(gotoMock).not.toHaveBeenCalled();
  });

  it("non-google providers keep the full-page begin()", async () => {
    const githubBegin = vi.fn().mockResolvedValue(true);
    const flows = fakeFlows({});
    flows.github = fakeFlow({ begin: githubBegin });
    mountButtons(flows);

    providerButton("GitHub").click();
    await settle();

    expect(githubBegin).toHaveBeenCalledTimes(1);
    expect(gotoMock).not.toHaveBeenCalled();
  });
});
