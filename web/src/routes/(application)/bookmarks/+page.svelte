<script lang="ts">
  import { onMount } from "svelte";
  import { SvelteSet } from "svelte/reactivity";
  import { toast } from "svelte-sonner";
  import { loadQuranData, peekQuranData } from "#lib/data/quran-data-client.js";
  import type { QuranData } from "#lib/data/quran-data.js";
  import {
    surahAyahPathFor,
    surahRouteContext,
    type SurahRouteContext,
  } from "#lib/data/quran.js";
  import { peekTranslationName } from "#lib/quran/catalogue.js";
  import { authModal } from "#lib/auth/auth-modal.svelte.js";
  import { authState } from "#lib/auth/auth-state.svelte.js";
  import { online } from "#lib/offline/online.svelte.js";
  import { reader } from "#lib/stores/reader.svelte.js";
  import { bookmarks } from "#lib/bookmarks/store.svelte.js";
  import type { Bookmark } from "#lib/bookmarks/schema.js";
  import { verseKeyOf } from "#lib/bookmarks/schema.js";
  import { appLocale } from "#lib/i18n/app-locale.js";
  import { getBookmarksCopy } from "#lib/i18n/bookmarks-copy.js";
  import { entryAttrs } from "#lib/sessions/entry.js";
  import { Button } from "#lib/components/ui/button/index.js";
  import * as Select from "#lib/components/ui/select/index.js";
  import { Icon } from "#lib/components/icon/index.js";
  import { Skeleton } from "#lib/components/ui/skeleton/index.js";
  import BookmarkRow from "./_components/BookmarkRow.svelte";
  import FoldersPanel from "./_components/FoldersPanel.svelte";
  import SyncIndicator from "./_components/SyncIndicator.svelte";

  // Opening a bookmark starts a reading thread at that verse.
  const BOOKMARK_ENTRY = entryAttrs({ origin: "bookmark", exact: true });
  // A personal page has no translation segments: saved translations reopen in
  // their own context, everything else in the Arabic reader.
  const ARABIC_CTX = surahRouteContext("uthmani");

  function ctxForSavedSource(sourceId: string | null): SurahRouteContext {
    if (sourceId !== null && peekTranslationName(sourceId) !== null) {
      return surahRouteContext(sourceId);
    }
    return ARABIC_CTX;
  }

  // This URL is never locale-prefixed; the layout's published locale keeps the
  // copy aligned with the nav/footer chrome.
  const copy = getBookmarksCopy(appLocale());

  let quranData = $state<QuranData | null>(peekQuranData() ?? null);

  const authed = $derived(bookmarks.authed);
  // Signed-in cold load: while the auth probe is in flight, show a neutral
  // skeleton — never the signed-out note.
  const authUnknown = $derived(authState.unknown);
  const folders = $derived(bookmarks.folders);

  // List controls: sort by date / folder / surah, filter by folder.
  type SortMode = "date" | "folder" | "surah";
  const SORT_MODES: readonly SortMode[] = ["date", "folder", "surah"];
  let sortMode = $state<SortMode>("folder");
  let folderFilter = $state<string>("all");

  const sortItems = [
    { value: "date", label: copy.sortDate },
    { value: "folder", label: copy.sortFolder },
    { value: "surah", label: copy.sortSurah },
  ];
  const folderItems = $derived(folders.map((folder) => ({ value: folder.id, label: folder.name })));
  const filterItems = $derived([{ value: "all", label: copy.filterAll }, ...folderItems]);

  const filteredRows = $derived(
    folderFilter === "all"
      ? bookmarks.bookmarks
      : bookmarks.bookmarks.filter((bookmark) => bookmark.folderId === folderFilter),
  );

  function byNewest(a: Bookmark, b: Bookmark): number {
    return b.updatedAt.localeCompare(a.updatedAt);
  }

  function byVerse(a: Bookmark, b: Bookmark): number {
    return a.surah - b.surah || a.ayah - b.ayah;
  }

  const flatRows = $derived.by<readonly Bookmark[]>(() => {
    if (sortMode === "folder") return [];
    const compare = sortMode === "date" ? byNewest : byVerse;
    return [...filteredRows].sort((a, b) => compare(a, b));
  });

  interface BookmarkGroup {
    readonly id: string;
    readonly label: string;
    readonly rows: readonly Bookmark[];
  }

  const groups = $derived.by<BookmarkGroup[]>(() => {
    const visible = folderFilter === "all" ? folders : folders.filter((f) => f.id === folderFilter);
    return visible.map((folder) => ({
      id: folder.id,
      label: folder.name,
      rows: filteredRows.filter((row) => row.folderId === folder.id).sort((a, b) => byNewest(a, b)),
    }));
  });

  const hasAnyBookmark = $derived(bookmarks.bookmarks.length > 0);

  // Authed with no snapshot yet and nothing cached: keep the skeleton only
  // while a snapshot can still land (online, inside a bounded window).
  const FIRST_SYNC_SKELETON_MAX_MS = 10_000;
  let firstSyncWindowElapsed = $state(false);
  const waitingForFirstSync = $derived(
    authed &&
      bookmarks.syncedAt === null &&
      !hasAnyBookmark &&
      online.online &&
      !firstSyncWindowElapsed,
  );
  const loading = $derived(authUnknown || waitingForFirstSync || quranData === null);

  function rowMeta(bookmark: Bookmark): { name: string; href: string } | null {
    const entry = quranData?.surahByNum(bookmark.surah);
    if (entry === undefined) return null;
    return {
      name: entry.name,
      href: surahAyahPathFor(ctxForSavedSource(bookmark.sourceId), entry, bookmark.ayah),
    };
  }

  // Notes never leave the device: the icon resolves against the local note store.
  function hasNote(bookmark: Bookmark): boolean {
    return reader.getNote(verseKeyOf(bookmark.surah, bookmark.ayah)).trim().length > 0;
  }

  // Bulk selection: move/remove many rows at once.
  let selected = new SvelteSet<string>();

  function toggleSelect(bookmarkId: string, next: boolean): void {
    if (next) selected.add(bookmarkId);
    else selected.delete(bookmarkId);
  }

  function moveSelected(folderId: string): void {
    for (const id of selected) bookmarks.moveToFolder(id, folderId);
    selected.clear();
  }

  /** Optimistic removal + one Undo toast (rapid removes share it). */
  function removeRows(rows: readonly Bookmark[]): void {
    if (rows.length === 0) return;
    for (const row of rows) bookmarks.remove(row.id);
    toast(copy.removed, {
      id: "bookmarks-removed",
      action: { label: copy.undo, onClick: () => bookmarks.restore(rows) },
    });
  }

  function removeSelected(): void {
    removeRows(bookmarks.bookmarks.filter((bookmark) => selected.has(bookmark.id)));
    selected.clear();
  }

  function deleteFolder(id: string): void {
    const deleted = bookmarks.deleteFolder(id);
    if (deleted === null) return;
    if (folderFilter === id) folderFilter = "all";
    toast(copy.folderDeleted, {
      action: { label: copy.undo, onClick: () => bookmarks.restoreFolder(deleted) },
    });
  }

  /** Manual order over the non-default folders: move, then one position write per changed slot. */
  function reorderFolder(id: string, direction: -1 | 1): void {
    const movable = folders.filter((folder) => !folder.isDefault);
    const index = movable.findIndex((folder) => folder.id === id);
    const target = index + direction;
    if (index === -1 || target < 0 || target >= movable.length) return;
    const next = [...movable];
    const [moved] = next.splice(index, 1);
    if (moved === undefined) return;
    next.splice(target, 0, moved);
    for (const [slot, folder] of next.entries()) {
      if (folder.position !== slot) bookmarks.setFolderPosition(folder.id, slot);
    }
  }

  onMount(() => {
    bookmarks.hydrate();
    if (peekQuranData() === undefined) {
      void loadQuranData().then(
        (data) => {
          quranData = data;
        },
        () => undefined,
      );
    }
    const skeletonTimer = setTimeout(() => {
      firstSyncWindowElapsed = true;
    }, FIRST_SYNC_SKELETON_MAX_MS);
    return () => clearTimeout(skeletonTimer);
  });
</script>

<svelte:head>
  <title>{copy.title} · EasyQuran</title>
</svelte:head>

<div lang={copy.locale} dir={copy.direction}>
  {#snippet rowList(rows: readonly Bookmark[])}
    <ul class="divide-y divide-border">
      {#each rows as bookmark (bookmark.id)}
        {@const meta = rowMeta(bookmark)}
        {#if meta !== null}
          <BookmarkRow
            {copy}
            bookmark={{ ...bookmark, updatedAt: bookmark.createdAt }}
            surahName={meta.name}
            href={meta.href}
            linkAttrs={BOOKMARK_ENTRY}
            folder={folderFilter === "all" && sortMode !== "folder"
              ? (bookmarks.folderById(bookmark.folderId) ?? null)
              : null}
            {folders}
            onMove={(id, folderId) => bookmarks.moveToFolder(id, folderId)}
            onRemove={() => removeRows([bookmark])}
            onSelect={toggleSelect}
            selected={selected.has(bookmark.id)}
            noteIcon={hasNote(bookmark)}
          />
        {/if}
      {/each}
    </ul>
  {/snippet}

  <div class="mx-auto max-w-[1180px] px-6 pt-5 pb-10 sm:px-7 sm:pt-6 sm:pb-12">
    <div class="flex flex-wrap items-center justify-between gap-3">
      <h1 class="text-[28px] font-medium leading-tight tracking-[-0.02em] text-foreground">
        {copy.title}
      </h1>
      {#if authUnknown}
        <!-- Neutral while the auth probe is in flight. -->
      {:else if authed}
        <SyncIndicator
          {copy}
          status={bookmarks.status}
          online={online.online}
          syncedAt={bookmarks.syncedAt}
          enqueueError={bookmarks.enqueueError}
        />
      {:else}
        <span class="text-caption text-muted">{copy.localOnly}</span>
      {/if}
    </div>

    {#if loading}
      <div class="mt-6 flex flex-col gap-3" role="status">
        <span class="sr-only">{copy.loading}</span>
        <Skeleton class="h-16 w-full" />
        <Skeleton class="h-16 w-full" />
        <Skeleton class="h-16 w-full" />
      </div>
    {:else}
      {#if authed && !online.online && bookmarks.syncedAt === null}
        <p class="mt-4 max-w-[70ch] text-caption leading-relaxed text-muted">{copy.offline}</p>
      {/if}
      {#if !hasAnyBookmark}
        <div class="mt-6 flex items-center gap-3.5">
          <span
            class="flex size-11 flex-none items-center justify-center rounded-sm text-[var(--hue-2-legible)]"
            style:background="var(--hue-2-soft)"
            aria-hidden="true"
          >
            <Icon name="bookmark" size={20} />
          </span>
          <p class="max-w-[70ch] text-[14.5px] leading-relaxed text-foreground-secondary">
            {copy.emptyPage}
          </p>
        </div>
      {/if}

      <FoldersPanel
        {copy}
        {folders}
        countIn={(id) => bookmarks.countIn(id)}
        onCreate={(name) => bookmarks.createFolder(name)}
        onRename={(id, name) => bookmarks.renameFolder(id, name)}
        onDelete={deleteFolder}
        onReorder={reorderFolder}
      />

      {#if selected.size > 0}
        <div
          class="mt-6 flex flex-wrap items-center gap-2 rounded-xl border border-border-strong bg-background-subtle px-4 py-2.5 sm:px-5"
        >
          <span class="text-caption tabular-nums text-foreground-secondary">
            {copy.selected(selected.size)}
          </span>
          <!-- An action menu, not a stateful field: the trigger stays on its prompt. -->
          <Select.Root
            type="single"
            value={undefined}
            onValueChange={(value) => moveSelected(value)}
            items={folderItems}
          >
            <Select.Trigger size="sm" aria-label={copy.moveSelectedTo}>
              <Select.Value placeholder={copy.moveSelectedTo} />
            </Select.Trigger>
            <Select.Content>
              {#each folders as folder (folder.id)}
                <Select.Item value={folder.id} label={folder.name}>{folder.name}</Select.Item>
              {/each}
            </Select.Content>
          </Select.Root>
          <Button variant="secondary" size="sm" class="text-danger" onclick={removeSelected}>
            {copy.removeSelected}
          </Button>
          <Button variant="ghost" size="sm" onclick={() => selected.clear()}>
            {copy.clearSelection}
          </Button>
        </div>
      {/if}

      {#if hasAnyBookmark}
        <div class="mt-6 flex flex-wrap items-center gap-x-5 gap-y-2">
          <span class="flex items-center gap-1.5">
            <span class="text-caption text-muted">{copy.sortLabel}</span>
            <Select.Root
              type="single"
              value={sortMode}
              onValueChange={(value) => {
                sortMode = SORT_MODES.find((mode) => mode === value) ?? "folder";
              }}
              items={sortItems}
            >
              <Select.Trigger size="sm" aria-label={copy.sortLabel}>
                <Select.Value />
              </Select.Trigger>
              <Select.Content>
                {#each sortItems as item (item.value)}
                  <Select.Item value={item.value} label={item.label}>{item.label}</Select.Item>
                {/each}
              </Select.Content>
            </Select.Root>
          </span>
          <span class="flex items-center gap-1.5">
            <span class="text-caption text-muted">{copy.filterLabel}</span>
            <Select.Root
              type="single"
              value={folderFilter}
              onValueChange={(value) => {
                folderFilter = value;
              }}
              items={filterItems}
            >
              <Select.Trigger size="sm" aria-label={copy.filterLabel}>
                <Select.Value />
              </Select.Trigger>
              <Select.Content>
                {#each filterItems as item (item.value)}
                  <Select.Item value={item.value} label={item.label}>{item.label}</Select.Item>
                {/each}
              </Select.Content>
            </Select.Root>
          </span>
        </div>

        {#if sortMode === "folder"}
          <div class="mt-6 flex flex-col gap-6">
            {#each groups as group (group.id)}
              <section
                aria-labelledby="bookmarks-group-{group.id}"
                class="overflow-hidden rounded-md border border-border-strong bg-surface"
              >
                <h2 id="bookmarks-group-{group.id}" class="eyebrow mb-0 border-b border-border px-4 py-3 sm:px-5">
                  {group.label}
                  <span class="ms-1 text-[12px] text-muted">{group.rows.length}</span>
                </h2>
                {#if group.rows.length === 0}
                  <p class="px-4 py-3.5 text-caption text-muted sm:px-5">{copy.folderEmpty}</p>
                {:else}
                  {@render rowList(group.rows)}
                {/if}
              </section>
            {/each}
          </div>
        {:else if flatRows.length > 0}
          <div class="mt-6 overflow-hidden rounded-md border border-border-strong bg-surface">
            {@render rowList(flatRows)}
          </div>
        {/if}
      {/if}

      {#if !authed && !authUnknown}
        <div
          class="mt-6 flex flex-wrap items-center justify-between gap-3 rounded-xl border border-border-strong bg-background-subtle px-4 py-3.5 sm:px-5"
        >
          <p class="max-w-[60ch] text-caption leading-relaxed text-foreground-secondary">
            {copy.signInNote}
          </p>
          <Button variant="primary" size="sm" onclick={() => authModal.show("login")}>
            {copy.signIn}
          </Button>
        </div>
      {/if}
    {/if}
  </div>
</div>
