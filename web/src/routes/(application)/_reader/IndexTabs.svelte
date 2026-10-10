<script lang="ts">
  import { getReaderUiCopy } from "#lib/i18n/reader-copy.js";
  import { readerHrefFor } from "#lib/i18n/reader.js";
  import { publicHref } from "#lib/i18n/public-href.js";

  /**
   * Index switcher for the sticky reader bar on /surah, /juz and /pages: three
   * tabs sharing the ReaderModeToggle segmented shape, so the control reads as
   * one family. Hrefs are the canonical index routes (no translation context —
   * indexes have no /t/ twins), matching the header's index links.
   */
  let { active }: { active: "surah" | "juz" | "pages" } = $props();

  const copy = getReaderUiCopy();

  const tabs = [
    {
      id: "surah",
      label: copy.index.surahsTitle,
      href: publicHref(readerHrefFor(copy.locale, "/surah")),
    },
    { id: "juz", label: copy.index.juzTitle, href: publicHref(readerHrefFor(copy.locale, "/juz")) },
    {
      id: "pages",
      label: copy.index.pagesTitle,
      href: publicHref(readerHrefFor(copy.locale, "/pages")),
    },
  ] as const;
</script>

<nav
  aria-label={copy.sidebar.browseLabel}
  data-index-tabs
  class="flex flex-none items-center gap-0.5 rounded-md border border-border bg-surface p-0.5"
>
  {#each tabs as tab (tab.id)}
    <a
      href={tab.href}
      data-sveltekit-preload-data="hover"
      aria-current={tab.id === active ? "page" : undefined}
      class={[
        "flex h-[34px] flex-none items-center rounded-sm px-3 text-[13px] font-medium transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring",
        tab.id === active
          ? "bg-foreground text-background hover:text-background"
          : "text-foreground-secondary hover:text-foreground",
      ]}>{tab.label}</a
    >
  {/each}
</nav>
