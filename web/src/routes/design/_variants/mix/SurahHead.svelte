<script lang="ts">
  import { Icon } from "#lib/components/icon/index.js";
  import type { HeadId, ResumeId } from "./axes";
  import type { MixSurah } from "./types";

  type ReaderMode = "verse" | "reading";

  let {
    head,
    resume,
    surah,
    mode,
    resumeName,
    resumeKey,
    onMode,
    onSmaller,
    onBigger,
  }: {
    head: HeadId;
    resume: ResumeId;
    surah: MixSurah;
    mode: ReaderMode;
    resumeName: string;
    resumeKey: string;
    onMode: (next: ReaderMode) => void;
    onSmaller: () => void;
    onBigger: () => void;
  } = $props();

  // Same hue-slot pairs the live ReaderHeader band uses.
  const HUE_SOFT = {
    1: "var(--hue-1-soft)",
    2: "var(--hue-2-soft)",
    3: "var(--hue-3-soft)",
    4: "var(--hue-4-soft)",
  } as const;
  const HUE_LEGIBLE = {
    1: "var(--hue-1-legible)",
    2: "var(--hue-2-legible)",
    3: "var(--hue-3-legible)",
    4: "var(--hue-4-legible)",
  } as const;

  const badge = $derived(String(surah.num).padStart(3, "0"));
  const pageLine = $derived(`Page 1 of ${surah.pageCount}`);
</script>

<!-- Live controls on the current band: grey wells and muted idle text. -->
{#snippet currentControls()}
  <div class="flex flex-wrap items-center justify-end gap-2">
    <div class="flex items-center gap-0.5 rounded-md bg-background-subtle p-1" role="group" aria-label="Arabic text size">
      <button
        type="button"
        onclick={onSmaller}
        aria-label="Smaller Arabic text"
        class="flex h-[26px] w-7 items-center justify-center rounded-pill text-[13px] text-foreground-secondary transition-colors hover:bg-surface-hover hover:text-foreground"
      >
        A&minus;
      </button>
      <button
        type="button"
        onclick={onBigger}
        aria-label="Larger Arabic text"
        class="flex h-[26px] w-7 items-center justify-center rounded-pill text-[15px] text-foreground-secondary transition-colors hover:bg-surface-hover hover:text-foreground"
      >
        A+
      </button>
    </div>
    <div class="flex items-center gap-0.5 rounded-md bg-background-subtle p-1" role="group" aria-label="Reading mode">
      <button
        type="button"
        aria-pressed={mode === "verse"}
        onclick={() => onMode("verse")}
        class="flex h-[26px] items-center gap-1.5 rounded-pill px-2.5 text-[13px] font-medium text-muted-foreground transition-colors hover:bg-surface-hover hover:text-foreground aria-pressed:bg-foreground aria-pressed:text-background aria-pressed:hover:bg-foreground aria-pressed:hover:text-background"
      >
        <Icon name="rows" size={13} />
        <span>Ayah-by-Ayah</span>
      </button>
      <button
        type="button"
        aria-pressed={mode === "reading"}
        onclick={() => onMode("reading")}
        class="flex h-[26px] items-center gap-1.5 rounded-pill px-2.5 text-[13px] font-medium text-muted-foreground transition-colors hover:bg-surface-hover hover:text-foreground aria-pressed:bg-foreground aria-pressed:text-background aria-pressed:hover:bg-foreground aria-pressed:hover:text-background"
      >
        <Icon name="continuous" size={13} />
        <span>Reading</span>
      </button>
    </div>
  </div>
{/snippet}

<!-- New variants: A−/A+ moved to the sticky bar; the mode switch sits in an outlined well on
     the reader's own ground, idle text at full strength. -->
{#snippet controls()}
  <div class="flex items-center gap-0.5 rounded-md border border-(--mix-line) p-0.5" role="group" aria-label="Reading mode">
    <button
      type="button"
      aria-pressed={mode === "verse"}
      onclick={() => onMode("verse")}
      class="flex h-8 items-center gap-1.5 rounded-sm px-3 text-[13px] font-medium text-foreground-secondary transition-colors hover:text-foreground aria-pressed:bg-foreground aria-pressed:text-background"
    >
      <Icon name="rows" size={13} />
      <span>Ayah-by-Ayah</span>
    </button>
    <button
      type="button"
      aria-pressed={mode === "reading"}
      onclick={() => onMode("reading")}
      class="flex h-8 items-center gap-1.5 rounded-sm px-3 text-[13px] font-medium text-foreground-secondary transition-colors hover:text-foreground aria-pressed:bg-foreground aria-pressed:text-background"
    >
      <Icon name="continuous" size={13} />
      <span>Reading</span>
    </button>
  </div>
{/snippet}

{#snippet resumeLine()}
  {#if resume === "d"}
    <button
      type="button"
      class="mt-1 inline-flex w-fit items-center gap-1.5 text-[13.5px] text-foreground-secondary transition-colors hover:text-foreground"
    >
      <Icon name="play" size={12} class="text-(--mix-accent-text)" />
      Last read <span class="font-medium text-foreground">{resumeName} {resumeKey}</span>
      <span class="text-(--mix-accent-text)">Jump <span aria-hidden="true">→</span></span>
    </button>
  {/if}
{/snippet}

{#if head === "a"}
  <div
    class="flex flex-wrap items-start justify-between gap-6 border-b border-border px-5 pb-[26px] pt-[30px] sm:px-9"
    style:background={HUE_SOFT[surah.hue]}
  >
    <div class="flex items-start gap-4">
      <div
        aria-hidden="true"
        class="flex h-16 w-16 flex-none items-center justify-center rounded-sm font-arabic text-lg font-bold"
        style:color={HUE_LEGIBLE[surah.hue]}
      >
        {badge}
      </div>
      <div class="flex min-w-0 flex-col gap-1.5">
        <span class="text-xs font-semibold uppercase tracking-[0.1em] text-foreground-secondary">
          Surah {surah.num} · {pageLine}
        </span>
        <div class="flex flex-wrap items-baseline gap-x-3.5 gap-y-1">
          <h1 class="text-[32px] font-semibold tracking-[-0.025em]">{surah.num}. {surah.name}</h1>
          <span dir="rtl" lang="ar" class="font-arabic text-[30px] leading-none text-foreground-secondary">
            {surah.arabic}
          </span>
        </div>
        <span class="text-sm text-foreground-secondary">{surah.meta}</span>
        {@render resumeLine()}
      </div>
    </div>
    {@render currentControls()}
  </div>
{:else if head === "b"}
  <div class="flex flex-wrap items-end justify-between gap-x-6 gap-y-5 border-b border-(--mix-line) px-5 pb-6 pt-8 sm:px-9">
    <div class="flex min-w-0 flex-col gap-2">
      <span class="text-[13.5px] font-semibold text-(--mix-accent-text)">Surah {surah.num} · {pageLine}</span>
      <div class="flex flex-wrap items-baseline gap-x-4 gap-y-1">
        <h1 class="text-[34px] font-semibold tracking-[-0.025em] text-foreground">{surah.name}</h1>
        <span dir="rtl" lang="ar" class="font-arabic text-[32px] leading-none text-foreground">{surah.arabic}</span>
      </div>
      <span class="text-[15px] text-foreground-secondary">{surah.meaning} · {surah.meta}</span>
      {@render resumeLine()}
    </div>
    {@render controls()}
  </div>
{:else if head === "c"}
  <div class="flex flex-col items-center gap-2 border-b border-(--mix-line) px-5 pb-7 pt-9 text-center sm:px-9">
    <span class="text-[13.5px] font-semibold text-(--mix-accent-text)">Surah {surah.num}</span>
    <span dir="rtl" lang="ar" class="font-arabic text-[52px] leading-[1.35] text-quran-foreground">{surah.arabic}</span>
    <h1 class="text-[26px] font-semibold tracking-[-0.02em] text-foreground">{surah.name}</h1>
    <span class="text-[15px] text-foreground-secondary">{surah.meaning} · {surah.meta} · {pageLine}</span>
    {@render resumeLine()}
    <div class="mt-4">{@render controls()}</div>
  </div>
{:else if head === "d"}
  <div
    class="flex flex-wrap items-end justify-between gap-x-6 gap-y-5 border-b border-(--mix-line) px-5 pb-6 pt-7 sm:px-9"
    style:border-top={`4px solid ${HUE_LEGIBLE[surah.hue]}`}
  >
    <div class="flex min-w-0 items-start gap-5">
      <span
        aria-hidden="true"
        class="pt-1 text-[44px] font-semibold leading-none tracking-[-0.03em] tabular-nums"
        style:color={HUE_LEGIBLE[surah.hue]}
      >
        {surah.num}
      </span>
      <div class="flex min-w-0 flex-col gap-2">
        <div class="flex flex-wrap items-baseline gap-x-4 gap-y-1">
          <h1 class="text-[32px] font-semibold tracking-[-0.025em] text-foreground">{surah.name}</h1>
          <span dir="rtl" lang="ar" class="font-arabic text-[30px] leading-none text-foreground">{surah.arabic}</span>
        </div>
        <span class="text-[15px] text-foreground-secondary">{surah.meaning} · {surah.meta} · {pageLine}</span>
        {@render resumeLine()}
      </div>
    </div>
    {@render controls()}
  </div>
{:else}
  <div class="flex flex-wrap items-center gap-x-4 gap-y-3 border-b border-(--mix-line) px-5 py-4 sm:px-9">
    <span
      class="flex size-9 flex-none items-center justify-center rounded-pill bg-primary text-[13px] font-semibold tabular-nums text-primary-foreground"
    >
      {surah.num}
    </span>
    <div class="flex min-w-0 flex-col">
      <div class="flex flex-wrap items-baseline gap-x-3">
        <h1 class="text-[21px] font-semibold tracking-[-0.015em] text-foreground">{surah.name}</h1>
        <span dir="rtl" lang="ar" class="font-arabic text-[24px] leading-none text-foreground">{surah.arabic}</span>
        <span class="text-[13.5px] text-foreground-secondary">{surah.meta} · {pageLine}</span>
      </div>
      {@render resumeLine()}
    </div>
    <div class="ms-auto">{@render controls()}</div>
  </div>
{/if}
