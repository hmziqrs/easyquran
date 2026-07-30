<!--
  SurahReader — the main reading surface for a surah (verse-by-verse OR
  continuous reading mode). Which surah renders comes from the `surah` PROP
  (driven by the URL in /app/[surah]/+page.svelte); the reader store is used
  only for prefs (mode, font size), per-verse state, and the player — never for
  the current surah. Rendering from the prop is what makes deep links like
  /app/al-baqarah prerender the right surah with no hydration mismatch.

  Mirrors quran.com/al-baqarah's reading layout in EasyQuran tokens:
    · an optional "Continue reading" pill (reader.hasLastRead);
    · a surah Card — a decorative zero-padded number badge, the Surah {n}
      eyebrow, name + Arabic name + meta, an A−/A+ size control, a
      Verse-by-Verse / Reading tablist (full WAI-ARIA tabs: roving tabindex +
      Arrow/Home/End keys), and a "Listen" button;
    · the centered Basmala (skipped for Al-Fatihah, whose first verse is it);
    · the verse list (one VerseRow per ayah) in verse mode, or a single
      justified mushaf block with inline ayah medallions in reading mode — both
      live inside the tabpanel;
    · a prev/next footer (adjacentSurahs → goto(surahPath)).
-->
<script lang="ts">
  import { goto } from "$app/navigation";
  import { reader } from "$lib/stores/reader.svelte";
  import {
    adjacentSurahs,
    surahMeta,
    verseKey,
    BISMILLAH,
    showsBismillah,
    toArabicDigits,
    surahPath,
    type Surah,
  } from "$lib/data/quran";
  import { Icon } from "$lib/components/icon";
  import VerseRow from "./VerseRow.svelte";
  import { cn } from "$lib/utils";

  let { surah }: { surah: Surah } = $props();

  const adj = $derived(adjacentSurahs(surah.num));
  const showBasmala = $derived(showsBismillah(surah));
  const badge = $derived(String(surah.num).padStart(3, "0"));

  const MODES = ["verse", "reading"] as const;

  function continueReading() {
    const lr = reader.lastRead;
    if (lr) reader.openVerse(lr.num, lr.n);
  }

  // WAI-ARIA Tabs pattern: arrows move focus + activate (automatic activation),
  // Home/End jump to the ends. Roving tabindex is set per-tab in the markup.
  function onModeKey(e: KeyboardEvent) {
    const idx = MODES.indexOf(reader.mode);
    let next = idx;
    if (e.key === "ArrowRight" || e.key === "ArrowDown") next = (idx + 1) % MODES.length;
    else if (e.key === "ArrowLeft" || e.key === "ArrowUp")
      next = (idx - 1 + MODES.length) % MODES.length;
    else if (e.key === "Home") next = 0;
    else if (e.key === "End") next = MODES.length - 1;
    else return;
    e.preventDefault();
    const m = MODES[next];
    reader.setMode(m);
    document.getElementById("mode-" + m)?.focus();
  }
</script>

<div class="flex flex-col gap-4">
  {#if reader.hasLastRead}
    <button
      type="button"
      onclick={continueReading}
      class="flex items-center gap-3 rounded-[12px] bg-accent-soft px-[18px] py-[13px] text-left transition-[filter] duration-150 hover:brightness-[0.98]"
    >
      <Icon name="play" size={15} class="flex-none text-accent" />
      <span class="text-sm text-accent">Continue reading — {reader.lastReadRef}</span>
      <span class="ml-auto text-[13px] text-accent/75">Jump →</span>
    </button>
  {/if}

  <div class="overflow-hidden rounded-2xl border border-line bg-bg-1">
    <!-- header -->
    <div
      class="flex flex-wrap items-start justify-between gap-6 border-b border-line px-5 pb-[26px] pt-[30px] sm:px-9"
    >
      <!-- left: decorative number badge + title block -->
      <div class="flex items-start gap-4">
        <div
          aria-hidden="true"
          class="flex h-16 w-16 flex-none items-center justify-center rounded-2xl border border-accent bg-accent-soft font-arabic text-lg text-accent"
        >
          {badge}
        </div>
        <div class="flex min-w-0 flex-col gap-1.5">
          <span class="text-xs font-semibold uppercase tracking-[0.1em] text-accent">
            Surah {surah.num}
          </span>
          <div class="flex flex-wrap items-baseline gap-x-3.5 gap-y-1">
            <h1 class="text-[32px] font-semibold tracking-[-0.025em]">
              {surah.num}. {surah.name}
            </h1>
            <span dir="rtl" class="font-arabic text-[30px] leading-none text-fg-2">
              {surah.arabic}
            </span>
          </div>
          <span class="text-sm text-fg-3">{surahMeta(surah)}</span>
        </div>
      </div>

      <!-- right: action row -->
      <div class="flex flex-wrap items-center justify-end gap-2">
        <!-- A− / A+ segmented font-size control -->
        <div
          class="flex items-center gap-0.5 rounded-[9px] bg-bg-2 p-1"
          role="group"
          aria-label="Arabic text size"
        >
          <button
            type="button"
            onclick={() => reader.smaller()}
            aria-label="Smaller Arabic text"
            class="flex h-[26px] w-7 items-center justify-center rounded-md text-[13px] text-fg-2 transition-colors hover:bg-bg-3 hover:text-fg"
          >
            A&minus;
          </button>
          <button
            type="button"
            onclick={() => reader.bigger()}
            aria-label="Larger Arabic text"
            class="flex h-[26px] w-7 items-center justify-center rounded-md text-[15px] text-fg-2 transition-colors hover:bg-bg-3 hover:text-fg"
          >
            A+
          </button>
        </div>

        <!-- reading-mode tablist: Verse-by-Verse | Reading (WAI-ARIA tabs).
             Focus lives on the tabs (roving tabindex below), so the tablist
             container itself is intentionally non-focusable. -->
        <!-- svelte-ignore a11y_interactive_supports_focus -->
        <div
          class="flex items-center gap-0.5 rounded-[9px] bg-bg-2 p-1"
          role="tablist"
          aria-label="Reading mode"
          onkeydown={onModeKey}
        >
          <button
            type="button"
            id="mode-verse"
            role="tab"
            aria-selected={reader.isVerseMode}
            tabindex={reader.isVerseMode ? 0 : -1}
            onclick={() => reader.setMode("verse")}
            class={cn(
              "flex h-[26px] items-center gap-1.5 rounded-md px-2.5 text-[13px] font-medium transition-colors",
              reader.isVerseMode ? "bg-bg-3 text-fg" : "text-fg-3 hover:text-fg",
            )}
          >
            <Icon name="rows" size={13} />
            <span class="hidden sm:inline">Verse-by-Verse</span>
            <span class="sm:hidden">Verses</span>
          </button>
          <button
            type="button"
            id="mode-reading"
            role="tab"
            aria-selected={reader.isReadingMode}
            tabindex={reader.isReadingMode ? 0 : -1}
            onclick={() => reader.setMode("reading")}
            class={cn(
              "flex h-[26px] items-center gap-1.5 rounded-md px-2.5 text-[13px] font-medium transition-colors",
              reader.isReadingMode ? "bg-bg-3 text-fg" : "text-fg-3 hover:text-fg",
            )}
          >
            <Icon name="continuous" size={13} />
            <span>Reading</span>
          </button>
        </div>
      </div>
    </div>

    <!-- Basmala (Al-Fatihah's first verse already IS it, so it is not duplicated) -->
    {#if showBasmala}
      <p dir="rtl" class="py-2 text-center font-arabic text-fg-3">{BISMILLAH}</p>
    {/if}

    <!-- tabpanel: verse list OR reading block (both keyed by stable vKey) -->
    <div
      role="tabpanel"
      id="surah-text"
      aria-labelledby="mode-verse"
      tabindex="0"
      class="focus-visible:outline-2 focus-visible:outline-offset-[-2px] focus-visible:outline-line-3"
    >
      {#if reader.isReadingMode}
        <div
          dir="rtl"
          class="reading-text px-5 py-8 text-fg sm:px-9"
          style="font-size:{reader.arabicSizePx}"
        >{#each surah.verses as text, i (verseKey(surah.num, i + 1))}<span>{text}</span><span id="ayah-{i + 1}" class="ayah-marker">{toArabicDigits(i + 1)}</span> {/each}</div>
      {:else}
        <div class="flex flex-col">
          {#each surah.verses as text, i (verseKey(surah.num, i + 1))}
            <VerseRow text={text} n={i + 1} vKey={verseKey(surah.num, i + 1)} />
          {/each}
        </div>
      {/if}
    </div>

    <!-- prev / next -->
    <div class="flex items-center justify-between gap-4 px-5 py-[22px] sm:px-9">
      <button
        type="button"
        onclick={() => goto(surahPath(adj.prev.num))}
        class="flex items-center gap-1.5 text-sm text-fg-2 transition-colors hover:text-fg"
      >
        <span aria-hidden="true">←</span>
        {adj.prev.name}
      </button>
      <button
        type="button"
        onclick={() => goto(surahPath(adj.next.num))}
        class="flex items-center gap-1.5 text-sm text-fg-2 transition-colors hover:text-fg"
      >
        {adj.next.name}
        <span aria-hidden="true">→</span>
      </button>
    </div>
  </div>
</div>
