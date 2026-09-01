<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { page } from "$app/state";
  import { goto, replaceState } from "$app/navigation";
  import amiriArabic from "@fontsource/amiri/files/amiri-arabic-400-normal.woff2?url";
  import { Nav } from "$lib/components/nav";
  import { Footer } from "$lib/components/footer";
  import { Tweaks } from "$lib/components/tweaks";
  import { SITE } from "$lib/config/site";
  import { SUPPORTED_UI_LOCALES, UI_LOCALES } from "$lib/i18n/locales";
  import { footerLinksFor } from "$lib/i18n/footer-links";
  import { setAppLocale } from "$lib/i18n/app-locale";
  import { getReaderUiCopy } from "$lib/i18n/reader-copy";
  import { bookmarksPageHref, readerHrefFor, yoursPageHref, type QuranReaderHref } from "$lib/i18n/reader";
  import { publicHref } from "$lib/i18n/public-href";
  import { marketingHomeHref, type LocaleLink } from "$lib/i18n/marketing-copy";
  import { deLocalizeUrl } from "$lib/paraglide/runtime";
  import { reader } from "$lib/stores/reader.svelte";
  import { authState } from "$lib/auth/auth-state.svelte";
  import { bookmarks } from "$lib/bookmarks/store.svelte";
  import { stackedTranslations } from "$lib/stores/stacked-translations.svelte";
  import { modeParamMatches, parseModeParam, withModeParam } from "$lib/reader/mode-param";
  import { moreParamMatches, parseMoreParam, withMoreParam } from "$lib/reader/more-param";
  import { quranWorker } from "$lib/quran/worker-client";
  import { TRANSLATION_CATALOGUE_BY_ID } from "$lib/quran/catalogue";
  import { translationIdFromSegments } from "$lib/data/quran";

  let { data, children } = $props();
  let menuOpen = $state(false);
  const copy = getReaderUiCopy();
  // De-localized app pages (/app/bookmarks, ...) cannot recover this locale from their own URL;
  // publish it so their copy follows the same locale the nav/footer render.
  setAppLocale(copy.locale);
  const SETTINGS_PATH = "/app/settings";
  const isNonReaderAppRoute = $derived(
    (page.route.id ?? "").endsWith("/app/settings") ||
      (page.route.id ?? "").endsWith("/app/search") ||
      (page.route.id ?? "").endsWith("/app/bookmarks") ||
      (page.route.id ?? "").endsWith("/app/yours"),
  );

  const canonicalReaderHref = $derived.by<QuranReaderHref>(() => {
    const canonical = deLocalizeUrl(page.url);
    const pathname = canonical.pathname.replace(/\/+$/, "") || "/";
    // SAFETY: this layout only mounts on /app reader routes and deLocalizeUrl strips the locale
    // prefix, so the rebuilt path is always a reader route.
    return `${pathname}${canonical.search}${canonical.hash}` as QuranReaderHref;
  });
  // The settings and search pages have no localized variant the server will render, so their
  // locale switcher and footer reader links fall back to the localized reader home instead of a 404.
  const chromeReaderHref = $derived<QuranReaderHref>(
    isNonReaderAppRoute ? "/app" : canonicalReaderHref,
  );
  const currentReaderHref = $derived(readerHrefFor(copy.locale, chromeReaderHref));
  const localeLinks = $derived.by<LocaleLink[]>(() =>
    SUPPORTED_UI_LOCALES.map((locale) => ({
      locale,
      direction: UI_LOCALES[locale].direction,
      label: UI_LOCALES[locale].endonym,
      href: readerHrefFor(locale, chromeReaderHref),
      current: locale === copy.locale,
    })),
  );
  const footerLinks = $derived(
    footerLinksFor(copy.locale, copy.footerLinks, currentReaderHref, bookmarksPageHref()),
  );
  // Inline desktop nav links — the same reader indexes the marketing header links.
  const indexLinks = $derived([
    { label: copy.index.surahsTitle, href: publicHref(readerHrefFor(copy.locale, "/app/surah")) },
    { label: copy.index.juzTitle, href: publicHref(readerHrefFor(copy.locale, "/app/juz")) },
    { label: copy.index.pagesTitle, href: publicHref(readerHrefFor(copy.locale, "/app/pages")) },
    { label: copy.index.yoursTitle, href: publicHref(yoursPageHref()) },
  ]);
  const knownMoreIds = (ids: readonly string[]): string[] =>
    ids.filter((id) => TRANSLATION_CATALOGUE_BY_ID.has(id));

  function openSettings(): void {
    void goto(publicHref(SETTINGS_PATH));
  }

  onMount(() => {
    reader.hydrate(parseModeParam(page.url) ?? undefined);
    bookmarks.hydrate();
    const moreIds = knownMoreIds(parseMoreParam(page.url));
    if (moreIds.length) stackedTranslations.setIds(moreIds);
    document.documentElement.dataset.readerHydrated = "true";

    const syncMenu = (): void => {
      menuOpen = document.getElementById("site-panel") !== null;
    };
    syncMenu();
    const observer = new MutationObserver(syncMenu);
    observer.observe(document.body, { childList: true, subtree: true });

    return () => observer.disconnect();
  });

  $effect(() => {
    if (isNonReaderAppRoute) return;
    const url = page.url;
    const current = untrack(() => reader.mode);
    const param = parseModeParam(url);
    if (param && param !== current) reader.setMode(param);
    if (!modeParamMatches(url, current)) {
      try {
        replaceState(withModeParam(url, current), page.state);
      } catch {
        // Dev navigation can dispose this reader layout before SvelteKit finishes
        // initializing its router; next navigation restores the mode parameter.
      }
    }
  });

  $effect(() => {
    if (isNonReaderAppRoute) return;
    const url = page.url;
    const parsed = parseMoreParam(url);
    if (parsed.length > 0) {
      const ids = untrack(() => stackedTranslations.ids);
      const known = knownMoreIds(parsed);
      if (known.length && !moreParamMatches(url, ids)) stackedTranslations.setIds(known);
    }
    const effective = untrack(() => stackedTranslations.ids);
    if (!moreParamMatches(url, effective)) {
      try {
        replaceState(withMoreParam(url, effective), page.state);
      } catch {
        // dev-only: layout effects can run before the SvelteKit router initializes on a cold
        // load; the next navigation re-runs this effect and syncs the URL.
      }
    }
  });

  // Reader script preference → worker default for Arabic reads without a pinned
  // source. Arabic routes stay canonical/SSG (uthmani first paint); the variant
  // corpus is served by the worker/API upgrade after hydration.
  $effect(() => {
    quranWorker.setPreferredArabicSource(reader.arabicScript);
  });

  $effect(() => {
    const ids = stackedTranslations.ids;
    const lang = page.params.lang;
    const translator = page.params.translator;
    const primary = lang && translator ? translationIdFromSegments(lang, translator) : null;
    const pinned = primary ? [primary, ...ids] : [...ids];
    void quranWorker.setPinnedTranslations(pinned).catch(() => {});
  });

  // Bookmark sync follows the session: anon→authed migrates legacy local
  // bookmarks and starts the engine; authed→anon drops the server view. The
  // effect body must not depend on store state, hence untrack. Status
  // "unknown" (probe not yet resolved) is NOT an anon edge: acting on it would
  // run the logout path — wiping the durable outbox — for every authed cold
  // load; the store's onAuthChanged guards the same seam a second time.
  // DEFERRED: an authed cold-load still shows the anon legacy view until the
  // probe resolves.
  $effect(() => {
    const status = authState.status;
    if (status === "unknown") return;
    const authed = authState.authenticated;
    untrack(() => bookmarks.onAuthChanged(authed));
  });

  // Registered asynchronously so `@tanstack/hotkeys` stays out of the initial bundle — same
  // shape as GlobalSearch.svelte. Mod+, opens the settings page from any reader surface.
  $effect(() => {
    let destroyed = false;
    let cleanup: (() => void) | undefined;

    void import("$lib/hotkeys.svelte").then(({ registerHotkey }) => {
      if (destroyed) return;
      const openSettingsHotkey = registerHotkey("Mod+,", (event) => {
        // IME: don't let a composition session's chord hijack the page.
        if (event.isComposing) return;
        openSettings();
      }, { meta: { name: "Open settings" } });
      cleanup = () => {
        openSettingsHotkey.unregister();
      };
    });

    return () => {
      destroyed = true;
      cleanup?.();
    };
  });

  // Dynamic import on purpose: the reader appearance panel's copy spans several message
  // namespaces (reader-settings, controls, settings, reader) and nothing renders until the user
  // opens the panel. See docs/quran-system.md (Part 2, Message chunking).
  const loadReaderSettingsCopy = async () => {
    const { getReaderSettingsCopy } = await import("$lib/i18n/reader-settings-copy");
    return getReaderSettingsCopy(copy.locale);
  };
</script>

<svelte:head>
  <link rel="preload" href={amiriArabic} as="font" type="font/woff2" crossorigin="anonymous" />
</svelte:head>

<div lang={copy.locale} dir={copy.direction} data-reader-root class="flex min-h-screen flex-col">
  <a
    href="#main"
    class="sr-only focus:not-sr-only focus:absolute focus:start-3 focus:top-3 focus:z-[101] focus:rounded-sm focus:bg-surface focus:px-3 focus:py-2 focus:text-sm focus:text-foreground"
  >{copy.skipToContent}</a>
  <Nav
    collapsible
    copy={copy.nav}
    brandCopy={{ homeLabel: copy.nav.homeLabel(SITE.name) }}
    brandHomeHref={marketingHomeHref(copy.locale)}
    {indexLinks}
    {localeLinks}
    direction={copy.direction}
  />
  <main
    id="main"
    tabindex="-1"
    inert={menuOpen || undefined}
    aria-hidden={menuOpen || undefined}
    class="flex-1 pb-28"
  >{@render children()}</main>
  <Footer owner={data.owner} year={data.year} copy={copy.footer} links={footerLinks} />
</div>
<Tweaks
  locale={copy.locale}
  triggerLabel={copy.appearanceTrigger}
  loadCopy={loadReaderSettingsCopy}
/>

<style>
  :global(body:has([data-reader-root]) > a[href="#main"]) {
    display: none;
  }
</style>
