<script lang="ts">
  import ChevronDownIcon from "@lucide/svelte/icons/chevron-down";
  import PanelLeftIcon from "@lucide/svelte/icons/panel-left";
  import { Icon } from "$lib/components/icon";
  import TranslateMark from "./TranslateMark.svelte";
  import type { BarId, ResumeId } from "./axes";
  import type { MixSurah, MixTranslation } from "./types";

  let {
    bar,
    resume,
    surah,
    translations,
    resumeName,
    resumeKey,
    showSize,
    onBrowse,
    onTranslations,
    onSmaller,
    onBigger,
  }: {
    bar: BarId;
    resume: ResumeId;
    surah: MixSurah;
    translations: readonly MixTranslation[];
    resumeName: string;
    resumeKey: string;
    /** A−/A+ live in the bar for the new headers; the current header keeps its own pair. */
    showSize: boolean;
    onBrowse: () => void;
    onTranslations: () => void;
    onSmaller: () => void;
    onBigger: () => void;
  } = $props();

  const CHIP_LIMIT = 2;
  const chips = $derived(translations.slice(0, CHIP_LIMIT));
  const chipOverflow = $derived(Math.max(0, translations.length - CHIP_LIMIT));
  const names = $derived(translations.map((t) => t.name).join(", "));
  const label = $derived(
    translations.length === 0 ? "Translations: Arabic only" : `Translations: ${names}`,
  );
</script>

<!-- Same frame as the live ReaderShell bar; z above the design header so it takes over on scroll. -->
<header class="sticky top-0 z-[60] border-b border-(--mix-line) bg-(--mix-page)/90 py-2 backdrop-blur-xl">
  <div class="flex h-11 w-full items-center gap-2 px-3 sm:gap-3 sm:px-7 lg:px-10">
    <button
      type="button"
      onclick={onBrowse}
      aria-label="Browse surahs, juz and pages"
      class="flex size-9 flex-none items-center justify-center rounded-md text-foreground-secondary transition-colors hover:text-foreground"
    >
      <PanelLeftIcon class="size-[18px]" />
    </button>

    {#if bar === "a"}
      <button
        type="button"
        aria-label={label}
        aria-haspopup="dialog"
        onclick={onTranslations}
        title={label}
        class="relative flex h-11 w-11 flex-none items-center justify-center rounded-md border border-border bg-background-subtle text-foreground-secondary transition-colors hover:border-border-strong hover:text-foreground"
      >
        <TranslateMark />
        {#if translations.length > 0}
          <span
            class="absolute -end-1.5 -top-1.5 flex h-[17px] min-w-[17px] items-center justify-center rounded-pill bg-foreground px-1 text-[10px] font-medium text-background"
          >
            {translations.length}
          </span>
        {/if}
      </button>
    {:else if bar === "b"}
      <button
        type="button"
        aria-label={label}
        aria-haspopup="dialog"
        onclick={onTranslations}
        class="flex h-10 flex-none items-center gap-2 rounded-md border border-(--mix-line) ps-2.5 pe-2.5 text-[14px] font-medium text-foreground transition-colors hover:border-primary sm:pe-2"
      >
        <TranslateMark />
        <span>{translations.length === 1 ? "Translation" : "Translations"}</span>
        {#if translations.length > 0}
          <span
            class="flex h-5 min-w-5 flex-none items-center justify-center rounded-pill bg-primary px-1.5 text-[12px] font-semibold tabular-nums text-primary-foreground"
          >
            {translations.length}
          </span>
        {/if}
        <ChevronDownIcon class="hidden size-4 flex-none text-foreground-secondary sm:block" />
      </button>
    {:else if bar === "c"}
      <button
        type="button"
        aria-label={label}
        aria-haspopup="dialog"
        onclick={onTranslations}
        class="flex h-10 min-w-0 flex-none items-center gap-1.5 rounded-md px-1.5 transition-colors hover:bg-foreground/5"
      >
        <TranslateMark />
        {#each chips as t (t.id)}
          <span
            class="flex h-8 min-w-0 items-center gap-1.5 rounded-pill border border-(--mix-line) px-2.5 text-[13px] text-foreground"
          >
            <span class="text-[11px] font-bold uppercase tracking-wide text-(--mix-accent-text)">{t.languageCode}</span>
            <!-- Phones keep the language codes only; the names would push the bar sideways. -->
            <span class="hidden max-w-[9rem] truncate sm:inline" dir="auto">{t.name}</span>
          </span>
        {/each}
        {#if chipOverflow > 0}
          <span class="text-[13px] font-semibold text-foreground-secondary">+{chipOverflow}</span>
        {/if}
        {#if translations.length === 0}
          <span class="text-[13px] text-foreground">Arabic only</span>
        {/if}
      </button>
    {:else}
      <button
        type="button"
        aria-label={label}
        aria-haspopup="dialog"
        onclick={onTranslations}
        class="flex h-10 min-w-0 max-w-[24rem] flex-1 items-center gap-2 rounded-md px-2 text-[14px] transition-colors hover:bg-foreground/5 sm:flex-none"
      >
        <span class="flex-none font-semibold text-foreground">Translations</span>
        <span class="min-w-0 truncate text-foreground-secondary" dir="auto">
          {translations.length === 0 ? "Arabic only" : names}
        </span>
        <ChevronDownIcon class="size-4 flex-none text-foreground-secondary" />
      </button>
    {/if}

    <span class="hidden min-w-0 truncate text-[13.5px] text-foreground-secondary md:inline">
      {surah.num}. {surah.name}
      <span dir="rtl" lang="ar" class="ms-1 font-arabic">{surah.arabic}</span>
    </span>

    <div class="ms-auto flex flex-none items-center gap-2">
      {#if resume === "b"}
        <button
          type="button"
          aria-label={`Resume at ${resumeName} ${resumeKey}`}
          class="flex h-9 items-center gap-1.5 rounded-md px-1.5 text-[13.5px] font-medium text-(--mix-accent-text) transition-colors hover:bg-primary/10 sm:px-2.5"
        >
          <Icon name="play" size={13} />
          <span class="hidden sm:inline">Resume {resumeName}</span>
          <span>{resumeKey}</span>
        </button>
      {/if}
      {#if showSize}
        <div
          class="flex items-center gap-0.5 rounded-md border border-(--mix-line) p-0.5"
          role="group"
          aria-label="Arabic text size"
        >
          <button
            type="button"
            onclick={onSmaller}
            aria-label="Smaller Arabic text"
            class="flex h-8 w-8 items-center justify-center rounded-sm text-[13px] text-foreground transition-colors hover:bg-foreground/5 sm:w-9"
          >
            A&minus;
          </button>
          <span class="h-4 w-px bg-(--mix-line)" aria-hidden="true"></span>
          <button
            type="button"
            onclick={onBigger}
            aria-label="Larger Arabic text"
            class="flex h-8 w-8 items-center justify-center rounded-sm text-[15px] text-foreground transition-colors hover:bg-foreground/5 sm:w-9"
          >
            A+
          </button>
        </div>
      {/if}
    </div>
  </div>
</header>
