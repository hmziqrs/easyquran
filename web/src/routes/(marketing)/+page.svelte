<script lang="ts">
  import { page } from "$app/state";
  import { Band, Button, Icon, MetricCard } from "$lib/components";
  import { resolveLandingCopy } from "$lib/i18n/landing-copy";
  import { marketingLocaleFromPath, marketingReaderHomeHref } from "$lib/i18n/marketing-copy";
  import { marketingHref } from "$lib/i18n/marketing";
  import MarketingSeo from "./_components/MarketingSeo.svelte";
  import { surahPathFor } from "$lib/data/quran";
  import { readerHrefFor } from "$lib/i18n/reader";
  import { publicHref } from "$lib/i18n/public-href";
  import { authState } from "$lib/auth/auth-state.svelte";
  import type { BookmarksStore } from "$lib/bookmarks/store.svelte";
  import type { ReaderApi } from "$lib/stores/reader.svelte";
  import type { LandingHue } from "$lib/i18n/marketing-copy";

  /**
   * Landing rebuilt to the boards (plan 05): eight full-bleed bands — header and
   * footer live in the marketing layout, this page owns hero / metric strip /
   * index / why / roadmap / closing. The hero pill search is the primary action:
   * a real form submitting to /app/search; ⌘K stays owned by GlobalSearch (the
   * shared TanStack registry), this page adds only an Escape-clear chord for the
   * field itself through the same registerHotkey path.
   */
  let { data } = $props();

  const arabicCtx = { kind: "arabic" } as const;
  const locale = $derived(marketingLocaleFromPath(page.url.pathname));
  const landing = $derived(resolveLandingCopy(locale));
  const aboutHref = $derived(marketingHref("about", locale));

  /* Hue slots resolve through the palette tokens (§61 — no colour literals).
     Boards cycle the four hues by position, never by surah number. */
  const HUE_SOFT = {
    1: "var(--hue-1-soft)",
    2: "var(--hue-2-soft)",
    3: "var(--hue-3-soft)",
    4: "var(--hue-4-soft)",
  } as const satisfies Record<LandingHue, string>;
  const HUE_LEGIBLE = {
    1: "var(--hue-1-legible)",
    2: "var(--hue-2-legible)",
    3: "var(--hue-3-legible)",
    4: "var(--hue-4-legible)",
  } as const satisfies Record<LandingHue, string>;
  const HUE_FILL = {
    1: "var(--hue-1)",
    2: "var(--hue-2)",
    3: "var(--hue-3)",
    4: "var(--hue-4)",
  } as const satisfies Record<LandingHue, string>;
  const ON_HUE = {
    1: "var(--on-hue-1)",
    2: "var(--on-hue-2)",
    3: "var(--on-hue-3)",
    4: "var(--on-hue-4)",
  } as const satisfies Record<LandingHue, string>;

  function hueAt(position: number): LandingHue {
    // SAFETY: position % 4 is 0–3 for any integer, so +1 is exactly the 1–4 hue-slot union; the assertion only re-narrows the widened number.
    return ((position % 4) + 1) as LandingHue;
  }

  /* Boards' "Often opened" row — the five surahs readers open most. */
  const OFTEN_OPENED = [1, 18, 36, 55, 67];
  let oftenOpened = $derived(
    OFTEN_OPENED.map((num) => data.surahs.find((s) => s.num === num)).filter(
      (s): s is (typeof data.surahs)[number] => s !== undefined,
    ),
  );

  /* ── Hero search ─────────────────────────────────────────────────────────── */
  const SEARCH_ACTION = publicHref("/app/search");
  let query = $state("");
  let searchInput = $state<HTMLInputElement | undefined>();

  // Escape clears the field (then blurs it) while it holds focus. Registered
  // through the shared TanStack wrapper, dynamically imported so the hotkeys
  // chunk stays out of the initial bundle; IME composition is never hijacked.
  $effect(() => {
    let destroyed = false;
    let cleanup: (() => void) | undefined;

    void import("$lib/hotkeys.svelte").then(({ registerHotkey }) => {
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

  /* ── Bookmarks metric (real data, never fabricated samples) ──────────────── */
  let bookmarksStore = $state<BookmarksStore | null>(null);
  let readerStore = $state<ReaderApi | null>(null);

  // Client-only: both stores arrive behind a dynamic import (the sync engine and
  // reader persistence never belong in the landing's initial bundle), hydrate,
  // then feed the derived count below.
  $effect(() => {
    let dead = false;
    void Promise.all([
      import("$lib/stores/reader.svelte"),
      import("$lib/bookmarks/store.svelte"),
    ]).then(([readerModule, bookmarksModule]) => {
      if (dead) return;
      readerModule.reader.hydrate();
      bookmarksModule.bookmarks.hydrate();
      readerStore = readerModule.reader;
      bookmarksStore = bookmarksModule.bookmarks;
    });
    return () => {
      dead = true;
    };
  });

  // An authed reader's server view needs the auth edge to start syncing — the
  // marketing page has no app layout to drive onAuthChanged.
  $effect(() => {
    if (authState.authenticated && bookmarksStore) bookmarksStore.onAuthChanged(true);
  });

  let bookmarkCount = $derived.by(() => {
    if (bookmarksStore?.authed) return bookmarksStore.bookmarks.length;
    return readerStore?.bookmarkedKeys.length ?? 0;
  });
  let bookmarkCountKnown = $derived(bookmarksStore !== null && readerStore !== null);

  function bookmarkValue(): string {
    if (bookmarkCount > 0) return String(bookmarkCount);
    return landing.metricYours;
  }
  function bookmarkCaption(): string {
    if (bookmarkCountKnown && bookmarkCount === 0) return landing.metricBookmarksEmpty;
    return landing.metricBookmarksNote;
  }
  let bookmarksValue = $derived(bookmarkValue());
  let bookmarksCaption = $derived(bookmarkCaption());
</script>

<MarketingSeo {locale} />

<!-- ── band 2: hero ─────────────────────────────────────────────────────── -->
<Band
  width="wide"
  class="py-16 md:py-[72px] xl:py-[88px]"
  contentClass="flex flex-col items-center"
>
  <h1
    aria-label={landing.heroTitleFull}
    class="max-w-[17ch] text-balance text-center text-display-xl"
  >
    {landing.heroTitleLead}<span
      class="mx-2 inline-block rounded-highlight bg-primary px-5 pb-2 pt-[2px] text-primary-foreground"
      >{landing.heroTitleHighlight}</span
    >{landing.heroTitleTail}
  </h1>
  <p
    class="mt-[26px] max-w-[55ch] text-pretty text-center text-body-xl text-foreground-secondary"
  >
    {landing.heroIntro}
  </p>

  <form
    method="GET"
    action={SEARCH_ACTION}
    role="search"
    class="mt-10 flex h-[64px] w-full max-w-[820px] items-center gap-4 rounded-pill border border-border bg-surface ps-[22px] pe-2 sm:h-[68px] sm:ps-[30px] xl:h-[76px]"
  >
    <Icon name="search" size={23} class="flex-none text-muted" />
    <label class="sr-only" for="hero-search">{landing.searchLabel}</label>
    <input
      id="hero-search"
      bind:this={searchInput}
      bind:value={query}
      type="search"
      name="q"
      placeholder={landing.searchPlaceholder}
      autocomplete="off"
      class="min-w-0 flex-grow bg-transparent text-[17px] font-semibold text-foreground outline-none placeholder:text-muted sm:text-[19px]"
    />
    <Button
      type="submit"
      variant="primary"
      size="lg"
      class="h-12 flex-none px-6 text-[16px] font-extrabold sm:h-14 sm:px-[34px] sm:text-[18px] xl:h-[60px]"
    >{landing.searchButton}</Button>
  </form>

  <div class="mt-[22px] flex flex-wrap items-center justify-center gap-2">
    <span class="me-1.5 text-body text-muted">{landing.oftenOpened}</span>
    {#each oftenOpened as s (s.num)}
      <a
        href={publicHref(readerHrefFor(locale, surahPathFor(arabicCtx, s)))}
        class="rounded-pill border border-border bg-surface px-4 py-[9px] text-[14.5px] font-bold text-foreground-secondary transition-colors duration-150 hover:border-border-strong hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
      >{#if locale === "ar"}<span lang="ar" dir="rtl" class="font-arabic">{s.arabic}</span>{:else}{s.name}{/if}</a
      >
    {/each}
  </div>
</Band>

<!-- ── band 3: metric strip — gapless, edge to edge, four hues ──────────── -->
<Band pad="none" width="full" contentClass="px-0 md:px-0 lg:px-0 xl:px-0">
  <div class="grid grid-cols-1 gap-0 md:grid-cols-2 lg:grid-cols-4">
    <MetricCard hue={1} value={String(data.surahCount)} label={landing.metricSurahs} caption={landing.metricSurahsNote}>
      <Icon name="book" />
    </MetricCard>
    <MetricCard hue={2} value={String(data.juzCount)} label={landing.metricJuz} caption={landing.metricJuzNote}>
      <Icon name="continuous" />
    </MetricCard>
    <MetricCard hue={3} value={String(data.pageCount)} label={landing.metricPages} caption={landing.metricPagesNote}>
      <Icon name="note" />
    </MetricCard>
    <MetricCard hue={4} value={bookmarksValue} label={landing.metricBookmarks} caption={bookmarksCaption}>
      <Icon name="bookmark" />
    </MetricCard>
  </div>
</Band>

<!-- ── band 4: index ────────────────────────────────────────────────────── -->
<Band id="surahs" width="wide" class="scroll-mt-20">
  <div class="mb-[34px] flex flex-wrap items-end justify-between gap-6">
    <div class="flex flex-col gap-3">
      <span class="text-micro text-primary">{landing.indexEyebrow}</span>
      <h2 class="max-w-[22ch] text-h1">{landing.indexTitle}</h2>
      <p class="max-w-[58ch] text-pretty text-body-l text-foreground-secondary">
        {landing.indexIntro}
      </p>
    </div>
    <a
      href="#surahs"
      class="flex h-12 flex-none items-center gap-[9px] rounded-pill border border-border px-[22px] text-[15.5px] font-extrabold text-foreground transition-colors duration-150 hover:border-border-strong hover:bg-surface-hover focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
    >
      {landing.indexSeeAll}
      <Icon
        name="arrow-right"
        size={17}
        class={locale === "ar" ? "rotate-180" : ""}
      />
    </a>
  </div>
  <ul class="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
    {#each data.surahs as s, i (s.num)}
      {@const hue = hueAt(i)}
      <li>
        <a
          href={publicHref(readerHrefFor(locale, surahPathFor(arabicCtx, s)))}
          class="group flex items-center gap-[15px] rounded-md border border-border bg-surface px-[18px] py-[15px] transition-colors duration-150 hover:border-border-strong hover:bg-surface-hover focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
        >
          <span
            class="flex h-9 min-w-11 flex-none items-center justify-center rounded-pill px-2.5 text-[15px] font-extrabold tabular-nums"
            style:background={HUE_SOFT[hue]}
            style:color={HUE_LEGIBLE[hue]}
            >{s.num}</span
          >
          <span class="flex min-w-0 flex-col gap-[2px]">
            <span class="truncate text-[16.5px] font-extrabold tracking-[-0.02em] text-foreground"
              >{s.name}</span
            >
            <span class="truncate text-caption text-muted">
              {s.meaning} · {s.ayahCount} ayahs
            </span>
          </span>
          <span
            dir="rtl"
            class="ms-auto shrink-0 font-arabic text-[24px] leading-[1.6] text-foreground-secondary transition-colors group-hover:text-primary"
            >{s.arabic}</span
          >
        </a>
      </li>
    {/each}
  </ul>
</Band>

<!-- ── band 5: why — the band that used to be a card ────────────────────── -->
<Band id="why" tone="panel" width="wide" class="scroll-mt-20">
  <div class="grid grid-cols-1 items-start gap-10 lg:grid-cols-2 lg:gap-20">
    <div class="flex flex-col gap-5">
      <span class="text-micro text-primary">{landing.whyEyebrow}</span>
      <h2 class="max-w-[18ch] text-h1">{landing.whyTitle}</h2>
      <p class="max-w-[52ch] text-pretty text-body-l text-foreground-secondary">
        {landing.whyIntro}
      </p>
      {#if aboutHref}
        <Button
          variant="primary"
          size="lg"
          href={publicHref(aboutHref)}
          class="mt-1.5 self-start text-[16.5px] font-extrabold"
        >{landing.secondaryCta}</Button>
      {/if}
    </div>
    <div class="flex flex-col">
      {#each landing.steps as step, i (step.id)}
        <div
          class="flex items-start gap-5 py-[26px] {i > 0 ? 'border-t border-border' : ''}"
        >
          <span
            class="flex size-10 flex-none items-center justify-center rounded-sm text-[16px] font-extrabold"
            style:background={HUE_FILL[step.hue]}
            style:color={ON_HUE[step.hue]}
            >{i + 1}</span
          >
          <span class="flex flex-col gap-[5px]">
            <span class="text-h3">{step.title}</span>
            <span class="text-body text-muted">{step.body}</span>
          </span>
        </div>
      {/each}
    </div>
  </div>
</Band>

<!-- ── band 6: roadmap — columns divided by rules, not cards ────────────── -->
<Band id="roadmap" width="wide" class="scroll-mt-20">
  <div class="mb-10 flex flex-col gap-3">
    <span class="text-micro text-primary">{landing.roadmapEyebrow}</span>
    <h2 class="text-h1">{landing.roadmapTitle}</h2>
    <p class="max-w-[60ch] text-pretty text-body-l text-foreground-secondary">
      {landing.roadmapIntro}
    </p>
  </div>
  <div class="grid grid-cols-1 gap-0 border-t border-border sm:grid-cols-2 lg:grid-cols-4">
    {#each landing.roadmap as item, i (item.id)}
      <div
        class="flex flex-col gap-3.5 px-0 pb-2 pt-8 {i > 0 ? 'lg:border-s lg:border-border lg:ps-8' : ''}"
      >
        <span
          class="flex size-10 items-center justify-center rounded-sm text-[15px] font-extrabold"
          style:background={HUE_SOFT[item.hue]}
          style:color={HUE_LEGIBLE[item.hue]}
          >{String(i + 1).padStart(2, "0")}</span
        >
        <span class="text-h3">{item.title}</span>
        <p class="text-body text-muted">{item.body}</p>
      </div>
    {/each}
  </div>
</Band>

<!-- ── band 7: closing — full-bleed colour ──────────────────────────────── -->
<Band tone="accent" width="wide" class="py-16 md:py-[72px] xl:py-[84px]">
  <div class="flex flex-col items-center gap-[22px]">
    <span
      lang="ar"
      dir="rtl"
      class="font-arabic text-[36px] leading-[1.9]">{landing.closingBismillah}</span
    >
    <h2 class="max-w-[24ch] text-balance text-center text-h1">{landing.closingTitle}</h2>
    <p class="max-w-[52ch] text-pretty text-center text-body-l opacity-85">
      {landing.closingIntro}
    </p>
    <Button
      variant="ink"
      size="lg"
      href={publicHref(marketingReaderHomeHref(locale))}
      class="mt-2.5 gap-2.5 text-[17.5px] font-extrabold"
    >
      {landing.closingCta}
      <Icon
        name="arrow-right"
        size={19}
        class={locale === "ar" ? "rotate-180" : ""}
      />
    </Button>
    <span class="text-body opacity-80">{landing.closingNote}</span>
  </div>
</Band>
