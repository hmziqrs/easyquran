<script lang="ts">
  import { page } from "$app/state";
  import { Bismillah, Button, Container, Icon } from "#lib/components/index.js";
  import { Nav } from "#lib/components/nav/index.js";
  import { Footer } from "#lib/components/footer/index.js";
  import { SITE } from "#lib/config/site.js";
  import { surahPathFor } from "#lib/data/quran.js";
  import { marketingFooterLinks, resolveChromeCopy } from "#lib/i18n/chrome-copy.js";
  import {
    marketingHomeHref,
    marketingLocaleFromPath,
    type LandingHue,
  } from "#lib/i18n/marketing-copy.js";
  import { publicHref } from "#lib/i18n/public-href.js";
  import { bookmarksPageHref, readerHrefFor, yoursPageHref } from "#lib/i18n/reader.js";

  let { status = 404, error }: { status?: number; error?: App.Error } = $props();

  const isNotFound = $derived(status === 404);
  const ARABIC_CTX = { kind: "arabic" } as const;

  const locale = $derived(marketingLocaleFromPath(page.url.pathname));
  const chrome = $derived(resolveChromeCopy(locale));
  const homeHref = $derived(publicHref(marketingHomeHref(locale)));
  const surahIndexHref = $derived(publicHref(readerHrefFor(locale, "/surah")));
  const footerLinks = $derived(marketingFooterLinks(locale));
  const year = new Date().getFullYear();

  const indexLinks = $derived([
    { label: chrome.index.surahs, href: publicHref(readerHrefFor(locale, "/surah")) },
    { label: chrome.index.juz, href: publicHref(readerHrefFor(locale, "/juz")) },
    { label: chrome.index.pages, href: publicHref(readerHrefFor(locale, "/pages")) },
    { label: chrome.index.yours, href: publicHref(yoursPageHref()) },
  ]);

  function pageTitle(): string {
    if (isNotFound) return "Page not found";
    return `${status} — error`;
  }

  function errorBody(): string {
    if (isNotFound) return `The page you're looking for isn't on ${SITE.domain}. It may have moved, or the link has a typo — pick a door below instead.`;
    if (error?.message) return error.message;
    return "Unexpected error.";
  }

  function retry(): void {
    location.reload();
  }

  const POPULAR = [
    { slug: "al-fatihah", num: 1, name: "Al-Fatihah", arabic: "الفاتحة", meaning: "The Opening", ayahs: 7 },
    { slug: "al-kahf", num: 18, name: "Al-Kahf", arabic: "الكهف", meaning: "The Cave", ayahs: 110 },
    { slug: "ya-sin", num: 36, name: "Ya-Sin", arabic: "يس", meaning: "Ya-Sin", ayahs: 83 },
    { slug: "ar-rahman", num: 55, name: "Ar-Rahman", arabic: "الرحمن", meaning: "The Beneficent", ayahs: 78 },
    { slug: "al-mulk", num: 67, name: "Al-Mulk", arabic: "الملك", meaning: "The Sovereignty", ayahs: 30 },
  ] as const;

  const HUE_EDGE = {
    1: "var(--hue-1)",
    2: "var(--hue-2)",
    3: "var(--hue-3)",
    4: "var(--hue-4)",
    5: "var(--hue-5)",
    6: "var(--hue-6)",
    7: "var(--hue-7)",
    8: "var(--hue-8)",
  } as const satisfies Record<LandingHue, string>;
  const HUE_DIM = {
    1: "color-mix(in oklab, var(--hue-1) 30%, transparent)",
    2: "color-mix(in oklab, var(--hue-2) 30%, transparent)",
    3: "color-mix(in oklab, var(--hue-3) 30%, transparent)",
    4: "color-mix(in oklab, var(--hue-4) 30%, transparent)",
    5: "color-mix(in oklab, var(--hue-5) 30%, transparent)",
    6: "color-mix(in oklab, var(--hue-6) 30%, transparent)",
    7: "color-mix(in oklab, var(--hue-7) 30%, transparent)",
    8: "color-mix(in oklab, var(--hue-8) 30%, transparent)",
  } as const satisfies Record<LandingHue, string>;

  function hueAt(position: number): LandingHue {
    // SAFETY: position % 8 is 0–7 for any integer, so +1 is exactly the 1–8 hue-slot union; the assertion only re-narrows the widened number.
    return ((position % 8) + 1) as LandingHue;
  }

  const searchHref = publicHref("/search");
  const juzHref = $derived(publicHref(readerHrefFor(locale, "/juz")));
  const pagesHref = $derived(publicHref(readerHrefFor(locale, "/pages")));
  const yoursHref = publicHref(yoursPageHref());
  const bookmarksHref = publicHref(bookmarksPageHref());
  const settingsHref = publicHref("/settings");
  const loginHref = publicHref("/login");
  const registerHref = publicHref("/register");
</script>

<svelte:head>
  <title>{pageTitle()} · {SITE.name}</title>
  <meta name="robots" content="noindex" />
  <meta name="description" content={`This page does not exist on ${SITE.domain}.`} />
</svelte:head>

<div lang={locale} dir={chrome.direction} data-marketing-root>
  <a
    href="#main"
    class="sr-only focus:not-sr-only focus:absolute focus:start-3 focus:top-3 focus:z-[101] focus:rounded focus:bg-surface focus:px-3 focus:py-2 focus:text-body-s focus:text-foreground focus:shadow-md"
  >{chrome.skipToContent}</a>
  <Nav
    copy={chrome.nav}
    brandCopy={chrome.brand}
    brandHomeHref={marketingHomeHref(locale)}
    {indexLinks}
    direction={chrome.direction}
  />
  <main id="main" tabindex="-1">
    {#if isNotFound}
      <Container
        width="narrow"
        class="relative flex flex-col items-center overflow-hidden pt-12 pb-10 text-center sm:pt-16"
      >
        <p
          aria-hidden="true"
          lang="ar"
          dir="rtl"
          class="font-arabic pointer-events-none absolute -top-6 left-1/2 -translate-x-1/2 text-[180px] leading-none text-foreground/5 select-none sm:text-[240px]"
        >٤٠٤</p>
        <Bismillah class="relative w-40 text-foreground sm:w-48" />
        <p class="relative mt-6 text-caption font-medium tracking-[0.14em] text-muted uppercase">
          404 · {SITE.domain}
        </p>
        <h1
          class="relative mt-3 max-w-[22ch] text-balance text-4xl font-semibold tracking-[-0.025em] text-foreground sm:text-5xl"
        >
          This page wandered off the path
        </h1>
        <p class="relative mt-4 max-w-[52ch] text-body text-foreground-secondary">
          {errorBody()}
        </p>
        <div class="relative mt-7 flex flex-wrap items-center justify-center gap-3">
          <Button href={homeHref} arrow>Back to home</Button>
          <Button href={surahIndexHref} variant="secondary">Browse surahs</Button>
          <Button href={searchHref} variant="ghost">Search the Qur&rsquo;an</Button>
        </div>
        <p class="relative mt-5 text-body-s text-muted">
          Have an account? <a
            class="text-foreground-secondary underline underline-offset-2 hover:text-foreground"
            href={loginHref}>Sign in</a
          >
          <span aria-hidden="true"> · </span> New here? <a
            class="text-foreground-secondary underline underline-offset-2 hover:text-foreground"
            href={registerHref}>Create account</a
          >
        </p>
      </Container>

      <Container class="pb-6">
        <div class="mb-4 flex items-end justify-between gap-4">
          <h2 class="text-xl font-semibold tracking-[-0.01em] text-foreground">Often opened</h2>
          <a
            href={surahIndexHref}
            data-sveltekit-preload-data="hover"
            class="inline-flex flex-none items-center gap-1 text-body-s font-medium text-muted transition-colors hover:text-foreground"
            >All 114 surahs <Icon name="arrow-right" size={15} /></a
          >
        </div>
        <ul class="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {#each POPULAR as s, i (s.num)}
            {@const hue = hueAt(i)}
            <li>
              <a
                href={publicHref(readerHrefFor(locale, surahPathFor(ARABIC_CTX, s.slug)))}
                data-sveltekit-preload-data="hover"
                class="group flex items-center gap-3.5 rounded-md border border-border p-4 transition-colors duration-150 hover:border-border-strong hover:bg-surface-hover focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
              >
                <span
                  class="flex size-10 flex-none items-center justify-center rounded-full border-2 text-[15px] font-medium"
                  style:border-color={HUE_EDGE[hue]}
                  style:background={HUE_DIM[hue]}
                  style:color={HUE_EDGE[hue]}>{s.num}</span
                >
                <span class="flex min-w-0 flex-1 flex-col gap-0.5">
                  <span
                    class="truncate text-[16px] font-medium leading-tight tracking-[-0.01em] text-foreground"
                    >{#if locale === "ar"}{s.arabic}{:else}{s.name}{/if}</span
                  >
                  <span class="flex min-w-0 items-baseline justify-between gap-3">
                    <span class="truncate text-caption leading-none text-muted">{s.meaning}</span>
                    <span
                      lang="ar"
                      dir="rtl"
                      class="shrink-0 font-arabic text-[22px] leading-none text-foreground-secondary transition-colors group-hover:text-foreground"
                      >{s.arabic}</span
                    >
                  </span>
                  <span class="truncate text-caption leading-none text-muted"
                    >{s.ayahs} verses</span
                  >
                </span>
              </a>
            </li>
          {/each}
        </ul>
      </Container>

      {#snippet quickLink(
        href: string,
        icon: "search" | "rows" | "note" | "continuous" | "user" | "plus" | "bookmark",
        label: string,
        hint: string,
      )}
        <a
          {href}
          data-sveltekit-preload-data="hover"
          class="group flex items-center gap-3 rounded-sm px-2 py-2.5 transition-colors hover:bg-surface-hover focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
        >
          <span
            class="flex size-9 flex-none items-center justify-center rounded-pill border border-border text-foreground-secondary transition-colors group-hover:text-foreground"
            ><Icon name={icon} size={16} /></span
          >
          <span class="flex min-w-0 flex-1 flex-col">
            <span class="text-body-s font-medium text-foreground">{label}</span>
            <span class="truncate text-caption text-muted">{hint}</span>
          </span>
          <Icon
            name="arrow-right"
            size={15}
            class="flex-none text-muted transition-transform duration-150 group-hover:translate-x-0.5"
          />
        </a>
      {/snippet}
      <Container class="pb-16 sm:pb-20">
        <div class="grid grid-cols-1 gap-4 md:grid-cols-2">
          <nav
            aria-label="Keep reading"
            class="flex flex-col gap-1 rounded-md border border-border bg-surface p-3"
          >
            <h2 class="eyebrow px-2 pt-1">Keep reading</h2>
            {@render quickLink(searchHref, "search", "Search the Qur'an", "Words, phrases, references")}
            {@render quickLink(juzHref, "rows", "Browse juz", "All 30 parts")}
            {@render quickLink(pagesHref, "note", "Browse pages", "All 604 mushaf pages")}
            {@render quickLink(yoursHref, "continuous", "Your history", "Pick up where you left off")}
          </nav>
          <nav
            aria-label="Account and tools"
            class="flex flex-col gap-1 rounded-md border border-border bg-surface p-3"
          >
            <h2 class="eyebrow px-2 pt-1">Account and tools</h2>
            {@render quickLink(loginHref, "user", "Sign in", "Sync bookmarks across devices")}
            {@render quickLink(registerHref, "plus", "Create account", "Free, in under a minute")}
            {@render quickLink(bookmarksHref, "bookmark", "Bookmarks", "Your saved verses")}
            {@render quickLink(settingsHref, "rows", "Settings", "Reading and appearance")}
          </nav>
        </div>
      </Container>
    {:else}
      <Container
        width="narrow"
        class="flex min-h-[60vh] flex-col items-center justify-center gap-4 py-24 text-center"
      >
        <span class="text-sm text-fg-3">{status}</span>
        <h1 class="text-3xl font-medium tracking-[-0.02em]">Something went wrong</h1>
        <p class="max-w-md text-fg-2">{errorBody()}</p>
        <div class="flex flex-wrap items-center justify-center gap-3">
          <Button onclick={retry} variant="secondary">Try again</Button>
          <Button href={homeHref} arrow>Back to {SITE.name}</Button>
        </div>
      </Container>
    {/if}
  </main>
  <Footer {year} copy={chrome.footer} links={footerLinks} />
</div>

<style>
  /* Same duplicate-skip-link fix as (marketing)/+layout.svelte: the root
       layout's link is a sibling of this [data-marketing-root] div, so the
       sibling form matches it — the old body > a form matched nothing. */
  :global(a[href="#main"]:has(~ [data-marketing-root])) {
    display: none;
  }
</style>
