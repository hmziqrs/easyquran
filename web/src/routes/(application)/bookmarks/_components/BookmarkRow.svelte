<script lang="ts">
  import { Icon } from "#lib/components/icon/index.js";
  import * as Select from "#lib/components/ui/select/index.js";
  import type { BookmarksCopy } from "#lib/i18n/bookmarks-copy.js";
  import type { BookmarkFolder } from "#lib/bookmarks/schema.js";

  /**
   * One row component for signed-in AND anonymous bookmarks (Feature 2.4):
   * anonymous rows pass a pseudo-row (id = verse key, no timestamps) and omit
   * `onMove`/`onSelect`, so the sync-only controls never render offline.
   */
  interface RowBookmark {
    readonly id: string;
    readonly folderId: string | null;
    readonly surah: number;
    readonly ayah: number;
    /** Range end (Feature 7.4): absent/null = single ayah. */
    readonly endSurah?: number | null;
    readonly endAyah?: number | null;
    /** ISO timestamp; "" (anonymous legacy rows) hides the saved-date line. */
    readonly updatedAt: string;
  }

  let {
    copy,
    bookmark,
    surahName,
    href,
    folder = null,
    folders = [],
    onMove,
    onRemove,
    onSelect,
    selected = false,
    noteIcon = false,
  }: {
    copy: BookmarksCopy;
    /** A full `Bookmark` or an anonymous pseudo-row. */
    bookmark: RowBookmark;
    surahName: string;
    href: string | null;
    folder?: BookmarkFolder | null;
    folders?: readonly BookmarkFolder[];
    /** Present only on the signed-in surface — hides the move control otherwise. */
    onMove?: (bookmarkId: string, folderId: string | null) => void;
    onRemove: (bookmarkId: string) => void;
    /** Present only when bulk selection is active (P05). */
    onSelect?: (bookmarkId: string, next: boolean) => void;
    selected?: boolean;
    /** Local note exists for this verse (BM-02) — notes never leave the device. */
    noteIcon?: boolean;
  } = $props();

  /** Range suffix ("2:285–286"; cross-surah "285–3:1"); "" keeps singles unchanged. */
  function rangeSuffix(): string {
    const endAyah = bookmark.endAyah;
    if (endAyah === undefined || endAyah === null || endAyah === bookmark.ayah) return "";
    const endSurah = bookmark.endSurah;
    if (endSurah === undefined || endSurah === null || endSurah === bookmark.surah) {
      return `–${endAyah}`;
    }
    return `–${endSurah}:${endAyah}`;
  }

  const reference = $derived(`${surahName} ${bookmark.surah}:${bookmark.ayah}${rangeSuffix()}`);
  const ayahLabel = $derived(`${copy.ayah(bookmark.ayah)}${rangeSuffix()}`);

  // Orphan folderId (folder deleted server-side, snapshot raced): fall back to
  // the root option so the select matches the "Unfiled" grouping instead of
  // showing nothing selected.
  const moveValue = $derived.by<string>(() => {
    const id = bookmark.folderId;
    if (id === null) return "";
    return folders.some((folderOption) => folderOption.id === id) ? id : "";
  });

  // shadcn Select label persistence: the trigger keeps naming the destination
  // folder from `items` after the menu closes.
  const moveItems = $derived([
    { value: "", label: copy.unfiled },
    ...folders.map((folderOption) => ({ value: folderOption.id, label: folderOption.name })),
  ]);

  // "Saved 3 days ago" (BM-02): relative day in the page locale; legacy
  // anonymous rows (updatedAt "") and unparsable stamps show nothing.
  const savedLabel = $derived.by<string | null>(() => {
    if (bookmark.updatedAt.length === 0) return null;
    const parsed = Date.parse(bookmark.updatedAt);
    if (Number.isNaN(parsed)) return null;
    const days = Math.max(0, Math.floor((Date.now() - parsed) / 86_400_000));
    const when = new Intl.RelativeTimeFormat(copy.locale, { numeric: "auto" }).format(
      -days,
      "day",
    );
    return copy.saved(when);
  });

  const ghostButton =
    "inline-flex h-8 items-center gap-1.5 rounded-lg border border-border-strong px-2.5 text-[12.5px] text-foreground-secondary transition-colors hover:border-border hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring";
</script>

<li class="flex flex-wrap items-center gap-x-3 gap-y-2 px-4 py-3 sm:px-5">
  {#if onSelect}
    <input
      type="checkbox"
      checked={selected}
      onchange={(event) => onSelect?.(bookmark.id, event.currentTarget.checked)}
      aria-label={copy.selectLabel(reference)}
      class="size-4 flex-none accent-[var(--focus-ring)]"
    />
  {/if}
  <Icon name="bookmark" size={15} class="shrink-0 text-primary" />
  <span class="flex min-w-0 flex-1 flex-col gap-0.5">
    {#if href}
      <a
        href={href}
        class="truncate text-[14px] font-medium text-foreground underline-offset-4 hover:underline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
        >{surahName}
        <span class="font-normal text-foreground-secondary">· {ayahLabel}</span></a
      >
    {:else}
      <span class="truncate text-[14px] font-medium text-foreground"
        >{surahName}
        <span class="font-normal text-foreground-secondary">· {ayahLabel}</span></span
      >
    {/if}
    {#if folder}
      <span class="w-fit rounded-pill border border-border-strong bg-background-subtle px-2 py-0.5 text-[11.5px] text-muted"
        >{folder.name}</span
      >
    {/if}
    {#if savedLabel !== null || noteIcon}
      <span class="flex items-center gap-1.5 text-[11.5px] text-muted">
        {#if noteIcon}
          <Icon name="note" size={12} class="shrink-0" />
          <span class="sr-only">{copy.noteLabel}</span>
        {/if}
        {#if savedLabel !== null}{savedLabel}{/if}
      </span>
    {/if}
  </span>

  <span class="flex items-center gap-1.5">
    {#if onMove}
      <!-- ACCT-04: a "Move to…" menu (folder glyph + destination) vs a bare select. -->
      <span class="inline-flex items-center gap-1 text-[12.5px] text-foreground-secondary">
        <Icon name="rows" size={13} class="text-muted" />
        {copy.moveTo}
      </span>
      <Select.Root
        type="single"
        value={moveValue}
        onValueChange={(value) => onMove?.(bookmark.id, value === "" ? null : value)}
        items={moveItems}
      >
        <Select.Trigger size="sm" aria-label={copy.moveLabel(reference)}>
          <Select.Value />
        </Select.Trigger>
        <Select.Content>
          <Select.Item value="" label={copy.unfiled}>{copy.unfiled}</Select.Item>
          {#each folders as folderOption (folderOption.id)}
            <Select.Item value={folderOption.id} label={folderOption.name}>
              {folderOption.name}
            </Select.Item>
          {/each}
        </Select.Content>
      </Select.Root>
    {/if}
    <button
      type="button"
      class={ghostButton}
      aria-label="{copy.removeLabel}: {reference}"
      onclick={() => onRemove(bookmark.id)}
    >
      <Icon name="x" size={13} />
      {copy.remove}
    </button>
  </span>
</li>
