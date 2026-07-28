<!--
  Sidebar — the sticky left rail of the reader (hidden below md). Owns:
    · the search input (bound to reader.query via reader.setQuery, with a clear
      button once reader.hasQuery);
    · the Surahs / Bookmarks tab switch (reader.setTab), Bookmarks showing the
      live reader.bookmarkCount;
    · the scrollable surah list (SURAHS, active = reader.current) or the
      bookmark list (reader.bookmarkList) with its dashed empty state.
  All state lives in the reader store; this component only calls its methods.
-->
<script lang="ts">
  import { reader } from "$lib/stores/reader.svelte";
  import { SURAHS, surahMeta } from "$lib/data/quran";
  import { Icon } from "$lib/components/icon";
  import { Input } from "$lib/components/ui/input";
  import { cn } from "$lib/utils";

  function oninput(e: Event) {
    reader.setQuery((e.currentTarget as HTMLInputElement).value);
  }
</script>

<aside class="hidden flex-col gap-3 md:flex md:sticky md:top-[88px]">
  <!-- search box -->
  <div
    class="flex items-center gap-2.5 rounded-[11px] border border-line bg-bg-2 px-[13px] py-[11px] transition-colors focus-within:border-line-3 focus-within:ring-2 focus-within:ring-accent/40"
  >
    <Icon name="search" size={15} class="flex-none text-fg-3" />
    <Input
      type="text"
      value={reader.query}
      {oninput}
      placeholder="Search surah, number or Arabic…"
      aria-label="Search surahs and verses"
      class="h-auto flex-1 border-0 bg-transparent px-0 py-0 text-sm text-fg shadow-none focus-visible:border-0 focus-visible:ring-0 placeholder:text-fg-3 dark:bg-transparent"
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

  <!-- Surahs / Bookmarks tab switch -->
  <div class="flex gap-1 rounded-[10px] bg-bg-2 p-1">
    <button
      type="button"
      onclick={() => reader.setTab("surahs")}
      aria-pressed={reader.isSurahTab}
      class={cn(
        "flex-1 rounded-[7px] py-2 text-[13px] font-medium transition-colors",
        reader.isSurahTab ? "bg-bg-3 text-fg" : "text-fg-3 hover:text-fg-2",
      )}
    >
      Surahs
    </button>
    <button
      type="button"
      onclick={() => reader.setTab("bookmarks")}
      aria-pressed={reader.isBookmarkTab}
      class={cn(
        "flex-1 rounded-[7px] py-2 text-[13px] font-medium transition-colors",
        reader.isBookmarkTab ? "bg-bg-3 text-fg" : "text-fg-3 hover:text-fg-2",
      )}
    >
      Bookmarks{reader.bookmarkCount ? ` (${reader.bookmarkCount})` : ""}
    </button>
  </div>

  {#if reader.isSurahTab}
    <div class="flex max-h-[calc(100vh-240px)] flex-col gap-[3px] overflow-y-auto pr-0.5">
      {#each SURAHS as s (s.num)}
        {@const active = reader.current === s.num}
        <button
          type="button"
          onclick={() => reader.setCurrent(s.num)}
          aria-current={active ? "true" : undefined}
          class={cn(
            "flex items-center justify-between gap-2 rounded-[9px] px-[13px] py-[11px] text-left transition-colors",
            active ? "bg-accent-soft" : "hover:bg-bg-2",
          )}
        >
          <span class="flex min-w-0 flex-col gap-px">
            <span class={cn("truncate text-sm font-medium", active ? "text-accent" : "text-fg-2")}>
              {s.num} · {s.name}
            </span>
            <span class="text-[11.5px] text-fg-3">{surahMeta(s)}</span>
          </span>
          <span
            dir="rtl"
            class={cn("flex-none font-arabic text-[17px]", active ? "text-accent" : "text-fg-2")}
          >
            {s.arabic}
          </span>
        </button>
      {/each}
    </div>
  {:else}
    <div class="flex flex-col gap-1.5">
      {#if reader.bookmarkCount === 0}
        <div
          class="rounded-[11px] border border-dashed border-line px-4 py-6 text-center text-[13.5px] leading-relaxed text-fg-3"
        >
          No bookmarks yet. Tap the ribbon beside any verse to save it here.
        </div>
      {:else}
        {#each reader.bookmarkList as b (b.key)}
          <button
            type="button"
            onclick={() => reader.openVerse(b.num, b.n)}
            class="flex flex-col gap-1.5 rounded-[10px] bg-bg-2 px-[13px] py-3 text-left transition-colors hover:bg-accent-soft"
          >
            <span class="text-xs font-semibold text-accent">{b.ref}</span>
            <span
              dir="rtl"
              class="block max-h-[34px] overflow-hidden font-arabic text-lg leading-[1.9] text-fg-2"
            >
              {b.text}
            </span>
          </button>
        {/each}
      {/if}
    </div>
  {/if}
</aside>
