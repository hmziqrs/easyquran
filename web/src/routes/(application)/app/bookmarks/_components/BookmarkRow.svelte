<script lang="ts">
  import { Icon } from "$lib/components/icon";
  import { cn } from "$lib/utils";
  import type { BookmarksCopy } from "$lib/i18n/bookmarks-copy";
  import type { Bookmark, BookmarkFolder } from "$lib/bookmarks/schema";

  let {
    copy,
    bookmark,
    surahName,
    href,
    folder,
    folders,
    onMove,
    onRemove,
  }: {
    copy: BookmarksCopy;
    bookmark: Bookmark;
    surahName: string;
    href: string | null;
    folder: BookmarkFolder | null;
    folders: readonly BookmarkFolder[];
    onMove: (bookmarkId: string, folderId: string | null) => void;
    onRemove: (bookmarkId: string) => void;
  } = $props();

  const reference = $derived(`${surahName} ${bookmark.surah}:${bookmark.ayah}`);

  const ghostButton =
    "inline-flex h-8 items-center gap-1.5 rounded-lg border border-line-2 px-2.5 text-[12.5px] text-fg-2 transition-colors hover:border-line hover:text-fg focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent";
</script>

<li class="flex flex-wrap items-center gap-x-3 gap-y-2 px-4 py-3 sm:px-5">
  <Icon name="bookmark" size={15} class="shrink-0 text-accent" />
  <span class="flex min-w-0 flex-1 flex-col gap-0.5">
    {#if href}
      <a
        href={href}
        class="truncate text-[14px] font-medium text-fg underline-offset-4 hover:underline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent"
        >{surahName}
        <span class="font-normal text-fg-2">{copy.ayah(bookmark.ayah)}</span></a
      >
    {:else}
      <span class="truncate text-[14px] font-medium text-fg"
        >{surahName}
        <span class="font-normal text-fg-2">{copy.ayah(bookmark.ayah)}</span></span
      >
    {/if}
    {#if folder}
      <span class="w-fit rounded-full border border-line-2 bg-bg-2 px-2 py-0.5 text-[11.5px] text-fg-3"
        >{folder.name}</span
      >
    {/if}
  </span>

  <span class="flex items-center gap-1.5">
    <label class="sr-only" for="bookmarks-move-{bookmark.id}"
      >{copy.moveLabel(reference)}</label
    >
    <select
      id="bookmarks-move-{bookmark.id}"
      value={bookmark.folderId ?? ""}
      onchange={(event) => {
        const value = (event.currentTarget as HTMLSelectElement).value;
        onMove(bookmark.id, value === "" ? null : value);
      }}
      class={cn(ghostButton, "h-8 appearance-none bg-transparent pr-6")}
    >
      <option value="">{copy.unfiled}</option>
      {#each folders as folderOption (folderOption.id)}
        <option value={folderOption.id}>{folderOption.name}</option>
      {/each}
    </select>
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
