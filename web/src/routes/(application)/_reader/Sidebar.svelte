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
  import { SurahSort, surahsInSortOrder } from "#lib/data/quran-browse.js";
  import { loadQuranData } from "#lib/data/quran-data-client.js";
  import { RangeKind, type QuranData } from "#lib/data/quran-data.js";
  import { hizbRange, juzOfPage } from "#lib/data/mushaf-divisions.js";
  import type { CatalogEntry, RangeEntry } from "#lib/data/quran-types.js";
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

  const BROWSE = [
    BrowseMode.Surah,
    BrowseMode.Ayah,
    BrowseMode.Juz,
    BrowseMode.Page,
    BrowseMode.Ruku,
    BrowseMode.Manzil,
    BrowseMode.Sajda,
  ] as const;
  const SORTS = [SurahSort.Mushaf, SurahSort.Revelation] as const;
  /** Range families the sidebar lists. Each value is a QuranData `RangeKind` literal. */
  type BrowseRangeKind = "juz" | "page" | "ruku" | "manzil";
  // Active rows take a soft primary fill instead of the grey sidebar accent.
  const ROW =
    "h-auto items-center gap-2 px-3 py-2.5 data-[active=true]:bg-primary/15 data-[active=true]:hover:bg-primary/15";
  const copy = getReaderUiCopy();
  const sidebar = useSidebar();
  const dataPromise = $derived(sidebar.openMobile ? loadQuranData() : null);

  let contentEl: HTMLDivElement | null = $state(null);
  let surahSort = $state<SurahSort>(SurahSort.Mushaf);

  function onItemClick() {
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

  const routeCtx = $derived<SurahRouteContext>(routeContextFromParams(page.params));

  function surahHref(slug: string): `/${string}` {
    return readerHrefFor(copy.locale, surahPathFor(routeCtx, slug));
  }

  function rangeKindFor(browse: BrowseMode): BrowseRangeKind {
    if (browse === BrowseMode.Juz) return "juz";
    if (browse === BrowseMode.Ruku) return "ruku";
    if (browse === BrowseMode.Manzil) return "manzil";
    return "page";
  }

  /** The surah and ayah a row opens, or null when the key names no surah in the data. */
  function verseTarget(
    quranData: QuranData,
    key: string,
  ): { surah: CatalogEntry; ayah: number } | null {
    const { num, n } = parseKey(key);
    const surah = quranData.surahByNum(num);
    return surah ? { surah, ayah: n } : null;
  }

  function verseHref(target: { surah: CatalogEntry; ayah: number }): string {
    return publicHref(
      readerHrefFor(copy.locale, surahAyahPathFor(routeCtx, target.surah, target.ayah)),
    );
  }

  /**
   * Juz and page rows link to their index routes. Ruku and manzil have no route
   * of their own, so they open the surah at the first verse they start on.
   */
  function rangeHref(kind: BrowseRangeKind, rg: RangeEntry, quranData: QuranData): string | null {
    if (kind === "ruku" || kind === "manzil") {
      const target = verseTarget(quranData, rg.first);
      return target ? verseHref(target) : null;
    }
    const quranHref =
      kind === "juz" ? juzPathFor(routeCtx, rg.index) : globalPagePathFor(routeCtx, rg.index);
    return publicHref(readerHrefFor(copy.locale, quranHref));
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

  /** Second line of a range row: where it ends, plus the juz or page count where that helps. */
  function rangeDetail(quranData: QuranData, kind: BrowseRangeKind, rg: RangeEntry): string {
    const end = rangeEnd(quranData, rg);
    if (kind === "juz") return `${end} · ${copy.index.pageCount(pagesInJuz(quranData, rg))}`;
    if (kind === "manzil") return end;
    return `${end} · ${copy.sidebar.rangeItem("juz", juzOfPage(quranData, rg.startGlobal))}`;
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
  entry: boolean = true,
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
        <a
          {...props}
          {href}
          data-session-origin={entry ? "index" : undefined}
          data-sveltekit-preload-data="hover"
          onclick={onItemClick}
        >
          {@render body()}
        </a>
      {/snippet}
    </SidebarMenuButton>
  </div>
{/snippet}

<Sidebar collapsible="offcanvas">
  <SidebarHeader>
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
          {copy.sidebar.mode(b)}
        </button>
      {/each}
    </div>
    {#if reader.browseSurah}
      <div class="mt-2 flex items-center gap-2">
        <span id="surah-sort-label" class="flex-none text-[12px] text-muted-foreground">
          {copy.sidebar.sortLabel}
        </span>
        <div
          class="grid flex-1 grid-cols-2 gap-1 rounded-md bg-background-subtle p-1"
          role="group"
          aria-labelledby="surah-sort-label"
        >
          {#each SORTS as sort (sort)}
            <button
              type="button"
              aria-pressed={surahSort === sort}
              onclick={() => (surahSort = sort)}
              class={cn(
                "rounded-sm py-1.5 text-[12px] font-medium transition-colors",
                surahSort === sort
                  ? "bg-foreground text-background hover:text-background"
                  : "text-muted-foreground hover:text-foreground-secondary",
              )}
            >
              {sort === SurahSort.Revelation ? copy.sidebar.sortRevelation : copy.sidebar.sortMushaf}
            </button>
          {/each}
        </div>
      </div>
    {/if}
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
          {@const surahs = surahsInSortOrder(quranData.surahs, surahSort)}
          <SidebarGroup>
            <SidebarGroupContent>
              <SidebarVirtualList
                getScrollElement={() => contentEl}
                count={surahs.length}
                estimateSize={56}
                activeIndex={surahs.findIndex((s) => s.slug === page.params.surah)}
              >
                {#snippet item(i)}
                  {@const s = surahs[i]!}
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
                      // Verse rows move within the surah being read: continuation.
                      false,
                    )}
                  {/if}
                {/snippet}
              </SidebarVirtualList>
            </SidebarGroupContent>
          </SidebarGroup>
          {/key}
        {:else if reader.browseMode === BrowseMode.Sajda}
          {@const sajdas = quranData.sajdas()}
          {@const cur = currentGlobal(quranData)}
          <SidebarGroup>
            <SidebarGroupContent>
              <SidebarVirtualList
                getScrollElement={() => contentEl}
                count={sajdas.length}
                estimateSize={44}
                activeIndex={sajdas.findIndex((s) => s.globalIndex === cur)}
              >
                {#snippet item(i)}
                  {@const sajda = sajdas[i]!}
                  {@const target = verseTarget(quranData, `${sajda.surah}:${sajda.ayah}`)}
                  {#if target}
                    {@const active = sajda.globalIndex === cur}
                    {#snippet body()}
                      <span
                        class={[
                          "w-11 flex-none text-[19px] font-medium tabular-nums",
                          active ? "text-primary" : "text-foreground",
                        ]}
                      >
                        <span class="sr-only">{copy.index.sajda} {i + 1}</span>
                        <span aria-hidden="true">{i + 1}</span>
                      </span>
                      <span class="flex min-w-0 flex-1 flex-col gap-0.5">
                        <span class="truncate text-[14.5px] font-medium text-foreground">
                          {target.surah.name} {sajda.surah}:{sajda.ayah}
                        </span>
                        <span class="truncate text-[12.5px] text-foreground-secondary">
                          {copy.index.sajda} · {copy.sidebar.sajdaKind(sajda.kind)}
                        </span>
                      </span>
                    {/snippet}
                    {@render navRow(verseHref(target), active, active ? "page" : undefined, ROW, body)}
                  {/if}
                {/snippet}
              </SidebarVirtualList>
            </SidebarGroupContent>
          </SidebarGroup>
        {:else}
          {@const rangeKind = rangeKindFor(reader.browseMode)}
          {@const ranges = quranData.ranges(rangeKind)}
          {@const activeRange = rangeRow(ranges, currentGlobal(quranData))}
          {#key rangeKind}
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
                  {@const href = rangeHref(rangeKind, rg, quranData)}
                  {@const active = i === activeRange}
                  {#if href}
                    {#snippet body()}
                      <span
                        class={[
                          "w-11 flex-none text-[19px] font-medium tabular-nums",
                          active ? "text-primary" : "text-foreground",
                        ]}
                      >
                        <span class="sr-only">{copy.sidebar.rangeItem(rangeKind, rg.index)}</span>
                        <span aria-hidden="true">{rg.index}</span>
                      </span>
                      <span class="flex min-w-0 flex-1 flex-col gap-0.5">
                        <span class="truncate text-[14.5px] font-medium text-foreground">
                          {surahNameOf(quranData, num)} {rg.first}
                        </span>
                        <span class="truncate text-[12.5px] text-foreground-secondary">
                          {rangeDetail(quranData, rangeKind, rg)}
                        </span>
                      </span>
                    {/snippet}
                    {@render navRow(href, active, active ? "page" : undefined, ROW, body)}
                  {/if}
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
