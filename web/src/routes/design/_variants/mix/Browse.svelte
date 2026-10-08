<script lang="ts">
  import type { Attachment } from "svelte/attachments";
  import { Icon } from "#lib/components/icon/index.js";
  import type { ListsId } from "./axes";
  import type { MixRangeRow, MixSurahRow } from "./types";

  type Tab = "surah" | "juz" | "page";

  let {
    lists,
    open,
    surahs,
    juz,
    pages,
    activeSurah,
    activeJuz,
    activePage,
    onClose,
  }: {
    lists: ListsId;
    open: boolean;
    surahs: readonly MixSurahRow[];
    juz: readonly MixRangeRow[];
    pages: readonly MixRangeRow[];
    activeSurah: number;
    activeJuz: number;
    activePage: number;
    onClose: () => void;
  } = $props();

  const TABS: readonly { id: Tab; label: string }[] = [
    { id: "surah", label: "Surah" },
    { id: "juz", label: "Juz" },
    { id: "page", label: "Page" },
  ];

  let tab = $state<Tab>("juz");
  let query = $state("");

  const needle = $derived(query.trim().toLowerCase());

  function rangeMatches(r: MixRangeRow): boolean {
    if (needle === "") return true;
    return (
      String(r.index) === needle ||
      r.startName.toLowerCase().includes(needle) ||
      r.first.startsWith(needle)
    );
  }

  const shownSurahs = $derived(
    needle === ""
      ? surahs
      : surahs.filter((s) => String(s.num) === needle || s.name.toLowerCase().includes(needle)),
  );
  const shownJuz = $derived(juz.filter((r) => rangeMatches(r)));
  const shownPages = $derived(pages.filter((r) => rangeMatches(r)));

  /** Pages grouped under the juz they start in, for the tile grid. */
  const pageGroups = $derived.by(() => {
    const groups: { juz: number; head: MixRangeRow | undefined; rows: MixRangeRow[] }[] = [];
    for (const row of shownPages) {
      const last = groups.at(-1);
      if (last && last.juz === row.juz) last.rows.push(row);
      else groups.push({ juz: row.juz, head: juz[row.juz - 1], rows: [row] });
    }
    return groups;
  });

  const reveal: Attachment<HTMLElement> = (el) => {
    el.scrollIntoView({ block: "center" });
  };

  function rangeEnd(r: MixRangeRow): string {
    if (r.startName === r.endName) return r.last;
    return `${r.endName} ${r.last}`;
  }
</script>

{#snippet currentRange(r: MixRangeRow, kind: string, active: boolean)}
  <button
    type="button"
    aria-current={active ? "page" : undefined}
    {@attach active && reveal}
    class={[
      "flex w-full items-center gap-3 rounded-md px-3.5 py-2.5 text-start transition-colors hover:bg-surface-hover",
      active && "bg-surface-hover",
    ]}
  >
    <span
      class="flex h-6 min-w-6 flex-none items-center justify-center rounded-pill border border-border px-1.5 text-[10.5px] text-muted-foreground"
    >
      {kind} {r.index}
    </span>
    <span class="min-w-0 flex-1 truncate text-[13px] text-foreground-secondary">{r.startName} {r.first}</span>
  </button>
{/snippet}

{#snippet rowsRange(r: MixRangeRow, active: boolean, sub: string)}
  <button
    type="button"
    aria-current={active ? "page" : undefined}
    {@attach active && reveal}
    class={[
      "grid w-full grid-cols-[2.75rem_1fr] items-center gap-2 rounded-md px-3 py-2.5 text-start transition-colors hover:bg-foreground/5",
      active && "bg-primary/15 hover:bg-primary/15",
    ]}
  >
    <span class={["text-[19px] font-semibold tabular-nums", active ? "text-(--mix-accent-text)" : "text-foreground"]}>
      {r.index}
    </span>
    <span class="flex min-w-0 flex-col gap-0.5">
      <span class="truncate text-[14.5px] font-medium text-foreground">{r.startName} {r.first}</span>
      <span class="truncate text-[12.5px] text-foreground-secondary">{sub}</span>
    </span>
  </button>
{/snippet}

{#snippet arabicRange(r: MixRangeRow, active: boolean, sub: string)}
  <button
    type="button"
    aria-current={active ? "page" : undefined}
    {@attach active && reveal}
    class={[
      "grid w-full grid-cols-[2.75rem_1fr] items-center gap-2 rounded-md px-3 py-2 text-start transition-colors hover:bg-foreground/5",
      active && "bg-primary/15 hover:bg-primary/15",
    ]}
  >
    <span class={["text-[19px] font-semibold tabular-nums", active ? "text-(--mix-accent-text)" : "text-foreground"]}>
      {r.index}
    </span>
    <span class="flex min-w-0 flex-col">
      <span dir="rtl" lang="ar" class="truncate text-end font-quran text-[21px] leading-[1.9] text-foreground">
        {r.opening}
      </span>
      <span class="truncate text-[12.5px] text-foreground-secondary">{sub}</span>
    </span>
  </button>
{/snippet}

{#if open}
  <button
    type="button"
    aria-label="Close browse"
    onclick={onClose}
    class="fixed inset-0 z-[70] cursor-default bg-black/55"
  ></button>
  <aside
    aria-label="Browse"
    class="fixed inset-y-0 left-0 z-[71] flex w-[min(360px,88vw)] flex-col border-e border-(--mix-line) bg-(--mix-page)"
  >
    <div class="flex flex-col gap-2 p-2">
      <div class="flex items-center gap-2">
        <div
          class="flex min-w-0 flex-1 items-center gap-2.5 rounded-md border border-(--mix-line) px-[13px] py-[11px] transition-colors focus-within:border-primary"
        >
          <Icon name="search" size={15} class="flex-none text-foreground-secondary" />
          <input
            type="text"
            bind:value={query}
            placeholder="Search surah, number or Arabic…"
            aria-label="Search surahs, juz and pages"
            class="min-w-0 flex-1 bg-transparent text-sm text-foreground outline-none placeholder:text-foreground-secondary"
          />
        </div>
        <button
          type="button"
          onclick={onClose}
          aria-label="Close browse"
          class="flex size-10 flex-none items-center justify-center rounded-md text-foreground transition-colors hover:bg-foreground/5"
        >
          <Icon name="x" size={16} />
        </button>
      </div>
      <div class="grid grid-cols-3 gap-1 rounded-md border border-(--mix-line) p-1" role="group" aria-label="Browse by">
        {#each TABS as t (t.id)}
          <button
            type="button"
            aria-pressed={tab === t.id}
            onclick={() => (tab = t.id)}
            class="rounded-sm py-2 text-[12.5px] font-medium text-foreground-secondary transition-colors hover:text-foreground aria-pressed:bg-primary aria-pressed:text-primary-foreground"
          >
            {t.label}
          </button>
        {/each}
      </div>
    </div>

    <div class="min-h-0 flex-1 overflow-y-auto overscroll-contain px-2 pb-6">
      {#if tab === "surah"}
        {#if lists === "c"}
          <div class="grid grid-cols-2 gap-1.5">
            {#each shownSurahs as s (s.num)}
              {@const active = s.num === activeSurah}
              <button
                type="button"
                aria-current={active ? "page" : undefined}
                {@attach active && reveal}
                class={[
                  "flex min-w-0 flex-col gap-1 rounded-lg border p-3 text-start transition-colors hover:border-primary",
                  active ? "border-primary bg-primary/15" : "border-(--mix-line)",
                ]}
              >
                <span class="flex items-baseline justify-between gap-2">
                  <span class="text-[13px] font-semibold tabular-nums text-(--mix-accent-text)">{s.num}</span>
                  <span dir="rtl" lang="ar" class="truncate font-quran text-[17px] leading-none text-foreground">{s.arabic}</span>
                </span>
                <span class="truncate text-[14px] font-medium text-foreground">{s.name}</span>
                <span class="truncate text-[12px] text-foreground-secondary">{s.meta}</span>
              </button>
            {/each}
          </div>
        {:else}
          <ul class="flex list-none flex-col gap-0.5">
            {#each shownSurahs as s (s.num)}
              {@const active = s.num === activeSurah}
              <li>
                {#if lists === "a"}
                  <button
                    type="button"
                    aria-current={active ? "page" : undefined}
                    {@attach active && reveal}
                    class={[
                      "flex w-full items-start gap-3 rounded-md px-3.5 py-3 text-start transition-colors hover:bg-surface-hover",
                      active && "bg-surface-hover",
                    ]}
                  >
                    <span class="flex min-w-0 flex-1 flex-col gap-1">
                      <span class="truncate text-sm font-medium">{s.num} · {s.name}</span>
                      <span class="text-[11.5px] text-muted-foreground">{s.meta}</span>
                    </span>
                    <span dir="rtl" class="flex-none font-arabic text-[17px] leading-none">{s.arabic}</span>
                  </button>
                {:else}
                  <button
                    type="button"
                    aria-current={active ? "page" : undefined}
                    {@attach active && reveal}
                    class={[
                      "grid w-full grid-cols-[2.75rem_1fr_auto] items-center gap-2 rounded-md px-3 py-2.5 text-start transition-colors hover:bg-foreground/5",
                      active && "bg-primary/15 hover:bg-primary/15",
                    ]}
                  >
                    <span class={["text-[15px] font-semibold tabular-nums", active ? "text-(--mix-accent-text)" : "text-foreground"]}>
                      {s.num}
                    </span>
                    <span class="flex min-w-0 flex-col gap-0.5">
                      <span class="truncate text-[14.5px] font-medium text-foreground">{s.name}</span>
                      <span class="truncate text-[12.5px] text-foreground-secondary">{s.meaning} · {s.meta}</span>
                    </span>
                    <span
                      dir="rtl"
                      lang="ar"
                      class={["flex-none font-quran leading-none text-foreground", lists === "d" ? "text-[24px]" : "text-[19px]"]}
                    >
                      {s.arabic}
                    </span>
                  </button>
                {/if}
              </li>
            {/each}
          </ul>
        {/if}
      {:else if tab === "juz"}
        {#if lists === "c"}
          <div class="grid grid-cols-2 gap-1.5">
            {#each shownJuz as r (r.index)}
              {@const active = r.index === activeJuz}
              <button
                type="button"
                aria-current={active ? "page" : undefined}
                {@attach active && reveal}
                class={[
                  "flex min-w-0 flex-col justify-between gap-2 rounded-lg border p-3 text-start transition-colors hover:border-primary",
                  active ? "border-primary bg-primary/15" : "border-(--mix-line)",
                ]}
              >
                <span class="flex items-baseline gap-1.5">
                  <span class="text-[12px] font-semibold text-foreground-secondary">Juz</span>
                  <span class={["text-[28px] font-semibold leading-none tabular-nums", active ? "text-(--mix-accent-text)" : "text-foreground"]}>
                    {r.index}
                  </span>
                </span>
                <span class="flex min-w-0 flex-col">
                  <span class="truncate text-[13.5px] font-medium text-foreground">{r.startName}</span>
                  <span class="truncate text-[12px] text-foreground-secondary">{r.first} · {r.pages} pages</span>
                </span>
              </button>
            {/each}
          </div>
        {:else}
          <ul class="flex list-none flex-col gap-0.5">
            {#each shownJuz as r (r.index)}
              {@const active = r.index === activeJuz}
              <li>
                {#if lists === "a"}
                  {@render currentRange(r, "Juz", active)}
                {:else if lists === "b"}
                  {@render rowsRange(r, active, `to ${rangeEnd(r)} · ${r.pages} pages`)}
                {:else}
                  {@render arabicRange(r, active, `${r.startName} ${r.first} · ${r.pages} pages`)}
                {/if}
              </li>
            {/each}
          </ul>
        {/if}
      {:else if lists === "c"}
        {#each pageGroups as group (group.juz)}
          <section class="pb-3">
            <h3 class="sticky top-0 z-10 bg-(--mix-page) px-1 py-2 text-[12.5px] font-semibold text-foreground-secondary">
              Juz {group.juz}
              {#if group.head}<span class="font-normal">· {group.head.startName} {group.head.first}</span>{/if}
            </h3>
            <div class="grid grid-cols-5 gap-1.5">
              {#each group.rows as r (r.index)}
                {@const active = r.index === activePage}
                <button
                  type="button"
                  title={`${r.startName} ${r.first}`}
                  aria-label={`Page ${r.index}, ${r.startName} ${r.first}`}
                  aria-current={active ? "page" : undefined}
                  {@attach active && reveal}
                  class={[
                    "flex h-11 items-center justify-center rounded-md border text-[14px] font-medium tabular-nums transition-colors",
                    active
                      ? "border-primary bg-primary text-primary-foreground"
                      : "border-(--mix-line) text-foreground hover:border-primary",
                  ]}
                >
                  {r.index}
                </button>
              {/each}
            </div>
          </section>
        {/each}
      {:else}
        <ul class="flex list-none flex-col gap-0.5">
          {#each shownPages as r (r.index)}
            {@const active = r.index === activePage}
            <li>
              {#if lists === "a"}
                {@render currentRange(r, "Page", active)}
              {:else if lists === "b"}
                {@render rowsRange(r, active, `to ${rangeEnd(r)} · Juz ${r.juz}`)}
              {:else}
                {@render arabicRange(r, active, `${r.startName} ${r.first} · Juz ${r.juz}`)}
              {/if}
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  </aside>
{/if}
