<script lang="ts">
  import { onMount } from "svelte";
  import { prefs } from "$lib/stores/prefs.svelte";
  import { authState } from "$lib/auth/auth-state.svelte";
  import { authModal } from "$lib/auth/auth-modal.svelte";
  import { Icon } from "$lib/components";
  import { SearchTrigger } from "$lib/components/search";
  import { marketingHomeHref, type MarketingLocale } from "$lib/i18n/marketing-copy";
  import { marketingHref } from "$lib/i18n/marketing";
  import { publicHref } from "$lib/i18n/public-href";
  import { readerHrefFor, yoursPageHref } from "$lib/i18n/reader";
  import { SITE } from "$lib/config/site";
  import {
    brand_home_label,
    nav_about,
    nav_account,
    nav_change_language,
    nav_header_search,
    nav_juz,
    nav_pages,
    nav_primary_label,
    nav_sign_in,
    nav_surahs,
    nav_toggle_theme,
    nav_yours,
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
      yours: nav_yours(undefined, options),
      about: nav_about(undefined, options),
      search: nav_header_search(undefined, options),
      toggleTheme: nav_toggle_theme(undefined, options),
      changeLanguage: nav_change_language(undefined, options),
      account: nav_account(undefined, options),
      signIn: nav_sign_in(undefined, options),
    };
  });
  let accountLabel = $derived(authState.authenticated ? t.account : t.signIn);
  let accountHref: "/account" | "/login" = $derived(authState.authenticated ? "/account" : "/login");
  // Reader index links — the dedicated index pages, same grammar as the app Nav.
  const readerIndexLinks = $derived([
    { label: t.surahs, href: publicHref(readerHrefFor(locale, "/app/surah")), muted: false },
    { label: t.juz, href: publicHref(readerHrefFor(locale, "/app/juz")), muted: false },
    { label: t.pages, href: publicHref(readerHrefFor(locale, "/app/pages")), muted: false },
    { label: t.yours, href: publicHref(yoursPageHref()), muted: true },
  ]);

  function onAccountClick(e: MouseEvent): void {
    if (authState.authenticated) return;
    if (e.defaultPrevented || e.button !== 0) return;
    if (e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
    e.preventDefault();
    authModal.show("login");
  }

</script>

<header class="sticky top-0 z-50 border-b border-border bg-surface">
  <div class="mx-auto flex h-14 w-full items-center gap-4 px-5 md:gap-6 sm:px-7 lg:px-10">
    <a
      class="flex flex-none items-center gap-2.5"
      href={publicHref(marketingHomeHref(locale))}
      aria-label={t.brandHome}
    >
      <span
        class="flex size-8 items-center justify-center rounded-sm bg-primary font-arabic text-[17px] font-bold leading-none text-primary-foreground"
        lang="ar"
        dir="rtl"
        aria-hidden="true">ق</span
      >
      <span class="text-[20px] font-extrabold tracking-[-0.035em] text-foreground"
        >{brand.slice(0, 4)}<span class="text-primary">{brand.slice(4)}</span></span
      >
    </a>

    <nav
      class="hidden items-center gap-6 text-[15px] font-bold lg:flex"
      aria-label={t.primary}
    >
      {#each readerIndexLinks as link (link.href)}
        <a
          class={link.muted
            ? "text-muted transition-colors hover:text-primary"
            : "text-foreground transition-colors hover:text-primary"}
          href={link.href}>{link.label}</a
        >
      {/each}
      {#if aboutHref}
        <a class="text-muted transition-colors hover:text-primary" href={publicHref(aboutHref)}
          >{t.about}</a
        >
      {/if}
    </nav>

    <SearchTrigger variant="pill" label={t.search} class="ms-auto" />

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
      class="flex-none inline-flex size-10 items-center justify-center rounded-pill border border-border text-foreground transition-colors duration-150 hover:bg-surface-hover focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
    >
      <Icon name={prefs.theme === "dark" ? "moon" : "sun"} size={18} />
    </button>

    <a
      href={publicHref(accountHref)}
      aria-label={accountLabel}
      title={accountLabel}
      onclick={onAccountClick}
      class="flex-none inline-flex size-10 items-center justify-center rounded-pill border border-border text-foreground transition-colors duration-150 hover:bg-surface-hover focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
    >
      <Icon name="user" size={18} title={accountLabel} />
    </a>

  </div>
</header>
