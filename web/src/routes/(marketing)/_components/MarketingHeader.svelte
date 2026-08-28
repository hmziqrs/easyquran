<script lang="ts">
  import { onMount } from "svelte";
  import { prefs } from "$lib/stores/prefs.svelte";
  import { authState } from "$lib/auth/auth-state.svelte";
  import { authModal } from "$lib/auth/auth-modal.svelte";
  import { Button, Icon } from "$lib/components";
  import { commandPalette } from "$lib/stores/command-palette.svelte";
  import { loadPalette } from "$lib/components/search/palette-loader";
  import {
    marketingHomeHref,
    marketingReaderHomeHref,
    type MarketingLocale,
  } from "$lib/i18n/marketing-copy";
  import { marketingHref } from "$lib/i18n/marketing";
  import { publicHref } from "$lib/i18n/public-href";
  import { bookmarksPageHref, readerHrefFor } from "$lib/i18n/reader";
  import { globalPagePathFor, juzPathFor } from "$lib/data/quran";
  import { SITE } from "$lib/config/site";
  import {
    brand_home_label,
    nav_about,
    nav_account,
    nav_bookmarks,
    nav_change_language,
    nav_header_search,
    nav_juz,
    nav_pages,
    nav_primary_label,
    nav_sign_in,
    nav_start_reading,
    nav_surahs,
    nav_toggle_theme,
  } from "$lib/i18n/m/chrome";
  import { UI_LOCALES } from "$lib/i18n/locales";

  /**
   * Board band 1 (design/PillLightCobalt.dc.html): a ruled 80px header — wordmark,
   * section links, an inline pill search that opens the global palette (⌘K chord is
   * owned by GlobalSearch via the shared TanStack registry), a mode toggle and the
   * primary CTA. Replaces the marketing Nav wrapper; Nav.svelte stays for app chrome
   * (area 6). Deviations kept for function: account entry (only sign-in door on
   * marketing) and the locale switch the old site panel exposed.
   */
  let { locale }: { locale: MarketingLocale } = $props();

  onMount(() => {
    authState.hydrate();
  });

  const arabicCtx = { kind: "arabic" } as const;
  const brand = SITE.name.toLowerCase();
  const otherLocale: MarketingLocale = $derived(locale === "en" ? "ar" : "en");

  const aboutHref = $derived(marketingHref("about", locale));
  const t = $derived.by(() => {
    const options = { locale } as const;
    return {
      brandHome: brand_home_label(undefined, options),
      primary: nav_primary_label(undefined, options),
      surahs: nav_surahs(undefined, options),
      juz: nav_juz(undefined, options),
      pages: nav_pages(undefined, options),
      bookmarks: nav_bookmarks(undefined, options),
      about: nav_about(undefined, options),
      search: nav_header_search(undefined, options),
      startReading: nav_start_reading(undefined, options),
      toggleTheme: nav_toggle_theme(undefined, options),
      changeLanguage: nav_change_language(undefined, options),
      account: nav_account(undefined, options),
      signIn: nav_sign_in(undefined, options),
    };
  });
  let accountLabel = $derived(authState.authenticated ? t.account : t.signIn);
  let accountHref: "/account" | "/login" = $derived(authState.authenticated ? "/account" : "/login");

  function onAccountClick(e: MouseEvent): void {
    if (authState.authenticated) return;
    if (e.defaultPrevented || e.button !== 0) return;
    if (e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
    e.preventDefault();
    authModal.show("login");
  }

  function openSearch(): void {
    void loadPalette();
    commandPalette.show();
  }
</script>

<header class="sticky top-0 z-50 border-b border-border bg-surface">
  <div
    class="mx-auto flex h-16 w-full items-center gap-4 px-5 md:gap-6 md:px-8 lg:h-[72px] lg:px-12 xl:h-20 xl:gap-[26px] xl:px-18"
  >
    <a
      class="flex flex-none items-center gap-2.5"
      href={publicHref(marketingHomeHref(locale))}
      aria-label={t.brandHome}
    >
      <span
        class="flex size-9 items-center justify-center rounded-sm bg-primary font-arabic text-[19px] font-bold leading-none text-primary-foreground"
        lang="ar"
        dir="rtl"
        aria-hidden="true">ق</span
      >
      <span class="text-[22px] font-extrabold tracking-[-0.035em] text-foreground"
        >{brand.slice(0, 4)}<span class="text-primary">{brand.slice(4)}</span></span
      >
    </a>

    <nav
      class="hidden items-center gap-6 text-[16.5px] font-bold lg:flex"
      aria-label={t.primary}
    >
      <a class="text-foreground transition-colors hover:text-primary"
        href={publicHref(marketingReaderHomeHref(locale))}>{t.surahs}</a
      >
      <a class="text-foreground transition-colors hover:text-primary"
        href={publicHref(readerHrefFor(locale, juzPathFor(arabicCtx, 1)))}>{t.juz}</a
      >
      <a class="text-foreground transition-colors hover:text-primary"
        href={publicHref(readerHrefFor(locale, globalPagePathFor(arabicCtx, 1)))}>{t.pages}</a
      >
      <a class="text-muted transition-colors hover:text-primary"
        href={publicHref(bookmarksPageHref())}>{t.bookmarks}</a
      >
      {#if aboutHref}
        <a class="text-muted transition-colors hover:text-primary" href={publicHref(aboutHref)}
          >{t.about}</a
        >
      {/if}
    </nav>

    <button
      type="button"
      onclick={openSearch}
      onpointerenter={() => void loadPalette()}
      onfocus={() => void loadPalette()}
      aria-label={t.search}
      aria-keyshortcuts="Meta+K Control+K"
      class="group ms-auto flex h-11 min-w-0 flex-grow items-center gap-3 rounded-pill border border-border px-5 text-start transition-colors duration-150 hover:border-border-strong hover:bg-surface-hover focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
    >
      <Icon name="search" size={17} class="flex-none text-muted" />
      <span class="min-w-0 flex-grow truncate text-body text-muted">{t.search}</span>
      <span
        class="hidden flex-none items-center gap-[3px] rounded-pill bg-background-subtle px-[9px] py-[5px] text-[13px] font-bold text-muted sm:flex"
      >
        <span aria-hidden="true">⌘</span><span aria-hidden="true">K</span>
      </span>
    </button>

    <a
      class="hidden flex-none text-[16.5px] font-bold text-muted transition-colors hover:text-primary md:block"
      href={publicHref(marketingHomeHref(otherLocale))}
      hreflang={otherLocale}
      lang={otherLocale}
      aria-label={t.changeLanguage}
      data-sveltekit-reload
    >{UI_LOCALES[otherLocale].endonym}</a>

    <button
      type="button"
      onclick={() => prefs.toggleTheme()}
      aria-label={t.toggleTheme}
      title={t.toggleTheme}
      class="flex-none inline-flex size-11 items-center justify-center rounded-pill border border-border text-foreground transition-colors duration-150 hover:bg-surface-hover focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
    >
      <Icon name={prefs.theme === "dark" ? "moon" : "sun"} size={19} />
    </button>

    <a
      href={publicHref(accountHref)}
      aria-label={accountLabel}
      title={accountLabel}
      onclick={onAccountClick}
      class="flex-none inline-flex size-11 items-center justify-center rounded-pill border border-border text-foreground transition-colors duration-150 hover:bg-surface-hover focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
    >
      <Icon name="user" size={19} title={accountLabel} />
    </a>

    <Button
      variant="primary"
      size="md"
      href={publicHref(marketingReaderHomeHref(locale))}
      class="hidden flex-none font-extrabold sm:inline-flex"
    >{t.startReading}</Button>
  </div>
</header>
