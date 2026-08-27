<script lang="ts">
  import { onMount } from "svelte";
  import { loadQuranData, peekQuranData } from "$lib/data/quran-data-client";
  import type { QuranData } from "$lib/data/quran-data";
  import { surahAyahPathFor, surahRouteContext } from "$lib/data/quran";
  import { authModal } from "$lib/auth/auth-modal.svelte";
  import { online } from "$lib/offline/online.svelte";
  import { reader } from "$lib/stores/reader.svelte";
  import { bookmarks } from "$lib/bookmarks/store.svelte";
  import type { Bookmark, BookmarkFolder } from "$lib/bookmarks/schema";
  import { parseVerseKey } from "$lib/bookmarks/schema";
  import { getBookmarksCopy } from "$lib/i18n/bookmarks-copy";
  import { Button } from "$lib/components/ui/button";
  import { Skeleton } from "$lib/components/ui/skeleton";
  import BookmarkRow from "./_components/BookmarkRow.svelte";
  import FoldersPanel from "./_components/FoldersPanel.svelte";
  import SyncIndicator from "./_components/SyncIndicator.svelte";

  // Reader nav context for opening a bookmarked verse: a non-reader app page has
  // no translation segments to build, so it always resolves the Arabic reader.
  const ARABIC_CTX = surahRouteContext("uthmani");

  const copy = getBookmarksCopy();

  let quranData = $state<QuranData | null>(peekQuranData() ?? null);

  const authed = $derived(bookmarks.authed);

  const sortedFolders = $derived(
    [...bookmarks.folders].sort((a, b) => a.name.localeCompare(b.name)),
  );

  interface BookmarkGroup {
    readonly id: string | null;
    readonly label: string;
    readonly rows: readonly Bookmark[];
  }

  // Store-side grouping: orphaned folderIds land under null, never hidden.
  const byFolder = $derived(bookmarks.bookmarksByFolder());

  function rowsFor(folderId: string | null): readonly Bookmark[] {
    return [...(byFolder.get(folderId) ?? [])].sort((a, b) =>
      b.updatedAt.localeCompare(a.updatedAt),
    );
  }

  const groups = $derived.by<BookmarkGroup[]>(() => {
    const unfiled: BookmarkGroup = { id: null, label: copy.unfiled, rows: rowsFor(null) };
    const named = sortedFolders.map(
      (folder): BookmarkGroup => ({ id: folder.id, label: folder.name, rows: rowsFor(folder.id) }),
    );
    // Unfiled first only when it carries rows; named folders always render so
    // their empty state stays visible.
    if (unfiled.rows.length === 0) return named;
    return [unfiled, ...named];
  });

  const hasAnyBookmark = $derived(bookmarks.bookmarks.length > 0);

  // Authed but no first snapshot yet: keep the skeleton up — even when the
  // round failed (offline first visit must not flash "No bookmarks"), and any
  // local optimistic row or folder means there is real content to render.
  const waitingForFirstSync = $derived(
    authed &&
      bookmarks.status.lastSyncAt === null &&
      !hasAnyBookmark &&
      sortedFolders.length === 0,
  );
  // Same skeleton for the blank window after sync while the quran catalog is
  // still loading (rows/folders exist but cannot render yet).
  const firstSyncPending = $derived(waitingForFirstSync || (authed && quranData === null));

  // Anon catalog wait: anonRows stays empty until the quran catalog lands, so
  // the empty state would flash — hold the loading skeleton instead.
  const anonCatalogPending = $derived(!authed && quranData === null);

  interface AnonRow {
    readonly key: string;
    readonly surah: number;
    readonly ayah: number;
    readonly name: string;
    readonly href: string;
  }

  const anonRows = $derived.by<AnonRow[]>(() => {
    const data = quranData;
    if (data === null) return [];
    const out: AnonRow[] = [];
    for (const key of reader.bookmarkedKeys) {
      const coords = parseVerseKey(key);
      if (coords === null) continue;
      const entry = data.surahByNum(coords.surah);
      if (!entry) continue;
      const localPage = data.surahLocalPageForAyah(coords.surah, coords.ayah)?.localPage ?? 1;
      out.push({
        key,
        surah: coords.surah,
        ayah: coords.ayah,
        name: entry.name,
        href: surahAyahPathFor(ARABIC_CTX, entry, localPage, coords.ayah),
      });
    }
    return out;
  });

  /** Display data for an authed bookmark row; null while the catalog is not loaded yet. */
  function rowMeta(bookmark: Bookmark): { name: string; href: string | null } | null {
    const data = quranData;
    if (data === null) return null;
    const entry = data.surahByNum(bookmark.surah);
    if (!entry) return null;
    const localPage = data.surahLocalPageForAyah(bookmark.surah, bookmark.ayah)?.localPage ?? 1;
    return {
      name: entry.name,
      href: surahAyahPathFor(ARABIC_CTX, entry, localPage, bookmark.ayah),
    };
  }

  function folderOf(bookmark: Bookmark): BookmarkFolder | null {
    if (bookmark.folderId === null) return null;
    return bookmarks.folderById(bookmark.folderId) ?? null;
  }

  function countIn(folderId: string): number {
    return byFolder.get(folderId)?.length ?? 0;
  }

  function removeAnon(key: string): void {
    reader.toggleBookmark(key);
  }

  function openSignIn(): void {
    authModal.show("login");
  }

  onMount(() => {
    if (peekQuranData() === undefined) {
      void loadQuranData().then(
        (data) => {
          quranData = data;
        },
        () => undefined,
      );
    }
  });
</script>

<svelte:head>
  <title>{copy.title} · EasyQuran</title>
</svelte:head>

<div lang={copy.locale} dir={copy.direction}>
  {#snippet loadingRows()}
    <span class="sr-only">{copy.loading}</span>
    <Skeleton class="h-16 w-full" />
    <Skeleton class="h-16 w-full" />
    <Skeleton class="h-16 w-full" />
  {/snippet}
  <div class="mx-auto max-w-[1180px] px-6 pt-5 pb-10 sm:px-7 sm:pt-6 sm:pb-12">
    <div class="flex flex-wrap items-center justify-between gap-3">
      <h1 class="text-[28px] font-semibold leading-tight tracking-[-0.02em] text-fg"
        >{copy.title}</h1
      >
      {#if authed}
        <SyncIndicator copy={copy} status={bookmarks.status} online={online.online} />
      {:else}
        <span class="text-[12px] text-fg-3">{copy.localOnly}</span>
      {/if}
    </div>

    {#if authed}
      {#if firstSyncPending}
        <div class="mt-6 flex flex-col gap-3" role="status">
          {@render loadingRows()}
          <!-- Offline note only; an online sync failure surfaces solely in the
               header SyncIndicator, never duplicated inline. -->
          {#if waitingForFirstSync && !online.online}
            <p class="max-w-[70ch] text-[13.5px] leading-relaxed text-fg-3">{copy.offline}</p>
          {/if}
        </div>
      {:else if quranData !== null && !hasAnyBookmark && sortedFolders.length === 0}
        <p class="mt-6 max-w-[70ch] text-[14.5px] leading-relaxed text-fg-2">{copy.empty}</p>
      {:else if quranData !== null}
        <FoldersPanel
          {copy}
          folders={sortedFolders}
          {countIn}
          onCreate={(name) => bookmarks.createFolder(name)}
          onRename={(id, name) => bookmarks.renameFolder(id, name)}
          onDelete={(id) => bookmarks.deleteFolder(id)}
        />

        {#if hasAnyBookmark}
          <div class="mt-6 flex flex-col gap-6">
            {#each groups as group (group.id ?? "unfiled")}
              <section
                aria-labelledby="bookmarks-group-{group.id ?? `unfiled`}"
                class="overflow-hidden rounded-xl border border-line-2 bg-bg-1"
              >
                <h2
                  id="bookmarks-group-{group.id ?? `unfiled`}"
                  class="eyebrow mb-0 border-b border-line px-4 py-3 sm:px-5"
                  >{group.label}
                  <span class="ms-1 font-mono text-[11px] text-fg-4">{group.rows.length}</span>
                </h2>
                {#if group.rows.length === 0}
                  <p class="px-4 py-3.5 text-[13.5px] text-fg-3 sm:px-5">{copy.folderEmpty}</p>
                {:else}
                  <ul class="divide-y divide-line">
                    {#each group.rows as bookmark (bookmark.id)}
                      {@const meta = rowMeta(bookmark)}
                      {#if meta !== null}
                        <BookmarkRow
                          {copy}
                          {bookmark}
                          surahName={meta.name}
                          href={meta.href}
                          folder={folderOf(bookmark)}
                          folders={sortedFolders}
                          onMove={(id, folderId) => bookmarks.moveToFolder(id, folderId)}
                          onRemove={(id) => bookmarks.remove(id)}
                        />
                      {/if}
                    {/each}
                  </ul>
                {/if}
              </section>
            {/each}
          </div>
        {/if}
      {/if}
    {:else}
      {#if anonCatalogPending}
        <div class="mt-6 flex flex-col gap-3" role="status">
          {@render loadingRows()}
        </div>
      {:else if anonRows.length === 0}
        <p class="mt-6 max-w-[70ch] text-[14.5px] leading-relaxed text-fg-2">{copy.empty}</p>
      {:else}
        <ul class="mt-6 divide-y divide-line overflow-hidden rounded-xl border border-line-2 bg-bg-1">
          {#each anonRows as row (row.key)}
            <li class="flex flex-wrap items-center gap-x-3 gap-y-2 px-4 py-3 sm:px-5">
              <span class="min-w-0 flex-1 truncate text-[14px] font-medium text-fg">
                <a
                  href={row.href}
                  class="underline-offset-4 hover:underline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent"
                  >{row.name}
                  <span class="font-normal text-fg-2">{copy.ayah(row.ayah)}</span></a
                >
              </span>
              <button
                type="button"
                class="inline-flex h-8 items-center gap-1.5 rounded-lg border border-line-2 px-2.5 text-[12.5px] text-fg-2 transition-colors hover:border-line hover:text-fg focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent"
                aria-label="{copy.removeLabel}: {row.surah}:{row.ayah}"
                onclick={() => removeAnon(row.key)}
              >
                {copy.remove}
              </button>
            </li>
          {/each}
        </ul>
      {/if}

      <div
        class="mt-6 flex flex-wrap items-center justify-between gap-3 rounded-xl border border-line-2 bg-bg-2 px-4 py-3.5 sm:px-5"
      >
        <p class="max-w-[60ch] text-[13.5px] leading-relaxed text-fg-2">{copy.signInNote}</p>
        <Button variant="ghost" size="sm" onclick={openSignIn}>{copy.signIn}</Button>
      </div>
    {/if}
  </div>
</div>
