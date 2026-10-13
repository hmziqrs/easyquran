<script lang="ts">
  import ChevronDownIcon from "@lucide/svelte/icons/chevron-down";

  import { Icon } from "#lib/components/icon/index.js";
  import { getReaderUiCopy } from "#lib/i18n/reader-copy.js";
  import type { UiLocale } from "#lib/i18n/locales.js";
  import { cn } from "#lib/utils.js";

  /**
   * The reader's Reading-text control (ReaderHeader), shared with the card
   * form's translation field: an outlined well of two pills — Arabic or the
   * flowing text's name — plus the chevron that opens the full Translations
   * picker (TranslationModal in readPick mode). One tap picks the text;
   * the caller decides what "picking" does.
   */
  let {
    locale,
    flowing,
    translationName,
    onArabic,
    onTranslation,
    onOpen,
    class: className,
  }: {
    /** Resolves the pill copy; omitted falls back to the page locale like the reader's chrome. */
    locale?: UiLocale;
    /** Which pill is active. */
    flowing: "arabic" | "translation";
    /** Shown on the Translation pill; null shows the bare "Translation". */
    translationName: string | null;
    onArabic: () => void;
    onTranslation: () => void;
    onOpen: () => void;
    class?: string;
  } = $props();

  // Locale changes navigate (full reload), so resolving once matches
  // getReaderUiCopy's lifecycle (same as TranslationModal).
  // svelte-ignore state_referenced_locally
  const copy = getReaderUiCopy(locale);

  // Mode pills sit in an outlined well on the reading ground; the active pill is
  // ground-inverted (fg on bg), never the palette accent.
  // On phones the pills share the row evenly; from sm up they size to their labels.
  const pill =
    "flex h-8 min-w-0 flex-1 items-center justify-center gap-1.5 rounded-sm px-3 text-[13px] font-medium transition-colors text-foreground-secondary hover:text-foreground aria-pressed:bg-foreground aria-pressed:text-background aria-pressed:hover:text-background sm:flex-none";
</script>

<div
  class={cn("flex w-full items-center gap-0.5 rounded-md border border-border p-0.5", className)}
  role="group"
  aria-label={copy.shell.readingTranslationPick}
>
  <button type="button" aria-pressed={flowing === "arabic"} onclick={onArabic} class={pill}>
    <Icon name="continuous" size={13} />
    <span>{copy.shell.readingArabic}</span>
  </button>
  <button
    type="button"
    aria-pressed={flowing === "translation"}
    onclick={onTranslation}
    class={pill}
  >
    <span class="max-w-[11rem] truncate" dir="auto">
      {flowing === "translation" && translationName ? translationName : copy.shell.readingTranslation}
    </span>
  </button>
  <button
    type="button"
    aria-haspopup="dialog"
    aria-label={copy.shell.readingTranslationPick}
    title={copy.shell.readingTranslationPick}
    onclick={onOpen}
    class="flex h-8 w-9 flex-none items-center justify-center rounded-sm text-foreground-secondary transition-colors hover:text-foreground"
  >
    <ChevronDownIcon class="size-3.5" />
  </button>
</div>
