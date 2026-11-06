// @vitest-environment node
import { readFileSync } from "node:fs";

import { createRawSnippet } from "svelte";
import { render } from "svelte/server";
import { describe, expect, it, vi } from "vite-plus/test";

vi.mock("$app/env", () => ({ browser: false, dev: false }));
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
vi.mock("$app/navigation", () => ({ goto: vi.fn().mockResolvedValue(undefined) }));

import { getAccountCopy } from "#lib/i18n/account-copy.js";
import { footer_read_quran } from "#lib/i18n/m/chrome.js";
import { marketingReaderHomeHref } from "#lib/i18n/marketing-copy.js";

import AccountLayout from "../+layout.svelte";

const SRC = (path: string): string => readFileSync(new URL(path, import.meta.url), "utf8");
const CHILD = createRawSnippet(() => ({ render: () => "<span>page</span>" }));

describe("account layout prerendered HTML", () => {
  it("ships the top bar and the Read Quran exit before any JS runs", () => {
    const { body } = render(AccountLayout, { props: { children: CHILD } });
    const copy = getAccountCopy("en");
    const exitLabel = footer_read_quran(undefined, { locale: "en" });
    const exitHref = marketingReaderHomeHref("en");

    // Exit anchor is in the static markup, not behind the session-probe gate.
    expect(body).toContain(`href="${exitHref}"`);
    expect(body).toContain(exitLabel);
    expect(body).toContain(copy.checkingSession);
    expect(body).toContain('lang="en"');
    expect(body).toContain('dir="ltr"');
    expect(body).toContain('id="main"');
  });

  it("localizes the shell and sets lang/dir for Arabic", () => {
    const copy = getAccountCopy("ar");
    expect(copy.direction).toBe("rtl");
    expect(copy.locale).toBe("ar");
    expect(copy.checkingSession).not.toBe(getAccountCopy("en").checkingSession);
  });
});

describe("account page source guards", () => {
  it("drops the hand-rolled session-row and passkey-card surfaces for Card", () => {
    const page = SRC("../account/+page.svelte");
    expect(page).not.toContain("border-line-2 bg-bg-2");
    expect(page).toContain("ui/card/index.js");
  });

  it("keeps the page free of hard-coded English UI strings", () => {
    const page = SRC("../account/+page.svelte");
    const layout = SRC("../+layout.svelte");
    for (const source of [page, layout]) {
      expect(source).not.toMatch(/>\s*[A-Z][a-z]+[^<{}]*</u);
    }
  });

  it("opens both CTAs through the auth modal, not a register link", () => {
    const page = SRC("../account/+page.svelte");
    expect(page).toContain('authModal.show("login")');
    expect(page).toContain('authModal.show("register")');
    expect(page).not.toContain("REGISTER_HREF");
  });
});
