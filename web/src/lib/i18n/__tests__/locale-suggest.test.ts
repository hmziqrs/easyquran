import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

const trackMock = vi.hoisted(() => vi.fn(() => Promise.resolve()));
vi.mock("#lib/firebase/analytics.js", () => ({ track: trackMock }));

import { authModal } from "#lib/auth/auth-modal.svelte.js";
import {
  LOCALE_SUGGEST_STORAGE_KEY,
  LocaleSuggestStore,
  buildTargetHref,
  decodeResolved,
  isLocaleSuggestSurface,
} from "#lib/i18n/locale-suggest.svelte.js";

// eslint-disable-next-line anti-slop/no-unknown-parameters -- minimal navigator double: the paraglide runtime reads only .languages, and matrix rows feed it exact browser-language lists.
function stubLanguages(languages: readonly string[]): void {
  vi.stubGlobal("navigator", { languages });
}

function freshStore(languages: readonly string[], pathname: string): LocaleSuggestStore {
  const store = new LocaleSuggestStore();
  store.hydrate();
  stubLanguages(languages);
  store.evaluate(pathname, "", "");
  return store;
}

beforeEach(() => {
  localStorage.clear();
  trackMock.mockClear();
});
afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  authModal.close();
});

describe("locale suggest trigger matrix (plan §3)", () => {
  it("the Pakistan guarantee: en-PK browser on / never sees a popup; silence for unsupported, empty, same-locale", () => {
    expect(freshStore(["en-PK", "en", "ur"], "/").open).toBe(false);
    expect(freshStore(["ur-PK", "en"], "/").open).toBe(false);
    expect(freshStore(["fr"], "/").open).toBe(false);
    expect(freshStore([], "/").open).toBe(false);
    expect(freshStore(["ar"], "/ar/").open).toBe(false);
  });

  it("suggests ar for an ar-primary browser on /, with /ar/ as the accept link", () => {
    const store = freshStore(["ar", "en"], "/");
    expect(store.open).toBe(true);
    expect(store.target).toBe("ar");
    expect(store.href).toBe("/ar/");
  });

  it("keeps translation context on a deep link (plan §8.1 Chrome-reduction case)", () => {
    const store = freshStore(["ar"], "/al-fatihah/t/en/sahih");
    expect(store.open).toBe(true);
    expect(store.target).toBe("ar");
    expect(store.href).toBe("/ar/al-fatihah/t/en/sahih");
    const reversed = freshStore(["en"], "/ar/al-fatihah/t/en/sahih");
    expect(reversed.href).toBe("/al-fatihah/t/en/sahih");
  });

  it("skips surfaces without a localized twin and records nothing", () => {
    for (const pathname of [
      "/about",
      "/ar/about",
      "/login",
      "/health",
      "/app/settings",
      "/faq.md",
    ]) {
      expect(freshStore(["ar", "en"], pathname).open, pathname).toBe(false);
    }
    expect(localStorage.getItem(LOCALE_SUGGEST_STORAGE_KEY)).toBeNull();
  });

  it("surface allowlist (plan §3 row 5, scheme A): home, reader routes, bounded app chrome", () => {
    const eligible = [
      "/",
      "/ar",
      "/ar/",
      "/al-fatihah",
      "/juz",
      "/juz/3",
      "/page/5",
      "/hizb/2",
      "/surah",
      "/pages",
      "/search",
      "/settings",
      "/bookmarks",
      "/yours",
      "/al-fatihah/t/en/sahih",
      "/t/en/sahih/juz/3",
      "/ar/al-fatihah",
      "/ar/search",
    ];
    for (const pathname of eligible) expect(isLocaleSuggestSurface(pathname), pathname).toBe(true);
    const ineligible = [
      "/about",
      "/ar/about",
      "/faq",
      "/contact",
      "/privacy",
      "/terms",
      "/login",
      "/register",
      "/forgot-password",
      "/verify-email",
      "/account",
      "/auth",
      "/design",
      "/health",
      "/app/settings",
      "/t",
      "/rub",
      "/al-fatihah.md",
      "/al-fatihah.txt",
      "/favicon.ico",
      "/al-fatihah/",
    ];
    for (const pathname of ineligible)
      expect(isLocaleSuggestSurface(pathname), pathname).toBe(false);
  });

  it("never suggests a locale the user already answered; /ar normalizes so en lands on /", () => {
    for (const value of ["dismissed", "accepted"] as const) {
      localStorage.setItem(
        LOCALE_SUGGEST_STORAGE_KEY,
        JSON.stringify({ v: 1, resolved: { ar: value } }),
      );
      expect(freshStore(["ar", "en"], "/").open, value).toBe(false);
    }
    expect(buildTargetHref("/ar", "", "", "en")).toBe("/");
    // Same-path guard: an accept link that reloads the current page must never ship.
    expect(buildTargetHref("/", "", "", "en")).toBe("");
    expect(buildTargetHref("/", "?v=1", "#2:255", "ar")).toBe("/ar/?v=1#2:255");
  });
});

describe("storage codec (plan §4)", () => {
  it("drops junk: future versions, wrong shapes, bogus literals, unknown locales", () => {
    expect(decodeResolved(undefined)).toEqual({});
    expect(decodeResolved("not-an-object")).toEqual({});
    expect(decodeResolved({ resolved: "nope" })).toEqual({});
    expect(decodeResolved({ v: 2, resolved: { ar: "dismissed" } })).toEqual({});
    expect(decodeResolved({ v: 1, resolved: { ar: "bogus" } })).toEqual({});
    expect(decodeResolved({ v: 1, resolved: { ur: "dismissed", fr: "accepted" } })).toEqual({});
    expect(decodeResolved({ v: 1, resolved: { ar: "accepted", en: "dismissed" } })).toEqual({
      ar: "accepted",
      en: "dismissed",
    });
  });

  it("survives unreadable storage by behaving as first-visit", () => {
    localStorage.setItem(LOCALE_SUGGEST_STORAGE_KEY, "{broken json");
    expect(freshStore(["ar", "en"], "/").open).toBe(true);
  });
});

describe("dismissal contract (plan §3)", () => {
  it("writes a dismissal exactly once no matter how the dialog closes, and never re-suggests", () => {
    const store = freshStore(["ar", "en"], "/");
    const setItem = vi.spyOn(localStorage, "setItem");
    store.noteCloseIntent("escape");
    store.onDialogOpenChange(false);
    store.onDialogOpenChange(false);
    expect(store.open).toBe(false);
    expect(JSON.parse(localStorage.getItem(LOCALE_SUGGEST_STORAGE_KEY) ?? "null")).toEqual({
      v: 1,
      resolved: { ar: "dismissed" },
    });
    expect(setItem).toHaveBeenCalledTimes(1);
    stubLanguages(["ar", "en"]);
    store.evaluate("/");
    expect(store.open).toBe(false);
  });

  it("forced close records nothing and latches; accept persists accepted", () => {
    const forced = freshStore(["ar", "en"], "/");
    forced.forceClose();
    expect(localStorage.getItem(LOCALE_SUGGEST_STORAGE_KEY)).toBeNull();
    stubLanguages(["ar", "en"]);
    forced.evaluate("/");
    expect(forced.open).toBe(false);

    const store = freshStore(["ar", "en"], "/");
    store.accept();
    expect(JSON.parse(localStorage.getItem(LOCALE_SUGGEST_STORAGE_KEY) ?? "null")).toEqual({
      v: 1,
      resolved: { ar: "accepted" },
    });
  });
});

describe("lifecycle (plan §5.3)", () => {
  it("a suppressed surface burns nothing; a later evaluation still shows; the shown-latch holds", () => {
    const store = new LocaleSuggestStore();
    store.hydrate();
    stubLanguages(["ar", "en"]);
    store.evaluate("/about");
    expect(store.open).toBe(false);
    expect(localStorage.getItem(LOCALE_SUGGEST_STORAGE_KEY)).toBeNull();
    store.evaluate("/search");
    expect(store.open).toBe(true);
    store.forceClose();
    store.evaluate("/search");
    expect(store.open).toBe(false);
  });

  it("retry-once: a modal at fire blocks, the retry re-runs the matrix, a second block ends it", () => {
    const store = new LocaleSuggestStore();
    store.hydrate();
    stubLanguages(["ar", "en"]);
    authModal.show("login");
    store.evaluate("/");
    expect(store.open).toBe(false);
    authModal.close();
    store.retryIfArmed("/");
    expect(store.open).toBe(true);
    store.forceClose();
    authModal.show("login");
    store.evaluate("/");
    authModal.close();
    store.retryIfArmed("/");
    expect(store.open).toBe(false);
  });

  it("cross-tab: another tab's resolution closes the open dialog without writing", () => {
    const store = freshStore(["ar", "en"], "/");
    expect(store.open).toBe(true);
    const off = store.subscribeExternal();
    // The other tab's answer lands in storage, then the storage event fires here.
    localStorage.setItem(
      LOCALE_SUGGEST_STORAGE_KEY,
      JSON.stringify({ v: 1, resolved: { ar: "dismissed" } }),
    );
    const setItem = vi.spyOn(localStorage, "setItem");
    window.dispatchEvent(new StorageEvent("storage", { key: LOCALE_SUGGEST_STORAGE_KEY }));
    expect(store.open).toBe(false);
    expect(setItem).not.toHaveBeenCalled();
    stubLanguages(["ar", "en"]);
    store.evaluate("/search");
    expect(store.open).toBe(false);
    off();
  });

  it("blocked storage degrades to memory: the dismissal lands, nothing re-opens this load, nothing throws", () => {
    const store = new LocaleSuggestStore();
    store.hydrate();
    stubLanguages(["ar", "en"]);
    store.evaluate("/");
    expect(store.open).toBe(true);
    vi.spyOn(localStorage, "setItem").mockImplementation(() => {
      throw new Error("quota");
    });
    store.dismiss("button");
    expect(store.open).toBe(false);
    store.evaluate("/search");
    expect(store.open).toBe(false);
  });
});

describe("analytics (plan §5.5)", () => {
  it("fires shown/dismissed/accepted and never the browser language list", () => {
    const store = freshStore(["ar", "en"], "/");
    expect(trackMock).toHaveBeenCalledWith("locale_suggest_shown", { suggested_locale: "ar" });
    store.noteCloseIntent("overlay");
    store.onDialogOpenChange(false);
    expect(trackMock).toHaveBeenCalledWith("locale_suggest_dismissed", {
      suggested_locale: "ar",
      dismiss_reason: "overlay",
    });
    const accepting = freshStore(["ar", "en"], "/");
    accepting.accept();
    expect(trackMock).toHaveBeenCalledWith("locale_suggest_accepted", { suggested_locale: "ar" });
    for (const call of trackMock.mock.calls) {
      expect(JSON.stringify(call)).not.toContain("en-PK");
    }
  });
});
