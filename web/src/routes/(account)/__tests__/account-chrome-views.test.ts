import { createRawSnippet, flushSync, mount, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

// SAFETY: the test drives only these three session states the layout branches on, so the literal union is exact.
const h = vi.hoisted(() => ({
  status: "unknown" as "unknown" | "anonymous" | "authenticated",
  probe: vi.fn(),
  goto: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("$app/env", () => ({ browser: true, dev: false }));
vi.mock("$app/env/public", () => ({
  PUBLIC_API_BASE_URL: undefined,
  PUBLIC_QURAN_API_BASE: undefined,
  PUBLIC_ENV: undefined,
  PUBLIC_FCM_VAPID_KEY: undefined,
}));
vi.mock("$app/state", () => ({
  page: {
    url: new URL("https://eq.test/account"),
    params: {},
  },
}));
vi.mock("$app/navigation", () => ({ goto: h.goto }));
vi.mock("#lib/auth/purge-hook.js", () => ({ installPurgeHook: vi.fn() }));
vi.mock("#lib/auth/auth-state.svelte.js", () => ({
  authState: {
    get status() {
      return h.status;
    },
    get user() {
      return null;
    },
    hydrate: vi.fn(),
    probe: h.probe,
  },
}));

import { footer_read_quran } from "#lib/i18n/m/chrome.js";
import { marketingReaderHomeHref } from "#lib/i18n/marketing-copy.js";

import AccountLayout from "../+layout.svelte";

const EXIT_HREF = marketingReaderHomeHref("en");
const EXIT_LABEL = footer_read_quran(undefined, { locale: "en" });

const CHILD = createRawSnippet(() => ({ render: () => "<span>page</span>" }));

let target: HTMLElement;
let instance: ReturnType<typeof mount> | null = null;

async function settle(): Promise<void> {
  for (let i = 0; i < 5; i++) await Promise.resolve();
  await new Promise((resolve) => setTimeout(resolve, 0));
  flushSync();
}

function mountLayout(): void {
  instance = mount(AccountLayout, {
    target,
    props: { children: CHILD },
  });
}

beforeEach(() => {
  target = document.createElement("div");
  document.body.appendChild(target);
  h.goto.mockClear();
});

afterEach(() => {
  if (instance) void unmount(instance);
  instance = null;
  target.remove();
});

function exitAnchors(): HTMLAnchorElement[] {
  return [...target.querySelectorAll<HTMLAnchorElement>("header a")].filter(
    (a) => a.textContent?.trim() === EXIT_LABEL,
  );
}

describe("account layout top bar and exit across session states", () => {
  it("shows the bar and exit while the session is still loading", async () => {
    h.status = "unknown";
    h.probe.mockReturnValue(new Promise(() => {}));
    mountLayout();
    await settle();

    expect(target.querySelector("header")).not.toBeNull();
    expect(exitAnchors()).toHaveLength(1);
    expect(exitAnchors()[0]?.getAttribute("href")).toBe(EXIT_HREF);
  });

  it("shows the bar and exit for an anonymous visitor", async () => {
    h.status = "anonymous";
    h.probe.mockResolvedValue(undefined);
    mountLayout();
    await settle();

    expect(exitAnchors()).toHaveLength(1);
    expect(exitAnchors()[0]?.getAttribute("href")).toBe(EXIT_HREF);
  });

  it("shows the bar and exit for an authenticated visitor", async () => {
    h.status = "authenticated";
    h.probe.mockResolvedValue(undefined);
    mountLayout();
    await settle();

    expect(exitAnchors()).toHaveLength(1);
    expect(exitAnchors()[0]?.getAttribute("href")).toBe(EXIT_HREF);
  });
});
