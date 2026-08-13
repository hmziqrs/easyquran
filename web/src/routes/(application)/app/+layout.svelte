<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/state";
  import amiriArabic from "@fontsource/amiri/files/amiri-arabic-400-normal.woff2?url";
  import { Nav } from "$lib/components/nav";
  import { Footer } from "$lib/components/footer";
  import { Tweaks } from "$lib/components/tweaks";
  import { SITE } from "$lib/config/site";
  import { SUPPORTED_UI_LOCALES, UI_LOCALES } from "$lib/i18n/locales";
  import { footerLinksFor } from "$lib/i18n/footer-links";
  import { getReaderUiCopy } from "$lib/i18n/reader-copy";
  import { readerHrefFor, type QuranReaderHref } from "$lib/i18n/reader";
  import type { LocaleLink } from "$lib/i18n/marketing-copy";
  import { deLocalizeUrl } from "$lib/paraglide/runtime";
  import { reader } from "$lib/stores/reader.svelte";

  let { data, children } = $props();
  let menuOpen = $state(false);
  const copy = getReaderUiCopy();
  const canonicalReaderHref = $derived.by<QuranReaderHref>(() => {
    const canonical = deLocalizeUrl(page.url);
    const pathname = canonical.pathname.replace(/\/+$/, "") || "/";
    return `${pathname}${canonical.search}${canonical.hash}` as QuranReaderHref;
  });
  const currentReaderHref = $derived(readerHrefFor(copy.locale, canonicalReaderHref));
  const localeLinks = $derived.by<LocaleLink[]>(() =>
    SUPPORTED_UI_LOCALES.map((locale) => ({
      locale,
      direction: UI_LOCALES[locale].direction,
      label: UI_LOCALES[locale].endonym,
      href: readerHrefFor(locale, canonicalReaderHref),
      current: locale === copy.locale,
    })),
  );
  const footerLinks = $derived(footerLinksFor(copy.locale, copy.footerLinks, currentReaderHref));

  onMount(() => {
    reader.hydrate();
    document.documentElement.dataset.readerHydrated = "true";

    const syncMenu = (): void => {
      menuOpen = document.getElementById("site-panel") !== null;
    };
    syncMenu();
    const observer = new MutationObserver(syncMenu);
    observer.observe(document.body, { childList: true, subtree: true });

    return () => observer.disconnect();
  });

  // Dynamic import on purpose: the reader appearance panel owns ~34 messages that nothing renders
  // until the user opens the panel. See docs/i18n-bundle-plan.md.
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
    class="sr-only focus:not-sr-only focus:absolute focus:start-3 focus:top-3 focus:z-[101] focus:rounded focus:bg-bg-1 focus:px-3 focus:py-2 focus:text-sm focus:text-fg focus:shadow-lg"
  >{copy.skipToContent}</a>
  <Nav
    collapsible
    copy={copy.nav}
    brandCopy={{ homeLabel: copy.nav.homeLabel(SITE.name) }}
    brandHomeHref={currentReaderHref}
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
<Tweaks triggerLabel={copy.appearanceTrigger} loadCopy={loadReaderSettingsCopy} />

<style>
  :global(body:has([data-reader-root]) > a[href="#main"]) {
    display: none;
  }
</style>
