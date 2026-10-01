<script lang="ts">
  import { page } from "$app/state";
  import { BrowseMode, reader } from "$lib/stores/reader.svelte";
  import {
    globalPagePathFor,
    juzPathFor,
    routeContextFromParams,
    surahAyahPathFor,
    surahMeta,
    surahPathFor,
    parseKey,
    type SurahRouteContext,
  } from "$lib/data/quran";
  import { loadQuranData } from "$lib/data/quran-data-client";
  import { RangeKind, type QuranData } from "$lib/data/quran-data";
  import type { RangeEntry } from "$lib/data/quran-types";
  import { Icon } from "$lib/components/icon";
  import { Input } from "$lib/components/ui/input";
  import { getReaderUiCopy } from "$lib/i18n/reader-copy";
  import { readerHrefFor } from "$lib/i18n/reader";
  import { publicHref } from "$lib/i18n/public-href";
  import { cn } from "$lib/utils";
  import type { Snippet } from "svelte";
  import {
    Sidebar,
    SidebarHeader,
    SidebarContent,
    SidebarGroup,
    SidebarGroupContent,
    SidebarMenuButton,
    useSidebar,
  } from "$lib/components/ui/sidebar";
  import SidebarVirtualList from "./SidebarVirtualList.svelte";

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
    const slug = page.params.surah;
    if (!slug) return;
    void loadQuranData().then((quranData) => {
      const current = quranData.surahBySlug(slug);
      if (current) void reader.refreshFromWorker(current.num);
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

  function toIndex(v: string | undefined): number | null {
    const n = v ? Number(v) : Number.NaN;
    return Number.isSafeInteger(n) && n > 0 ? n : null;
  }

  /** Global ayah the current route points at, used to reveal the matching row. */
  function currentGlobal(quranData: QuranData): number | null {
    const slug = page.params.surah;
    if (slug) {
      const s = quranData.surahBySlug(slug);
      if (!s) return null;
      const localPage = toIndex(page.params.localPage) ?? 1;
      return quranData.surahLocalPage(s.num, localPage)?.startGlobal ?? s.startGlobal;
    }
    const n = toIndex(page.params.n);
    if (n === null) return null;
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

  function juzOf(quranData: QuranData, global: number): number {
    return quranData.ranges(RangeKind.Juz).find((j) => global >= j.startGlobal && global <= j.endGlobal)?.index ?? 1;
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
    <!-- One focus frame: the wrapper takes it (border → primary), the input drops its own
         pill-shaped outline, which used to draw a second ring inside this box. -->
    <div
      class="flex items-center gap-2.5 rounded-md border border-border bg-background-subtle px-[13px] py-[11px] transition-colors focus-within:border-primary"
    >
      <Icon name="search" size={15} class="flex-none text-muted-foreground" />
      <Input
        type="text"
        value={reader.query}
        {oninput}
        placeholder={copy.sidebar.searchPlaceholder}
        aria-label={copy.sidebar.searchLabel}
        class="h-auto flex-1 rounded-none border-0 bg-transparent px-0 py-0 text-sm text-foreground shadow-none focus-visible:border-0 focus-visible:outline-none placeholder:text-muted-foreground"
      />
      {#if reader.hasQuery}
        <button
          type="button"
          onclick={() => reader.clearQuery()}
          aria-label={copy.sidebar.clearSearch}
          class="flex-none text-muted-foreground transition-colors hover:text-foreground"
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
        {@const current = page.params.surah ? quranData.surahBySlug(page.params.surah) : undefined}
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
                        "w-9 flex-none text-[15px] font-semibold tabular-nums",
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
          {#if current}
            {@const cur = current}
            {@const verses = reader.versesFor(cur.num)}
            {#key verses.length}
            <SidebarGroup>
              <SidebarGroupContent>
                <SidebarVirtualList
                  getScrollElement={() => contentEl}
                  count={verses.length}
                  estimateSize={44}
                  activeIndex={(quranData.surahLocalPage(cur.num, toIndex(page.params.localPage) ?? 1)
                    ?.startAyah ?? 1) - 1}
                >
                  {#snippet item(i)}
                    {@const n = i + 1}
                    {@const v = verses[i]}
                    {@const localPage = quranData.surahLocalPageForAyah(cur.num, n)}
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
                          readerHrefFor(
                            copy.locale,
                            surahAyahPathFor(routeCtx, cur, localPage?.localPage ?? 1, n),
                          ),
                        ),
                        undefined,
                        undefined,
                        "h-auto gap-3 px-3.5 py-2.5",
                        body,
                      )}
                    {/if}
                  {/snippet}
                </SidebarVirtualList>
              </SidebarGroupContent>
            </SidebarGroup>
            {/key}
          {/if}
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
                        "w-11 flex-none text-[19px] font-semibold tabular-nums",
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
                          {copy.sidebar.rangeItem("juz", juzOf(quranData, rg.startGlobal))}
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
