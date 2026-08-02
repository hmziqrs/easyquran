<script lang="ts">
  import { page } from "$app/state";
  import { resolve } from "$app/paths";
  import { reader } from "$lib/stores/reader.svelte";
  import {
    surahAyahPath,
    surahMeta,
    surahPath,
    parseKey,
    verseKey,
  } from "$lib/data/quran";
  import { loadQuranData } from "$lib/data/quran-data-client";
  import { RangeKind } from "$lib/data/quran-data";
  import { Icon } from "$lib/components/icon";
  import { Input } from "$lib/components/ui/input";
  import { cn } from "$lib/utils";
  import {
    Sidebar,
    SidebarHeader,
    SidebarContent,
    SidebarFooter,
    SidebarGroup,
    SidebarGroupContent,
    SidebarMenu,
    SidebarMenuItem,
    SidebarMenuButton,
    useSidebar,
  } from "$lib/components/ui/sidebar";

  const BROWSE = ["surah", "ayah", "juz", "page"] as const;
  const sidebar = useSidebar();
  const dataPromise = $derived(sidebar.openMobile ? loadQuranData() : null);

  let inputEl: HTMLInputElement | null = $state(null);

  function oninput(e: Event) {
    reader.setQuery((e.currentTarget as HTMLInputElement).value);
  }
  function onKey(e: KeyboardEvent) {
    if (e.key === "k" && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      inputEl?.focus();
      inputEl?.select();
    }
  }
  function onItemClick() {
    reader.clearQuery();
    sidebar.setOpenMobile(false);
  }

  function selectBrowse(browse: (typeof BROWSE)[number]) {
    reader.setBrowse(browse);
    if (browse !== "ayah") return;
    void loadQuranData().then((quranData) => {
      const current = quranData.surahBySlug(page.params.surah as string);
      if (current) void reader.refreshFromWorker(current.num);
    });
  }
</script>

<svelte:window onkeydown={onKey} />

<Sidebar collapsible="offcanvas">
  <SidebarHeader>
    <div
      class="flex items-center gap-2.5 rounded-[11px] border border-line bg-bg-2 px-[13px] py-[11px] transition-colors focus-within:border-line-3 focus-within:ring-2 focus-within:ring-accent/40"
    >
      <Icon name="search" size={15} class="flex-none text-fg-3" />
      <Input
        bind:ref={inputEl}
        type="text"
        value={reader.query}
        {oninput}
        placeholder="Search surah, number or Arabic…"
        aria-label="Search surahs and ayahs"
        class="h-auto flex-1 border-0 bg-transparent px-0 py-0 text-sm text-fg shadow-none focus-visible:border-0 focus-visible:ring-0 placeholder:text-fg-3"
      />
      {#if reader.hasQuery}
        <button
          type="button"
          onclick={() => reader.clearQuery()}
          aria-label="Clear search"
          class="flex-none text-fg-3 transition-colors hover:text-fg"
        >
          <Icon name="x" size={15} />
        </button>
      {/if}
    </div>

    <div class="grid grid-cols-4 gap-1 rounded-[10px] bg-bg-2 p-1" role="group" aria-label="Browse">
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
          {b}
        </button>
      {/each}
    </div>
  </SidebarHeader>

  <SidebarContent>
    {#if dataPromise}
      {#await dataPromise}
        <p class="px-4 py-3 text-sm text-fg-3">Loading Quran navigation…</p>
      {:then quranData}
        {@const current = quranData.surahBySlug(page.params.surah as string)}
        {#if reader.browseSurah}
          <SidebarGroup>
            <SidebarGroupContent>
              <SidebarMenu class="gap-1">
                {#each quranData.surahs as s (s.num)}
                  {@const active = page.params.surah === s.slug}
                  <SidebarMenuItem>
                    <SidebarMenuButton
                      isActive={active}
                      aria-current={active ? "page" : undefined}
                      class="h-auto items-start gap-3 px-3.5 py-3"
                    >
                      {#snippet child({ props })}
                        <a
                          {...props}
                          href={resolve(surahPath(s))}
                          data-sveltekit-preload-data="hover"
                          onclick={onItemClick}
                        >
                          <span class="flex min-w-0 flex-1 flex-col gap-1">
                            <span class="truncate text-sm font-medium">{s.num} · {s.name}</span>
                            <span class="text-[11.5px] text-fg-3">{surahMeta(s)}</span>
                          </span>
                          <span dir="rtl" class="flex-none font-arabic text-[17px] leading-none">
                            {s.arabic}
                          </span>
                        </a>
                      {/snippet}
                    </SidebarMenuButton>
                  </SidebarMenuItem>
                {/each}
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>
        {:else if reader.browseAyah}
          <SidebarGroup>
            <SidebarGroupContent>
              <SidebarMenu class="gap-1">
                {#if current}
                  {#each reader.versesFor(current.num) as v, i (verseKey(current.num, i + 1))}
                    {@const n = i + 1}
                    {@const localPage = quranData.surahLocalPageForAyah(current.num, n)}
                    {#if v}
                      <SidebarMenuItem>
                      <SidebarMenuButton class="h-auto gap-3 px-3.5 py-2.5">
                        {#snippet child({ props })}
                          <a
                            {...props}
                            href={resolve(
                              localPage
                                ? surahAyahPath(current, localPage.localPage, n)
                                : surahPath(current, n),
                            )}
                            data-sveltekit-preload-data="hover"
                            onclick={onItemClick}
                          >
                            <span
                              class="flex h-6 w-6 flex-none items-center justify-center rounded-full border border-line text-[11px] text-fg-3"
                            >
                              {n}
                            </span>
                            <span
                              dir="rtl"
                              class="min-w-0 flex-1 truncate font-arabic text-[15px]"
                            >
                              {v}
                            </span>
                          </a>
                        {/snippet}
                      </SidebarMenuButton>
                      </SidebarMenuItem>
                    {/if}
                  {/each}
                {/if}
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>
        {:else}
          {@const ranges = quranData.ranges(
            reader.browseJuz ? RangeKind.Juz : RangeKind.Page,
          )}
          <SidebarGroup>
            <SidebarGroupContent>
              <SidebarMenu class="gap-1">
                {#each ranges as rg (rg.index)}
                  {@const { num, n } = parseKey(rg.first)}
                  {@const href = resolve(
                    `/app/${reader.browseJuz ? "juz" : "page"}/${rg.index}`,
                  )}
                  <SidebarMenuItem>
                    <SidebarMenuButton class="h-auto gap-3 px-3.5 py-2.5">
                      {#snippet child({ props })}
                        <a
                          {...props}
                          {href}
                          data-sveltekit-preload-data="hover"
                          onclick={onItemClick}
                        >
                          <span
                            class="flex h-6 min-w-6 flex-none items-center justify-center rounded-full border border-line px-1.5 text-[10.5px] text-fg-3"
                          >
                            {reader.browseJuz ? "Juz" : "Pg"} {rg.index}
                          </span>
                          <span class="min-w-0 flex-1 truncate text-[13px] text-fg-2">
                            {quranData.surahByNum(num)?.name ?? `Surah ${num}`} {num}:{n}
                          </span>
                        </a>
                      {/snippet}
                    </SidebarMenuButton>
                  </SidebarMenuItem>
                {/each}
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>
        {/if}
      {:catch}
        <p class="px-4 py-3 text-sm text-fg-3">Quran navigation could not be loaded.</p>
      {/await}
    {/if}
  </SidebarContent>

  <SidebarFooter>
    <span class="px-1 text-[11px] text-fg-3">Tip: press Ctrl+K to search</span>
  </SidebarFooter>
</Sidebar>
