<script lang="ts">
  import { page } from "$app/state";
  import { Button, Container, Icon } from "#lib/components/index.js";
  import { resolveLandingCopy } from "#lib/i18n/landing-copy.js";
  import { marketingLocaleFromPath } from "#lib/i18n/marketing-copy.js";
  import MarketingSeo from "./_components/MarketingSeo.svelte";
  import { surahPathFor } from "#lib/data/quran.js";
  import { readerHrefFor, yoursPageHref } from "#lib/i18n/reader.js";
  import { publicHref } from "#lib/i18n/public-href.js";
  import type { LandingHue } from "#lib/i18n/marketing-copy.js";

  /**
   * The home page is the index, not a pitch: bismillah, one search field, a few
   * shortcuts and every surah. It shares the header's frame (Container: 1200px,
   * 24px gutter) so the logo, the search and the first card line up. The search
   * is a real form submitting to /search; ⌘K stays owned by GlobalSearch (the
   * shared TanStack registry), this page adds only an Escape-clear chord for the
   * field itself through the same registerHotkey path.
   */
  let { data } = $props();

  const arabicCtx = { kind: "arabic" } as const;
  const locale = $derived(marketingLocaleFromPath(page.url.pathname));
  const landing = $derived(resolveLandingCopy(locale));
  // The sibling indexes. The surah index is this page, so it is the current tab.
  const indexLinks = $derived([
    { label: landing.indexJuz, href: publicHref(readerHrefFor(locale, "/juz")) },
    { label: landing.indexPages, href: publicHref(readerHrefFor(locale, "/pages")) },
    { label: landing.indexYours, href: publicHref(yoursPageHref()) },
  ]);

  /* Hue slots resolve through the palette tokens (§61 — no colour literals),
     cycled by position like the app's surah index. */
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

  /* The five surahs readers open most. */
  const OFTEN_OPENED = [1, 18, 36, 55, 67];

  // Route data can lack `surahs` for one render (dev hot swap keeps the old load
  // output; the SW's stale-while-revalidate __data cache can serve a pre-rebuild
  // payload). Degrade to an empty index instead of crashing hydration.
  let surahs = $derived(data.surahs ?? []);
  let oftenOpened = $derived(
    OFTEN_OPENED.map((num) => surahs.find((s) => s.num === num)).filter(
      (s): s is (typeof surahs)[number] => s !== undefined,
    ),
  );

  /* ── Search ──────────────────────────────────────────────────────────────── */
  const SEARCH_ACTION = publicHref("/search");
  let query = $state("");
  let searchInput = $state<HTMLInputElement | undefined>();

  // Escape clears the field (then blurs it) while it holds focus. Registered
  // through the shared TanStack wrapper, dynamically imported so the hotkeys
  // chunk stays out of the initial bundle; IME composition is never hijacked.
  $effect(() => {
    let destroyed = false;
    let cleanup: (() => void) | undefined;

    void import("#lib/hotkeys.svelte.js").then(({ registerHotkey }) => {
      if (destroyed) return;
      const escape = registerHotkey(
        "Escape",
        (event) => {
          if (event.isComposing) return;
          const el = searchInput;
          if (!el || document.activeElement !== el) return;
          if (el.value.length > 0) {
            query = "";
            return;
          }
          el.blur();
        },
        { meta: { name: "Clear landing search field" } },
      );
      cleanup = () => {
        escape.unregister();
      };
    });

    return () => {
      destroyed = true;
      cleanup?.();
    };
  });
</script>

<MarketingSeo {locale} />

<Container class="flex flex-col items-center pt-10 pb-10 sm:pt-14 md:pt-16">
  <h1 class="sr-only">{landing.heroTitleFull}</h1>
  <p lang="ar" dir="rtl" class="font-arabic text-[26px] leading-[1.9] text-foreground sm:text-[32px]">
    {landing.bismillah}
  </p>

  <form
    method="GET"
    action={SEARCH_ACTION}
    role="search"
    class="mt-6 flex h-14 w-full max-w-[720px] items-center gap-3 rounded-pill border border-border bg-surface ps-5 pe-1.5 transition-colors focus-within:border-border-strong sm:h-16 sm:ps-6 sm:pe-2"
  >
    <Icon name="search" size={20} class="flex-none text-muted" />
    <label class="sr-only" for="hero-search">{landing.searchLabel}</label>
    <input
      id="hero-search"
      bind:this={searchInput}
      bind:value={query}
      type="search"
      name="q"
      placeholder={landing.searchPlaceholder}
      autocomplete="off"
      class="min-w-0 flex-grow bg-transparent text-[16px] font-medium text-foreground outline-none placeholder:text-muted sm:text-[17px] [&::-webkit-search-cancel-button]:hidden"
    />
    <Button
      type="submit"
      variant="primary"
      class="h-11 flex-none px-5 text-[15px] font-bold sm:h-12 sm:px-6"
    >{landing.searchButton}</Button>
  </form>

  <div class="mt-4 flex flex-wrap items-center justify-center gap-2">
    <span class="me-1 text-caption text-muted">{landing.oftenOpened}</span>
    {#each oftenOpened as s (s.num)}
      <a
        href={publicHref(readerHrefFor(locale, surahPathFor(arabicCtx, s)))}
        class="rounded-pill border border-border px-3.5 py-1.5 text-[14px] font-semibold text-foreground-secondary transition-colors duration-150 hover:border-border-strong hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
      >{#if locale === "ar"}<span lang="ar" dir="rtl" class="font-arabic">{s.arabic}</span>{:else}{s.name}{/if}</a
      >
    {/each}
  </div>
</Container>

<Container id="surahs" class="scroll-mt-20 pb-16">
  <!-- Tab row: this page is the surah index, so "Surahs" is the current tab and
       the sibling indexes are plain links — the only way to Juz/Pages on phones,
       where the header hides its index links. -->
  <div
    class="mb-4 flex items-end gap-1 overflow-x-auto overflow-y-hidden border-b border-border [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
  >
    <h2
      class="-mb-px inline-flex h-11 flex-none items-center border-b-2 border-foreground px-3 text-[15px] font-bold text-foreground"
    >{landing.indexSurahs}</h2>
    <nav aria-label={landing.indexLabel} class="flex flex-none items-end gap-1">
      {#each indexLinks as link (link.href)}
        <a
          href={link.href}
          data-sveltekit-preload-data="hover"
          class="-mb-px inline-flex h-11 items-center border-b-2 border-transparent px-3 text-[15px] font-semibold text-muted transition-colors hover:text-foreground focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-focus-ring"
        >{link.label}</a>
      {/each}
    </nav>
    <span class="ms-auto flex h-11 flex-none items-center px-1 text-caption tabular-nums text-muted"
      >{surahs.length}</span
    >
  </div>

  <ul class="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
    {#each surahs as s, i (s.num)}
      {@const hue = hueAt(i)}
      <li>
        <a
          href={publicHref(readerHrefFor(locale, surahPathFor(arabicCtx, s)))}
          class="group flex items-center gap-3.5 rounded-md border border-border p-4 transition-colors duration-150 hover:border-border-strong hover:bg-surface-hover focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
        >
          <span
            class="flex size-10 flex-none items-center justify-center rounded-full border-2 text-[15px] font-extrabold tabular-nums"
            style:border-color={HUE_EDGE[hue]}
            style:background={HUE_DIM[hue]}
            style:color={HUE_EDGE[hue]}
            >{s.num}</span
          >
          <span class="flex min-w-0 flex-1 flex-col gap-0.5">
            <span class="truncate text-[16px] font-bold leading-tight tracking-[-0.01em] text-foreground"
              >{s.name}</span
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
            <span class="truncate text-caption leading-none tabular-nums text-muted"
              >{landing.ayahCount(s.ayahCount)}</span
            >
          </span>
        </a>
      </li>
    {/each}
  </ul>
</Container>
