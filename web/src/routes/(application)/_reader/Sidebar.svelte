<script lang="ts">
  import { page } from "$app/state";
  import { BrowseMode, reader } from "#lib/stores/reader.svelte.js";
  import {
    globalPagePathFor,
    juzPathFor,
    routeContextFromParams,
    surahAyahPathFor,
    surahMeta,
    surahPathFor,
    parseKey,
    type SurahRouteContext,
  } from "#lib/data/quran.js";
  import { loadQuranData } from "#lib/data/quran-data-client.js";
  import { RangeKind, type QuranData } from "#lib/data/quran-data.js";
  import { hizbRange, juzOfPage } from "#lib/data/mushaf-divisions.js";
  import type { CatalogEntry, RangeEntry } from "#lib/data/quran-types.js";
  import { Icon } from "#lib/components/icon/index.js";
  import { Input } from "#lib/components/ui/input/index.js";
  import { getReaderUiCopy } from "#lib/i18n/reader-copy.js";
  import { readerHrefFor } from "#lib/i18n/reader.js";
  import { publicHref } from "#lib/i18n/public-href.js";
  import { cn } from "#lib/utils.js";
  import type { Snippet } from "svelte";
  import {
    Sidebar,
    SidebarHeader,
    SidebarContent,
    SidebarGroup,
    SidebarGroupContent,
    SidebarMenuButton,
    useSidebar,
  } from "#lib/components/ui/sidebar/index.js";
  import SidebarVirtualList from "./SidebarVirtualList.svelte";
  import { ayahTabSurah } from "./ayah-tab";

  const BROWSE = [BrowseMode.Surah, BrowseMode.Ayah, BrowseMode.Juz, BrowseMode.Page] as const;
  // Active rows take a soft primary fill instead of the grey sidebar accent.
  const ROW =
    "h-auto items-center gap-2 px-3 py-2.5 data-[active=true]:bg-primary/15 data-[active=true]:hover:bg-primary/15";
  const copy = getReaderUiCopy();
  const sidebar = useSidebar();
  const dataPromise = $derived(sidebar.openMobile ? loadQuranData() : null);

  let contentEl: HTMLDivElement | null = $state(null);

  function oninput(e: Event) {
    // SAFETY: oninput is bound only to the search Input's oninput below; currentTarget is that input element.
    reader.setQuery((e.currentTarget as HTMLInputElement).value);
  }
  function onItemClick() {
    reader.clearQuery();
    sidebar.setOpenMobile(false);
  }

  function selectBrowse(browse: BrowseMode) {
    reader.setBrowse(browse);
    if (browse !== BrowseMode.Ayah) return;
    void loadQuranData().then((quranData) => {
      const current = ayahTabSurah(quranData, page.params, reader.current);
      void reader.refreshFromWorker(current.num);
    });
  }

  function browseLabel(browse: BrowseMode): string {
    switch (browse) {
      case BrowseMode.Surah:
        return copy.sidebar.mode(browse);
      case BrowseMode.Ayah:
        return copy.sidebar.mode(browse);
      case BrowseMode.Juz:
        return copy.sidebar.mode(browse);
      case BrowseMode.Page:
        return copy.sidebar.mode(browse);
    }
  }

  const routeCtx = $derived<SurahRouteContext>(routeContextFromParams(page.params));

  function surahHref(slug: string): `/${string}` {
    return readerHrefFor(copy.locale, surahPathFor(routeCtx, slug));
  }

  function rangeHref(useJuz: boolean, index: number): `/${string}` {
    const quranHref = useJuz ? juzPathFor(routeCtx, index) : globalPagePathFor(routeCtx, index);
    return readerHrefFor(copy.locale, quranHref);
  }

  const isJuzRoute = $derived((page.route.id ?? "").includes("/juz/"));
  const isHizbRoute = $derived((page.route.id ?? "").includes("/hizb/"));

  function toIndex(v: string | undefined): number | null {
    const n = v ? Number(v) : Number.NaN;
    return Number.isSafeInteger(n) && n > 0 ? n : null;
  }

  function clampAyah(surah: CatalogEntry, n: number): number {
    return Number.isSafeInteger(n) && n >= 1 && n <= surah.ayahCount ? n : 1;
  }

  /**
   * Ayah of `surah` the current route or reader position points at — hash
   * anchor, shared ?v= position, or the sticky bar's live mushaf page. Used to
   * reveal the matching row; deep links always name something now.
   */
  function routeAyah(quranData: QuranData, surah: CatalogEntry): number {
    const hash = new RegExp(`^#ayah-${surah.num}-(\\d+)$`).exec(page.url.hash)?.[1];
    if (hash !== undefined) return clampAyah(surah, Number(hash));
    const shared = /^(\d+):(\d+)$/.exec(page.url.searchParams.get("v") ?? "");
    if (shared && Number(shared[1]) === surah.num) return clampAyah(surah, Number(shared[2]));
    const position = reader.position;
    if (position) {
      const start = quranData.rangeByIndex(RangeKind.Page, position.globalPage)?.startGlobal;
      const surahEnd = surah.startGlobal + surah.ayahCount - 1;
      if (start !== undefined && start >= surah.startGlobal && start <= surahEnd) {
        return start - surah.startGlobal + 1;
      }
    }
    return 1;
  }

  /** Global ayah the current route points at, used to reveal the matching row. */
  function currentGlobal(quranData: QuranData): number | null {
    const slug = page.params.surah;
    if (slug) {
      const s = quranData.surahBySlug(slug);
      if (!s) return null;
      return quranData.globalIndexOf(s.num, routeAyah(quranData, s)) ?? s.startGlobal;
    }
    const n = toIndex(page.params.n);
    if (n === null) return null;
    if (isHizbRoute) return hizbRange(quranData, n)?.startGlobal ?? null;
    const kind = isJuzRoute ? RangeKind.Juz : RangeKind.Page;
    return quranData.rangeByIndex(kind, n)?.startGlobal ?? null;
  }

  // Rows list (user pick, /design/mix lists=b): a juz names where it ends and how many
  // pages it spans; a page names where it ends and its juz. Memoised per data instance.
  const juzPageCounts = new WeakMap<QuranData, Map<number, number>>();
  function pagesInJuz(quranData: QuranData, juz: RangeEntry): number {
    let counts = juzPageCounts.get(quranData);
    if (!counts) {
      counts = new Map();
      const pageRanges = quranData.ranges(RangeKind.Page);
      for (const j of quranData.ranges(RangeKind.Juz)) {
        counts.set(
          j.index,
          pageRanges.filter((p) => p.startGlobal >= j.startGlobal && p.startGlobal <= j.endGlobal).length,
        );
      }
      juzPageCounts.set(quranData, counts);
    }
    return counts.get(juz.index) ?? 0;
  }

  function surahNameOf(quranData: QuranData, num: number): string {
    return quranData.surahByNum(num)?.name ?? `${copy.sidebar.mode("surah")} ${num}`;
  }

  /** "to Al-Kahf 18:74", or just "to 67:12" when the range ends in the surah it starts in. */
  function rangeEnd(quranData: QuranData, rg: RangeEntry): string {
    const start = parseKey(rg.first);
    const end = parseKey(rg.last);
    if (start.num === end.num) return `→ ${rg.last}`;
    return `→ ${surahNameOf(quranData, end.num)} ${rg.last}`;
  }

  function rangeRow(ranges: readonly RangeEntry[], global: number | null): number {
    if (global === null) return -1;
    return ranges.findIndex((r) => global >= r.startGlobal && global <= r.endGlobal);
  }
</script>


{#snippet navRow(
  href: string,
  isActive: boolean | undefined,
  ariaCurrent: "page" | undefined,
  cls: string,
  body: Snippet,
)}
  <!-- A div, not SidebarMenuItem's <li>: SidebarVirtualList's row is already the list
       item, and an <li> outside a <ul> rendered the browser's default bullet. -->
  <div data-sidebar="menu-item" class="group/menu-item relative">
    <!-- bg-transparent: rows must rest on the sidebar ground. The button's hover/
         active affordance is --sidebar-accent (= surface-hover); with no explicit
         resting background the row already wore that exact value, so hover and
         focus were invisible — the feedback area looked smaller than the row. -->
    <SidebarMenuButton isActive={isActive} aria-current={ariaCurrent} class={cn(cls, "bg-transparent")}>
      {#snippet child({ props })}
        <a {...props} {href} data-sveltekit-preload-data="hover" onclick={onItemClick}>
          {@render body()}
        </a>
      {/snippet}
    </SidebarMenuButton>
  </div>
{/snippet}

<Sidebar collapsible="offcanvas">
  <SidebarHeader>
    <!-- shadcn icon-input pattern: the Input keeps its own border/background/focus ring;
         the search glyph and clear button overlay it, so the 44px field stays the hit target. -->
    <div class="relative">
      <Icon
        name="search"
        size={15}
        class="pointer-events-none absolute start-4 top-1/2 -translate-y-1/2 text-muted-foreground"
      />
      <Input
        type="text"
        value={reader.query}
        {oninput}
        placeholder={copy.sidebar.searchPlaceholder}
        aria-label={copy.sidebar.searchLabel}
        class="ps-11 pe-11"
      />
      {#if reader.hasQuery}
        <button
          type="button"
          onclick={() => reader.clearQuery()}
          aria-label={copy.sidebar.clearSearch}
          class="absolute end-3 top-1/2 flex size-6 -translate-y-1/2 items-center justify-center text-muted-foreground transition-colors hover:text-foreground"
        >
          <Icon name="x" size={15} />
        </button>
      {/if}
    </div>

    <div
      class="grid grid-cols-4 gap-1 rounded-md bg-background-subtle p-1"
      role="group"
      aria-label={copy.sidebar.browseLabel}
    >
      {#each BROWSE as b (b)}
        <button
          type="button"
          aria-pressed={reader.browseMode === b}
          onclick={() => selectBrowse(b)}
          class={cn(
            "rounded-sm py-2 text-[12.5px] font-medium capitalize transition-colors",
            reader.browseMode === b
              ? "bg-primary text-primary-foreground shadow-sm hover:bg-primary hover:text-primary-foreground"
              : "text-muted-foreground hover:text-foreground-secondary",
          )}
        >
          {browseLabel(b)}
        </button>
      {/each}
    </div>
  </SidebarHeader>

  <SidebarContent bind:ref={contentEl}>
    {#if dataPromise}
      {#await dataPromise}
        <p class="px-4 py-3 text-sm text-muted-foreground" aria-live="polite">
          {copy.sidebar.loadingNavigation}
        </p>
      {:then quranData}
        {@const current = ayahTabSurah(quranData, page.params, reader.current)}
        {#if reader.browseSurah}
          <SidebarGroup>
            <SidebarGroupContent>
              <SidebarVirtualList
                getScrollElement={() => contentEl}
                count={quranData.surahs.length}
                estimateSize={56}
                activeIndex={quranData.surahs.findIndex((s) => s.slug === page.params.surah)}
              >
                {#snippet item(i)}
                  {@const s = quranData.surahs[i]!}
                  {@const active = page.params.surah === s.slug}
                  {#snippet body()}
                    <span
                      class={[
                        "w-9 flex-none text-[15px] font-medium tabular-nums",
                        active ? "text-primary" : "text-foreground",
                      ]}
                    >
                      {s.num}
                    </span>
                    <span class="flex min-w-0 flex-1 flex-col gap-0.5">
                      <span class="truncate text-[14.5px] font-medium text-foreground">{s.name}</span>
                      <span class="truncate text-[12.5px] text-foreground-secondary">
                        {s.meaning} · {surahMeta(s)}
                      </span>
                    </span>
                    <span dir="rtl" lang="ar" class="flex-none font-quran text-[19px] leading-none text-foreground">
                      {s.arabic}
                    </span>
                  {/snippet}
                  {@render navRow(
                    publicHref(surahHref(s.slug)),
                    active,
                    active ? "page" : undefined,
                    ROW,
                    body,
                  )}
                {/snippet}
              </SidebarVirtualList>
            </SidebarGroupContent>
          </SidebarGroup>
        {:else if reader.browseAyah}
          {@const cur = current}
          {@const verses = reader.versesFor(cur.num)}
          {@const routeVerse = routeAyah(quranData, cur)}
          {#key verses.length}
          <SidebarGroup>
            <SidebarGroupContent>
              <SidebarVirtualList
                getScrollElement={() => contentEl}
                count={verses.length}
                estimateSize={44}
                activeIndex={routeVerse - 1}
              >
                {#snippet item(i)}
                  {@const n = i + 1}
                  {@const v = verses[i]}
                  {#if v}
                    {#snippet body()}
                      <span
                        class="flex h-6 w-6 flex-none items-center justify-center rounded-pill border border-border text-[11px] text-muted-foreground"
                      >
                        {n}
                      </span>
                      <span dir="rtl" class="min-w-0 flex-1 truncate font-arabic text-[15px]">
                        {v}
                      </span>
                    {/snippet}
                    {@render navRow(
                      publicHref(
                        readerHrefFor(copy.locale, surahAyahPathFor(routeCtx, cur, n)),
                      ),
                      n === routeVerse ? true : undefined,
                      n === routeVerse ? "page" : undefined,
                      "h-auto gap-3 px-3.5 py-2.5",
                      body,
                    )}
                  {/if}
                {/snippet}
              </SidebarVirtualList>
            </SidebarGroupContent>
          </SidebarGroup>
          {/key}
        {:else}
          {@const ranges = quranData.ranges(
            reader.browseJuz ? RangeKind.Juz : RangeKind.Page,
          )}
          {@const activeRange = rangeRow(ranges, currentGlobal(quranData))}
          {#key reader.browseJuz ? "juz" : "page"}
          <SidebarGroup>
            <SidebarGroupContent>
              <SidebarVirtualList
                getScrollElement={() => contentEl}
                count={ranges.length}
                estimateSize={44}
                activeIndex={activeRange}
              >
                {#snippet item(i)}
                  {@const rg = ranges[i]!}
                  {@const { num } = parseKey(rg.first)}
                  {@const href = publicHref(rangeHref(reader.browseJuz, rg.index))}
                  {@const active = i === activeRange}
                  {#snippet body()}
                    <span
                      class={[
                        "w-11 flex-none text-[19px] font-medium tabular-nums",
                        active ? "text-primary" : "text-foreground",
                      ]}
                    >
                      <span class="sr-only">{copy.sidebar.rangeItem(reader.browseJuz ? "juz" : "page", rg.index)}</span>
                      <span aria-hidden="true">{rg.index}</span>
                    </span>
                    <span class="flex min-w-0 flex-1 flex-col gap-0.5">
                      <span class="truncate text-[14.5px] font-medium text-foreground">
                        {surahNameOf(quranData, num)} {rg.first}
                      </span>
                      <span class="truncate text-[12.5px] text-foreground-secondary">
                        {rangeEnd(quranData, rg)} ·
                        {#if reader.browseJuz}
                          {copy.index.pageCount(pagesInJuz(quranData, rg))}
                        {:else}
                          {copy.sidebar.rangeItem("juz", juzOfPage(quranData, rg.startGlobal))}
                        {/if}
                      </span>
                    </span>
                  {/snippet}
                  {@render navRow(href, active, active ? "page" : undefined, ROW, body)}
                {/snippet}
              </SidebarVirtualList>
            </SidebarGroupContent>
          </SidebarGroup>
          {/key}
        {/if}
      {:catch}
        <p class="px-4 py-3 text-sm text-muted-foreground" role="alert">{copy.sidebar.navigationError}</p>
      {/await}
    {/if}
  </SidebarContent>
</Sidebar>
