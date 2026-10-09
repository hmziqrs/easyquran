<script lang="ts">
  import { page } from "$app/state";
  import { replaceState } from "$app/navigation";
  import { Icon, type IconName } from "#lib/components/icon/index.js";
  import { getReaderUiCopy } from "#lib/i18n/reader-copy.js";
  import { withModeParam } from "#lib/reader/mode-param.js";
  import { visibleUrl } from "#lib/reader/visible-url.js";
  import { reader, type ReaderMode } from "#lib/stores/reader.svelte.js";
  import { changeTypography } from "./typography-change";

  /**
   * Ayah-by-Ayah / Reading switch for the sticky reader bar, so every reader route (surah,
   * page, juz, hizb, rub — Arabic or translated) can change mode from anywhere in the text,
   * not only from the top of a surah. The change rides the same anchor-preserving path as
   * A−/A+ (the mounted reader keeps the reading position across the reflow) and writes the
   * ?mode= param itself: the layout's sync effect reads the visible URL, and a stale param
   * would flip the mode straight back.
   */
  const copy = getReaderUiCopy();

  function setMode(mode: ReaderMode): void {
    if (reader.mode === mode) return;
    changeTypography(() => {
      reader.setMode(mode);
      // visibleUrl: kit 3 shallow writes (the ?v= verse anchor) live only there.
      replaceState(withModeParam(visibleUrl(page), mode), page.state);
    });
  }

  const options: { mode: ReaderMode; icon: IconName; label: string; short: string }[] = [
    { mode: "verse", icon: "rows", label: copy.shell.ayahByAyah, short: copy.shell.ayahs },
    { mode: "reading", icon: "book", label: copy.shell.reading, short: copy.shell.reading },
  ];
</script>

<!-- Active segment is ground-inverted (fg on bg), never the palette accent. Icons only on
     phones (aria-label carries the name); the short label joins from md up. -->
<div
  data-reader-mode-toggle
  class="flex flex-none items-center gap-0.5 rounded-md border border-border p-0.5"
  role="group"
  aria-label={copy.shell.readingModeLabel}
>
  {#each options as option (option.mode)}
    <button
      type="button"
      data-mode-option={option.mode}
      aria-pressed={reader.mode === option.mode}
      aria-label={option.label}
      title={option.label}
      onclick={() => setMode(option.mode)}
      class="flex h-[34px] min-w-9 touch-manipulation items-center justify-center gap-1.5 rounded-sm px-2 text-[13px] font-medium text-foreground-secondary transition-colors hover:text-foreground aria-pressed:bg-foreground aria-pressed:text-background aria-pressed:hover:text-background md:px-2.5"
    >
      <Icon name={option.icon} size={15} />
      <span class="hidden md:inline">{option.short}</span>
    </button>
  {/each}
</div>
