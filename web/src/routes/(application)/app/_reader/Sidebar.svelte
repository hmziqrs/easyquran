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
    SidebarFooter,
    SidebarGroup,
    SidebarGroupContent,
    SidebarMenuItem,
    SidebarMenuButton,
    useSidebar,
  } from "$lib/components/ui/sidebar";
  import SidebarVirtualList from "./SidebarVirtualList.svelte";
  import TranslationPicker from "./TranslationPicker.svelte";

  const BROWSE = [BrowseMode.Surah, BrowseMode.Ayah, BrowseMode.Juz, BrowseMode.Page] as const;
  const copy = getReaderUiCopy();
  const sidebar = useSidebar();
  const dataPromise = $derived(sidebar.openMobile ? loadQuranData() : null);

  let contentEl: HTMLDivElement | null = $state(null);

  function oninput(e: Event) {
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
  <SidebarMenuItem>
    <SidebarMenuButton isActive={isActive} aria-current={ariaCurrent} class={cls}>
      {#snippet child({ props })}
        <a {...props} {href} data-sveltekit-preload-data="hover" onclick={onItemClick}>
          {@render body()}
        </a>
      {/snippet}
    </SidebarMenuButton>
  </SidebarMenuItem>
{/snippet}

<Sidebar collapsible="offcanvas">
  <SidebarHeader>
    <div
      class="flex items-center gap-2.5 rounded-[11px] border border-line bg-bg-2 px-[13px] py-[11px] transition-colors focus-within:border-line-3 focus-within:ring-2 focus-within:ring-accent/40"
    >
      <Icon name="search" size={15} class="flex-none text-fg-3" />
      <Input
        type="text"
        value={reader.query}
        {oninput}
        placeholder={copy.sidebar.searchPlaceholder}
        aria-label={copy.sidebar.searchLabel}
        class="h-auto flex-1 border-0 bg-transparent px-0 py-0 text-sm text-fg shadow-none focus-visible:border-0 focus-visible:ring-0 placeholder:text-fg-3"
      />
      {#if reader.hasQuery}
        <button
          type="button"
          onclick={() => reader.clearQuery()}
          aria-label={copy.sidebar.clearSearch}
          class="flex-none text-fg-3 transition-colors hover:text-fg"
        >
          <Icon name="x" size={15} />
        </button>
      {/if}
    </div>

    <div
      class="grid grid-cols-4 gap-1 rounded-[10px] bg-bg-2 p-1"
      role="group"
      aria-label={copy.sidebar.browseLabel}
    >
      {#each BROWSE as b (b)}
        <button
          type="button"
          aria-pressed={reader.browseMode === b}
          onclick={() => selectBrowse(b)}
          class={cn(
            "rounded-[7px] py-2 text-[12.5px] font-medium capitalize transition-colors",
            reader.browseMode === b ? "bg-bg-3 text-fg shadow-sm" : "text-fg-3 hover:text-fg-2",
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
        <p class="px-4 py-3 text-sm text-fg-3" aria-live="polite">
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
                    <span class="flex min-w-0 flex-1 flex-col gap-1">
                      <span class="truncate text-sm font-medium">{s.num} · {s.name}</span>
                      <span class="text-[11.5px] text-fg-3">{surahMeta(s)}</span>
                    </span>
                    <span dir="rtl" class="flex-none font-arabic text-[17px] leading-none">
                      {s.arabic}
                    </span>
                  {/snippet}
                  {@render navRow(
                    publicHref(surahHref(s.slug)),
                    active,
                    active ? "page" : undefined,
                    "h-auto items-start gap-3 px-3.5 py-3",
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
                          class="flex h-6 w-6 flex-none items-center justify-center rounded-full border border-line text-[11px] text-fg-3"
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
          {#key reader.browseJuz ? "juz" : "page"}
          <SidebarGroup>
            <SidebarGroupContent>
              <SidebarVirtualList
                getScrollElement={() => contentEl}
                count={ranges.length}
                estimateSize={44}
                activeIndex={rangeRow(ranges, currentGlobal(quranData))}
              >
                {#snippet item(i)}
                  {@const rg = ranges[i]!}
                  {@const { num, n } = parseKey(rg.first)}
                  {@const href = publicHref(rangeHref(reader.browseJuz, rg.index))}
                  {#snippet body()}
                    <span
                      class="flex h-6 min-w-6 flex-none items-center justify-center rounded-full border border-line px-1.5 text-[10.5px] text-fg-3"
                    >
                      {copy.sidebar.rangeItem(reader.browseJuz ? "juz" : "page", rg.index)}
                    </span>
                    <span class="min-w-0 flex-1 truncate text-[13px] text-fg-2">
                      {quranData.surahByNum(num)?.name ?? `${copy.sidebar.mode("surah")} ${num}`} {num}:{n}
                    </span>
                  {/snippet}
                  {@render navRow(href, undefined, undefined, "h-auto gap-3 px-3.5 py-2.5", body)}
                {/snippet}
              </SidebarVirtualList>
            </SidebarGroupContent>
          </SidebarGroup>
          {/key}
        {/if}
      {:catch}
        <p class="px-4 py-3 text-sm text-fg-3" role="alert">{copy.sidebar.navigationError}</p>
      {/await}
    {/if}
  </SidebarContent>

  <SidebarFooter>
    <TranslationPicker />
    <span class="px-1 text-[11px] text-fg-3">{copy.sidebar.tip}</span>
  </SidebarFooter>
</Sidebar>
